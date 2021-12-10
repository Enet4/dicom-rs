//! This crate contains the DICOM pixel data handlers and is
//! responsible for decoding various forms of native and compressed pixel data,
//! such as JPEG lossless,
//! and convert it into a [`DynamicImage`],
//! [`Array`] or raw [`DecodedPixelData`].
//!
//! `dicom-pixeldata` currently supports a small,
//! but increasing number of DICOM image encodings in pure Rust.
//! As a way to mitigate the current gap,
//! this library has an integration with [GDCM bindings]
//! for an extended range of encodings.
//! This integration is behind the Cargo feature "gdcm",
//! which requires CMake and a C++ compiler.
//!
//! [GDCM bindings]: https://github.com/pevers/gdcm-rs
//!
//! ```toml
//! dicom-pixeldata = { version = "0.1", features = ["gdcm"] }
//! ```
//!
//! # WebAssembly support
//! This library works in WebAssembly
//! by ensuring that the "gdcm" feature is disabled.
//! This allows the crate to be compiled for WebAssembly
//! albeit at the cost of supporting a lesser variety of compression algorithms.
//!
//! # Examples
//!
//! Example using `to_dynamic_image`
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::PixelDecoder;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("dicom.dcm")?;
//! let image = obj.decode_pixel_data()?;
//! let dynamic_image = image.to_dynamic_image(0)?;
//! dynamic_image.save("out.png")?;
//! # Ok(())
//! # }
//! ```
//!
//! Example using `to_ndarray`
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::PixelDecoder;
//! use ndarray::s;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("rgb_dicom.dcm")?;
//! let pixel_data = obj.decode_pixel_data()?;
//! let ndarray = pixel_data.to_ndarray::<u16>()?;
//! let red_values = ndarray.slice(s![.., .., 0]);
//! # Ok(())
//! # }
//! ```

use byteorder::{ByteOrder, NativeEndian};
use dicom_core::{value::Value, DataDictionary};
use dicom_encoding::adapters::DecodeError;
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_encoding::Codec;
use dicom_object::{FileDicomObject, InMemDicomObject};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use image::{DynamicImage, ImageBuffer, Luma, Rgb};
use ndarray::{Array, IxDyn};
use num_traits::NumCast;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use snafu::OptionExt;
use snafu::{ResultExt, Snafu};
use std::borrow::Cow;

pub use image;
pub use ndarray;

#[cfg(feature = "gdcm")]
mod gdcm;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Missing required element"))]
    MissingRequiredField { source: dicom_object::Error },

    #[snafu(display("Could not cast element"))]
    CastValueError {
        source: dicom_core::value::CastValueError,
    },

    #[snafu(display("Invalid PixelData"))]
    InvalidPixelData,

    #[snafu(display("Invalid PixelRepresentation, must be 0 or 1"))]
    InvalidPixelRepresentation,

    #[snafu(display("Invalid BitsAllocated, must be 8 or 16"))]
    InvalidBitsAllocated,

    #[snafu(display("Unsupported PhotometricInterpretation {}", pi))]
    UnsupportedPhotometricInterpretation { pi: String },

    #[snafu(display("Unsupported SamplesPerPixel {}", spp))]
    UnsupportedSamplesPerPixel { spp: u16 },

    #[snafu(display("Unsupported TransferSyntax {}", ts))]
    UnsupportedTransferSyntax { ts: String },

    #[snafu(display("Multi-frame dicoms are not supported"))]
    UnsupportedMultiFrame,

    #[snafu(display("Invalid buffer when constructing ImageBuffer"))]
    InvalidImageBuffer,

    #[snafu(display("Unknown error while decoding pixel data"))]
    ImplementerError {
        source: Box<dyn std::error::Error + Send + 'static>,
    },

    #[snafu(display("Invalid shape for ndarray"))]
    ShapeError { source: ndarray::ShapeError },

    #[snafu(display("Invalid data type for ndarray element"))]
    InvalidDataType,

    #[snafu(display("Unsupported color space"))]
    UnsupportedColorSpace,

    #[snafu(display("Pixel data decode error"))]
    PixelDecodeError { source: DecodeError },

    #[snafu(display("Frame out of range error"))]
    FrameOutOfRangeError,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Decoded pixel data
pub struct DecodedPixelData<'a> {
    pub data: Cow<'a, [u8]>,
    pub rows: u32,
    pub cols: u32,
    pub number_of_frames: u16,
    pub photometric_interpretation: String,
    pub samples_per_pixel: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
    // Enhanced MR Images are not yet supported having
    // a RescaleSlope/RescaleIntercept Per-Frame Functional Group
    pub rescale_intercept: i16,
    pub rescale_slope: f32,
}

impl DecodedPixelData<'_> {
    /// Convert decoded pixel data for a specific frame into a DynamicImage.
    /// A new <u8> or <u16> vector is created in memory
    /// with normalized grayscale values after applying Modality LUT.
    pub fn to_dynamic_image(&self, frame: u16) -> Result<DynamicImage> {
        match self.samples_per_pixel {
            1 => {
                let mut image = match self.bits_allocated {
                    8 => {
                        let frame_length = self.rows as usize
                            * self.cols as usize
                            * self.samples_per_pixel as usize;
                        let frame_start = frame_length * frame as usize;
                        let frame_end = frame_start + frame_length;
                        if frame_end > (*self.data).len() {
                            FrameOutOfRangeSnafu.fail()?
                        }
                        let buffer: Vec<u8> =
                            (&self.data[(frame_start as usize..frame_end as usize)]).to_vec();
                        let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                            ImageBuffer::from_raw(self.cols, self.rows, buffer)
                                .context(InvalidImageBufferSnafu)?;
                        DynamicImage::ImageLuma8(image_buffer)
                    }
                    16 => {
                        let frame_length = self.rows as usize
                            * self.cols as usize
                            * 2
                            * self.samples_per_pixel as usize;
                        let frame_start = frame_length * frame as usize;
                        let frame_end = frame_start + frame_length;
                        if frame_end > (*self.data).len() {
                            FrameOutOfRangeSnafu.fail()?
                        }
                        let mut buffer = vec![0; frame_length / 2];
                        match self.pixel_representation {
                            // Unsigned 16-bit representation
                            0 => {
                                NativeEndian::read_u16_into(
                                    &self.data[frame_start..frame_end],
                                    &mut buffer,
                                );
                            }
                            // Signed 16-bit representation
                            1 => {
                                let mut signed_buffer = vec![0; frame_length / 2];
                                NativeEndian::read_i16_into(
                                    &self.data[frame_start..frame_end],
                                    &mut signed_buffer,
                                );
                                // Convert buffer to unsigned
                                buffer = normalize_i16_to_u16(&signed_buffer);
                            }
                            _ => InvalidPixelRepresentationSnafu.fail()?,
                        };
                        let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                            ImageBuffer::from_raw(self.cols, self.rows, buffer)
                                .context(InvalidImageBufferSnafu)?;
                        DynamicImage::ImageLuma16(image_buffer)
                    }
                    _ => InvalidBitsAllocatedSnafu.fail()?,
                };
                // Convert MONOCHROME1 => MONOCHROME2
                if self.photometric_interpretation == "MONOCHROME1" {
                    image.invert();
                }
                Ok(image)
            }
            3 => {
                // RGB, YBR_FULL or YBR_FULL_422 colors
                match self.bits_allocated {
                    8 => {
                        let frame_length = self.rows as usize
                            * self.cols as usize
                            * self.samples_per_pixel as usize;
                        let frame_start = frame_length * frame as usize;
                        let frame_end = frame_start + frame_length;
                        if frame_end > (*self.data).len() {
                            FrameOutOfRangeSnafu.fail()?
                        }
                        let mut pixel_array: Vec<u8> =
                            (&self.data[(frame_start as usize..frame_end as usize)]).to_vec();

                        // Convert YBR_FULL or YBR_FULL_422 to RGB
                        let pixel_array = match self.photometric_interpretation.as_str() {
                            "RGB" => pixel_array,
                            "YBR_FULL" | "YBR_FULL_422" => {
                                convert_colorspace_u8(&mut pixel_array);
                                pixel_array
                            }
                            _ => UnsupportedColorSpaceSnafu.fail()?,
                        };

                        let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
                            ImageBuffer::from_raw(self.cols, self.rows, pixel_array)
                                .context(InvalidImageBufferSnafu)?;
                        Ok(DynamicImage::ImageRgb8(image_buffer))
                    }
                    16 => {
                        let frame_length = self.rows as usize
                            * self.cols as usize
                            * 2
                            * self.samples_per_pixel as usize;
                        let frame_start = frame_length * frame as usize;
                        let frame_end = frame_start + frame_length;
                        if frame_end > (*self.data).len() {
                            FrameOutOfRangeSnafu.fail()?
                        }
                        let buffer: Vec<u8> =
                            (&self.data[(frame_start as usize..frame_end as usize)]).to_vec();
                        let mut pixel_array: Vec<u16> = vec![0; (frame_length / 2) as usize];
                        NativeEndian::read_u16_into(&buffer, &mut pixel_array);

                        // Convert YBR_FULL or YBR_FULL_422 to RGB
                        let pixel_array = match self.photometric_interpretation.as_str() {
                            "RGB" => pixel_array,
                            "YBR_FULL" | "YBR_FULL_422" => {
                                convert_colorspace_u16(&mut pixel_array);
                                pixel_array
                            }
                            _ => UnsupportedColorSpaceSnafu.fail()?,
                        };

                        let image_buffer: ImageBuffer<Rgb<u16>, Vec<u16>> =
                            ImageBuffer::from_raw(self.cols, self.rows, pixel_array)
                                .context(InvalidImageBufferSnafu)?;
                        Ok(DynamicImage::ImageRgb16(image_buffer))
                    }
                    _ => InvalidBitsAllocatedSnafu.fail()?,
                }
            }
            _ => InvalidPixelRepresentationSnafu.fail()?,
        }
    }

    /// Convert decoded pixel data into an ndarray of a given type T.
    /// The pixel data type is extracted from the bits_allocated and
    /// pixel_representation, and automatically converted to the requested type T.
    pub fn to_ndarray<T>(&self) -> Result<Array<T, IxDyn>>
    where
        T: NumCast,
        T: Send,
    {
        // Array size is NumberOfFrames x Rows x Cols x SamplesPerPixel (1 for grayscale, 3 for RGB)
        let shape = IxDyn(&[
            self.number_of_frames as usize,
            self.rows as usize,
            self.cols as usize,
            self.samples_per_pixel as usize,
        ]);

        match self.bits_allocated {
            8 => {
                // 1-channel Grayscale image
                let converted: Result<Vec<T>, _> = self
                    .data
                    .par_iter()
                    .map(|v| T::from(*v).ok_or(snafu::NoneError))
                    .collect();
                let converted = converted.context(InvalidDataTypeSnafu)?;
                let ndarray = Array::from_shape_vec(shape, converted).context(ShapeSnafu)?;
                Ok(ndarray)
            }
            16 => match self.pixel_representation {
                // Unsigned 16 bit representation
                0 => {
                    let mut dest = vec![0; self.data.len() / 2];
                    NativeEndian::read_u16_into(&self.data, &mut dest);

                    let converted: Result<Vec<T>, _> = dest
                        .par_iter()
                        .map(|v| T::from(*v).ok_or(snafu::NoneError))
                        .collect();
                    let converted = converted.context(InvalidDataTypeSnafu)?;
                    let ndarray = Array::from_shape_vec(shape, converted).context(ShapeSnafu)?;
                    Ok(ndarray)
                }
                // Signed 16 bit 2s complement representation
                1 => {
                    let mut signed_buffer = vec![0; self.data.len() / 2];
                    NativeEndian::read_i16_into(&self.data, &mut signed_buffer);

                    let converted: Result<Vec<T>, _> = signed_buffer
                        .par_iter()
                        .map(|v| T::from(*v).ok_or(snafu::NoneError))
                        .collect();
                    let converted = converted.context(InvalidDataTypeSnafu)?;
                    let ndarray = Array::from_shape_vec(shape, converted).context(ShapeSnafu)?;
                    Ok(ndarray)
                }
                _ => InvalidPixelRepresentationSnafu.fail()?,
            },
            _ => InvalidBitsAllocatedSnafu.fail()?,
        }
    }
}

// Convert u8 pixel array from YBR_FULL or YBR_FULL_422 to RGB
// Every pixel is replaced with an RGB value
fn convert_colorspace_u8(i: &mut Vec<u8>) {
    // Matrix multiplication taken from
    // https://github.com/pydicom/pydicom/blob/f36517e10/pydicom/pixel_data_handlers/util.py#L576
    i.par_iter_mut().chunks(3).for_each(|mut pixel| {
        let y = *pixel[0] as f32;
        let b: f32 = *pixel[1] as f32;
        let r: f32 = *pixel[2] as f32;
        let b = b - 128.0;
        let r = r - 128.0;

        let cr = (y + 1.402 * r) + 0.5;
        let cg = (y + (0.114 * 1.772 / 0.587) * b + (-0.299 * 1.402 / 0.587) * r) + 0.5;
        let cb = (y + 1.772 * b) + 0.5;

        let cr = cr.floor().clamp(0.0, u8::MAX as f32) as u8;
        let cg = cg.floor().clamp(0.0, u8::MAX as f32) as u8;
        let cb = cb.floor().clamp(0.0, u8::MAX as f32) as u8;

        *pixel[0] = cr;
        *pixel[1] = cg;
        *pixel[2] = cb;
    });
}

// Convert u16 pixel array from YBR_FULL or YBR_FULL_422 to RGB
// Every pixel is replaced with an RGB value
fn convert_colorspace_u16(i: &mut Vec<u16>) {
    // Matrix multiplication taken from
    // https://github.com/pydicom/pydicom/blob/f36517e10/pydicom/pixel_data_handlers/util.py#L576
    i.par_iter_mut().chunks(3).for_each(|mut pixel| {
        let y = *pixel[0] as f32;
        let b: f32 = *pixel[1] as f32;
        let r: f32 = *pixel[2] as f32;
        let b = b - 32768.0;
        let r = r - 32768.0;

        let cr = (y + 1.402 * r) + 0.5;
        let cg = (y + (0.114 * 1.772 / 0.587) * b + (-0.299 * 1.402 / 0.587) * r) + 0.5;
        let cb = (y + 1.772 * b) + 0.5;

        let cr = cr.floor().clamp(0.0, u16::MAX as f32) as u16;
        let cg = cg.floor().clamp(0.0, u16::MAX as f32) as u16;
        let cb = cb.floor().clamp(0.0, u16::MAX as f32) as u16;

        *pixel[0] = cr;
        *pixel[1] = cg;
        *pixel[2] = cb;
    });
}

// Noramlize i16 vector using min/max normalization
fn normalize_i16_to_u16(i: &[i16]) -> Vec<u16> {
    let min = *i.iter().min().unwrap() as f64;
    let max = *i.iter().max().unwrap() as f64;
    i.par_iter()
        .map(|p| (u16::MAX as f64 * (*p as f64 - min) / (max - min)) as u16)
        .collect()
}

// Apply the Modality rescale operation to the input array and return a Vec<f64> containing transformed pixel data.
// An InvalidDataType error is returned when T cannot be represented as f64
pub fn apply_modality_lut<T>(
    i: &[T],
    rescale_intercept: i16,
    rescale_slope: f32,
) -> Result<Vec<f64>>
where
    T: NumCast + Sync,
{
    let result: Result<Vec<f64>, _> = i
        .par_iter()
        .map(|e| {
            (*e).to_f64()
                .map(|v| v * (rescale_slope as f64) + (rescale_intercept as f64))
                .ok_or(snafu::NoneError)
        })
        .collect();
    result.context(InvalidDataTypeSnafu)
}

pub trait PixelDecoder {
    /// Decode compressed pixel data.
    /// A new buffer (Vec<u8>) is created holding the decoded pixel data.
    fn decode_pixel_data(&self) -> Result<DecodedPixelData>;
}

#[cfg(not(feature = "gdcm"))]
impl<D> PixelDecoder for FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary + Clone,
{
    fn decode_pixel_data(&self) -> Result<DecodedPixelData> {
        let pixel_data = self
            .element(dicom_dictionary_std::tags::PIXEL_DATA)
            .context(MissingRequiredFieldSnafu)?;
        let cols = cols(self)?;
        let rows = rows(self)?;

        let photometric_interpretation = photometric_interpretation(self)?;
        let samples_per_pixel = samples_per_pixel(self)?;
        let bits_allocated = bits_allocated(self)?;
        let bits_stored = bits_stored(self)?;
        let high_bit = high_bit(self)?;
        let pixel_representation = pixel_representation(self)?;
        let rescale_intercept = rescale_intercept(self);
        let rescale_slope = rescale_slope(self);
        let number_of_frames = number_of_frames(self);

        let transfer_syntax = &self.meta().transfer_syntax;
        let registry = TransferSyntaxRegistry
            .get(transfer_syntax)
            .filter(|ts| ts.fully_supported())
            .context(UnsupportedTransferSyntax {
                ts: transfer_syntax,
            })?;

        // Try decoding it using a native Rust decoder
        if let Codec::PixelData(decoder) = registry.codec() {
            let mut data: Vec<u8> = Vec::new();
            (*decoder)
                .decode(self, &mut data)
                .context(PixelDecodeSnafu)?;

            return Ok(DecodedPixelData {
                data: Cow::from(data),
                cols: cols.into(),
                rows: rows.into(),
                number_of_frames,
                photometric_interpretation,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                high_bit,
                pixel_representation,
                rescale_intercept,
                rescale_slope,
            });
        }

        let decoded_pixel_data = match pixel_data.value() {
            Value::PixelSequence {
                fragments,
                offset_table: _,
            } => {
                // Return all fragments concatenated
                fragments.into_iter().flatten().copied().collect()
            }
            Value::Primitive(p) => {
                // Non-encoded, just return the pixel data for all frames
                p.to_bytes().to_vec()
            }
            Value::Sequence { items: _, size: _ } => InvalidPixelDataSnafu.fail()?,
        };

        Ok(DecodedPixelData {
            data: Cow::from(decoded_pixel_data),
            cols: cols.into(),
            rows: rows.into(),
            number_of_frames,
            photometric_interpretation,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            rescale_intercept,
            rescale_slope,
        })
    }
}

/// Get the Columns of the dicom
fn cols<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::COLUMNS)
        .context(MissingRequiredFieldSnafu)?
        .uint16()
        .context(CastValueSnafu)
}

/// Get the Rows of the dicom
fn rows<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::ROWS)
        .context(MissingRequiredFieldSnafu)?
        .uint16()
        .context(CastValueSnafu)
}

/// Get the PhotoMetricInterpretation of the Dicom
fn photometric_interpretation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<String> {
    Ok(obj
        .element(dicom_dictionary_std::tags::PHOTOMETRIC_INTERPRETATION)
        .context(MissingRequiredFieldSnafu)?
        .string()
        .context(CastValueSnafu)?
        .trim()
        .to_string())
}

/// Get the SamplesPerPixel of the Dicom
fn samples_per_pixel<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::SAMPLES_PER_PIXEL)
        .context(MissingRequiredFieldSnafu)?
        .uint16()
        .context(CastValueSnafu)
}

/// Get the BitsAllocated of the Dicom
fn bits_allocated<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_ALLOCATED)
        .context(MissingRequiredFieldSnafu)?
        .uint16()
        .context(CastValueSnafu)
}

/// Get the BitsStored of the Dicom
fn bits_stored<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_STORED)
        .context(MissingRequiredFieldSnafu)?
        .uint16()
        .context(CastValueSnafu)
}

/// Get the HighBit of the Dicom
fn high_bit<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::HIGH_BIT)
        .context(MissingRequiredFieldSnafu)?
        .uint16()
        .context(CastValueSnafu)
}

/// Get the PixelRepresentation of the Dicom
fn pixel_representation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::PIXEL_REPRESENTATION)
        .context(MissingRequiredFieldSnafu)?
        .uint16()
        .context(CastValueSnafu)
}

/// Get the RescaleIntercept of the Dicom or returns 0
fn rescale_intercept<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> i16 {
    obj.element(dicom_dictionary_std::tags::RESCALE_INTERCEPT)
        .map_or(Ok(0), |e| e.to_int())
        .unwrap_or(0)
}

/// Get the RescaleSlope of the Dicom or returns 1.0
fn rescale_slope<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> f32 {
    obj.element(dicom_dictionary_std::tags::RESCALE_SLOPE)
        .map_or(Ok(1.0), |e| e.to_float32())
        .unwrap_or(1.0)
}

/// Get the NumberOfFrames of the Dicom or returns 1
fn number_of_frames<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> u16 {
    obj.element(dicom_dictionary_std::tags::NUMBER_OF_FRAMES)
        .map_or(Ok(1), |e| e.to_int())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_object::open_file;
    use dicom_test_files;

    #[cfg(feature = "gdcm")]
    use rstest::rstest;
    #[cfg(feature = "gdcm")]
    #[rstest(value => [
         "pydicom/693_UNCI.dcm",
         "pydicom/693_UNCR.dcm",
         "pydicom/CT_small.dcm",
         "pydicom/MR_small.dcm",
         "pydicom/MR_small_implicit.dcm",
         "pydicom/MR_small_bigendian.dcm",
         "pydicom/MR_small_expb.dcm",
         "pydicom/SC_rgb.dcm",
         "pydicom/SC_rgb_16bit.dcm",
         "pydicom/SC_rgb_expb.dcm", 
         "pydicom/SC_rgb_expb_16bit.dcm",
         "pydicom/color-pl.dcm",
         "pydicom/color-px.dcm",
         "pydicom/SC_ybr_full_uncompressed.dcm",
    ])]
    fn test_parse_dicom_pixel_data(value: &str) {
        use std::path::Path;
        let test_file = dicom_test_files::path(value).unwrap();
        println!("Parsing pixel data for {:?}", test_file);
        let obj = open_file(test_file).unwrap();
        let image = obj
            .decode_pixel_data()
            .unwrap()
            .to_dynamic_image(0)
            .unwrap();
        image
            .save(format!(
                "../target/dicom_test_files/pydicom/{}.png",
                Path::new(value).file_stem().unwrap().to_str().unwrap()
            ))
            .unwrap();
    }

    #[test]
    fn test_to_ndarray_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_16bit.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let ndarray = obj
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<u16>()
            .unwrap();
        assert_eq!(ndarray.shape(), &[1, 100, 100, 3]);
        assert_eq!(ndarray.len(), 30000);
        assert_eq!(ndarray[[0, 50, 80, 1]], 32896);
    }

    #[test]
    fn test_to_ndarray_error() {
        let test_file = dicom_test_files::path("pydicom/JPEG2000.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        assert!(matches!(
            obj.decode_pixel_data().unwrap().to_ndarray::<u8>(),
            Err(Error::InvalidDataType)
        ));
    }

    #[test]
    fn test_correct_ri_extracted() {
        // RescaleIntercept exists for this scan
        let test_file = dicom_test_files::path("pydicom/693_J2KR.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(pixel_data.rescale_intercept, -1024);
    }

    #[test]
    fn test_correct_ri_extracted_without_element() {
        // RescaleIntercept does not exists for this scan
        let test_file = dicom_test_files::path("pydicom/MR_small_jpeg_ls_lossless.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(pixel_data.rescale_intercept, 0);
    }

    #[test]
    fn test_correct_rs_extracted() {
        // RescaleIntercept exists for this scan
        let test_file = dicom_test_files::path("pydicom/MR_small_jpeg_ls_lossless.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(pixel_data.rescale_slope, 1.0);
    }

    #[cfg(not(feature = "gdcm"))]
    #[test]
    fn test_native_decoding_pixel_data_rle_8bit_1frame() {
        let path =
            dicom_test_files::path("pydicom/SC_rgb_rle.dcm").expect("test DICOM file should exist");
        let object = open_file(&path).unwrap();
        let ndarray = object
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<u8>()
            .unwrap();
        // Validated using Numpy
        // This doesn't reshape the array based on the PlanarConfiguration
        // So for this scan the pixel layout is [Rlsb..Rmsb, Glsb..Gmsb, Blsb..msb]
        assert_eq!(ndarray.shape(), &[1, 100, 100, 3]);
        assert_eq!(ndarray.len(), 30000);
        assert_eq!(ndarray[[0, 0, 0, 0]], 255);
        assert_eq!(ndarray[[0, 0, 0, 1]], 255);
        assert_eq!(ndarray[[0, 0, 0, 2]], 255);
        assert_eq!(ndarray[[0, 50, 50, 0]], 128);
        assert_eq!(ndarray[[0, 50, 50, 1]], 128);
        assert_eq!(ndarray[[0, 50, 50, 2]], 128);
        assert_eq!(ndarray[[0, 75, 75, 0]], 0);
        assert_eq!(ndarray[[0, 75, 75, 1]], 0);
        assert_eq!(ndarray[[0, 75, 75, 2]], 0);
    }

    #[cfg(not(feature = "gdcm"))]
    #[test]
    fn test_native_decoding_pixel_data_rle_8bit_2frame() {
        let path = dicom_test_files::path("pydicom/SC_rgb_rle_2frame.dcm")
            .expect("test DICOM file should exist");
        let object = open_file(&path).unwrap();
        let ndarray = object
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<u8>()
            .unwrap();
        // Validated using Numpy
        // This doesn't reshape the array based on the PlanarConfiguration
        // So for this scan the pixel layout is [Rlsb..Rmsb, Glsb..Gmsb, Blsb..msb]
        assert_eq!(ndarray.shape(), &[2, 100, 100, 3]);
        assert_eq!(ndarray.len(), 60000);
        // The second frame is the inverse of the first frame
        assert_eq!(ndarray[[1, 0, 0, 0]], 0);
        assert_eq!(ndarray[[1, 0, 0, 1]], 0);
        assert_eq!(ndarray[[1, 0, 0, 2]], 0);
        assert_eq!(ndarray[[1, 50, 50, 0]], 127);
        assert_eq!(ndarray[[1, 50, 50, 1]], 127);
        assert_eq!(ndarray[[1, 50, 50, 2]], 127);
        assert_eq!(ndarray[[1, 75, 75, 0]], 255);
        assert_eq!(ndarray[[1, 75, 75, 1]], 255);
        assert_eq!(ndarray[[1, 75, 75, 2]], 255);
    }

    #[cfg(not(feature = "gdcm"))]
    #[test]
    fn test_native_decoding_pixel_data_rle_16bit_1frame() {
        let path = dicom_test_files::path("pydicom/SC_rgb_rle_16bit.dcm")
            .expect("test DICOM file should exist");
        let object = open_file(&path).unwrap();
        let ndarray = object
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<u16>()
            .unwrap();
        // Validated using Numpy
        // This doesn't reshape the array based on the PlanarConfiguration
        // So for this scan the pixel layout is [Rlsb..Rmsb, Glsb..Gmsb, Blsb..msb]
        assert_eq!(ndarray.shape(), &[1, 100, 100, 3]);
        assert_eq!(ndarray.len(), 30000);
        assert_eq!(ndarray[[0, 0, 0, 0]], 65535);
        assert_eq!(ndarray[[0, 0, 0, 1]], 65535);
        assert_eq!(ndarray[[0, 0, 0, 2]], 65535);
        assert_eq!(ndarray[[0, 50, 50, 0]], 32896);
        assert_eq!(ndarray[[0, 50, 50, 1]], 32896);
        assert_eq!(ndarray[[0, 50, 50, 2]], 32896);
        assert_eq!(ndarray[[0, 75, 75, 0]], 0);
        assert_eq!(ndarray[[0, 75, 75, 1]], 0);
        assert_eq!(ndarray[[0, 75, 75, 2]], 0);
    }

    #[cfg(not(feature = "gdcm"))]
    #[test]
    fn test_native_decoding_pixel_data_rle_16bit_2frame() {
        let path = dicom_test_files::path("pydicom/SC_rgb_rle_16bit_2frame.dcm")
            .expect("test DICOM file should exist");
        let object = open_file(&path).unwrap();
        let ndarray = object
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<u16>()
            .unwrap();
        // Validated using Numpy
        // This doesn't reshape the array based on the PlanarConfiguration
        // So for this scan the pixel layout is [Rlsb..Rmsb, Glsb..Gmsb, Blsb..msb]
        assert_eq!(ndarray.shape(), &[2, 100, 100, 3]);
        assert_eq!(ndarray.len(), 60000);
        // The second frame is the inverse of the first frame
        assert_eq!(ndarray[[1, 0, 0, 0]], 0);
        assert_eq!(ndarray[[1, 0, 0, 1]], 0);
        assert_eq!(ndarray[[1, 0, 0, 2]], 0);
        assert_eq!(ndarray[[1, 50, 50, 0]], 32639);
        assert_eq!(ndarray[[1, 50, 50, 1]], 32639);
        assert_eq!(ndarray[[1, 50, 50, 2]], 32639);
        assert_eq!(ndarray[[1, 75, 75, 0]], 65535);
        assert_eq!(ndarray[[1, 75, 75, 1]], 65535);
        assert_eq!(ndarray[[1, 75, 75, 2]], 65535);
    }

    #[test]
    fn test_frame_out_of_range() {
        let path =
            dicom_test_files::path("pydicom/CT_small.dcm").expect("test DICOM file should exist");
        let image = open_file(&path).unwrap();
        // Only one frame in this test dicom
        image
            .decode_pixel_data()
            .unwrap()
            .to_dynamic_image(0)
            .unwrap();
        let result = image.decode_pixel_data().unwrap().to_dynamic_image(1);
        match result {
            Err(Error::FrameOutOfRangeError) => {}
            _ => panic!("Cannot index frame that is out of range"),
        }
    }
}
