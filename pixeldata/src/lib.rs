//! This crate contains the Dicom pixeldata handlers and is
//! responsible for decoding pixeldata, such as JPEG-lossy and convert it
//! into a [`DynamicImage`], [`Array`] or raw [`DecodedPixelData`].
//!
//! This crate is using GDCM bindings to convert
//! different compression formats to raw pixeldata.
//! This should become a pure Rust implementation in the future.
//!
//! # Examples
//!
//! Example using `to_dynamic_image`
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::PixelDecoder;
//!
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("dicom.dcm")?;
//! let image = obj.decode_pixel_data()?;
//! let dynamic_image = image.to_dynamic_image()?;
//! dynamic_image.save("out.png")?;
//! #   Ok(())
//! # }
//! ```
//!
//! Example using `to_ndarray`
//! ```no_run
//! use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::{PixelDecoder};
//! use ndarray::s;

//! fn main() -> Result<(), Box<dyn Error>> {
//!     let obj = open_file("rgb_dicom.dcm")?;
//!     let pixel_data = obj.decode_pixel_data()?;
//!     let ndarray = pixel_data.to_ndarray::<u16>()?;
//!     let red_values = ndarray.slice(s![.., .., 0]);
//!     Ok(())
//! }
//! ```

use byteorder::{ByteOrder, NativeEndian};
use dicom_core::{value::Value, DataDictionary};
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_object::{FileDicomObject, InMemDicomObject};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use gdcm_rs::{decode_single_frame_compressed, GDCMPhotometricInterpretation, GDCMTransferSyntax};
use image::{DynamicImage, ImageBuffer, Luma, Rgb};
use ndarray::{Array, IxDyn};
use ndarray_stats::QuantileExt;
use num_traits::NumCast;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use snafu::OptionExt;
use snafu::{ResultExt, Snafu};
use std::{borrow::Cow, str::FromStr};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Missing required element"))]
    MissingRequiredField { source: dicom_object::Error },

    #[snafu(display("Could not cast element"))]
    CastValueError {
        source: dicom_core::value::CastValueError,
    },

    #[snafu(display("Non supported GDCM PhotometricInterpretation: {}", pi))]
    GdcmNonSupportedPi {
        source: gdcm_rs::InvalidGDCMPI,
        pi: String,
    },

    #[snafu(display("Non supported GDCM TransferSyntax: {}", ts))]
    GdcmNonSupportedTs {
        source: gdcm_rs::InvalidGDCMTS,
        ts: String,
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

    #[snafu(display("Unknown GDCM error while decoding image"))]
    UnknownGdcmError { source: gdcm_rs::Error },

    #[snafu(display("Invalid shape for ndarray"))]
    ShapeError { source: ndarray::ShapeError },

    #[snafu(display("Invalid data type for ndarray element"))]
    InvalidDataType,

    #[snafu(display("Unsupported color space"))]
    UnsupportedColorSpace,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Decoded pixel data
pub struct DecodedPixelData<'a> {
    pub data: Cow<'a, [u8]>,
    pub rows: u32,
    pub cols: u32,
    pub photometric_interpretation: String,
    pub samples_per_pixel: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
    pub rescale_intercept: i16,
    pub rescale_slope: f32,
}

impl DecodedPixelData<'_> {
    /// Convert decoded pixel data into a DynamicImage.
    /// A new <u8> or <u16> vector is created in memory
    /// with normalized grayscale values after applying Modality LUT.
    pub fn to_dynamic_image(&self) -> Result<DynamicImage> {
        match self.samples_per_pixel {
            1 => {
                let mut pixel_array = self.to_ndarray::<f64>()?;
                // Only Monochrome images can have a Modality LUT
                pixel_array.mapv_inplace(|v| {
                    (v * self.rescale_slope as f64) + self.rescale_intercept as f64
                });

                // TODO(#122): Apply VOI LUT

                // Normalize to u16
                let min = pixel_array.min().unwrap();
                let max = pixel_array.max().unwrap();
                let mut pixel_array =
                    pixel_array.map(|v| (u16::MAX as f64 * (v - *min) / (*max - *min)) as u16);

                // Convert MONOCHROME1 to MONOCHROME2 if needed
                if self.photometric_interpretation == "MONOCHROME1" {
                    pixel_array.mapv_inplace(|v| u16::MAX - v);
                }

                let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                    ImageBuffer::from_raw(self.cols, self.rows, pixel_array.into_raw_vec())
                        .context(InvalidImageBuffer)?;
                Ok(DynamicImage::ImageLuma16(image_buffer))
            }
            3 => {
                // RGB, YBR_FULL or YBR_FULL_422 colors
                match self.bits_allocated {
                    8 => {
                        // Convert YBR_FULL or YBR_FULL_422 to RGB
                        let pixel_array = match self.photometric_interpretation.as_str() {
                            "RGB" => self.data.to_vec(),
                            "YBR_FULL" | "YBR_FULL_422" => {
                                let mut pixel_array = self.data.to_vec();
                                convert_colorspace_u8(&mut pixel_array);
                                pixel_array
                            }
                            _ => UnsupportedColorSpace.fail()?,
                        };
                        let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
                            ImageBuffer::from_raw(self.cols, self.rows, pixel_array)
                                .context(InvalidImageBuffer)?;
                        Ok(DynamicImage::ImageRgb8(image_buffer))
                    }
                    16 => {
                        let mut pixel_array = vec![0; self.data.len() / 2];
                        NativeEndian::read_u16_into(&self.data, &mut pixel_array);

                        // Convert YBR_FULL or YBR_FULL_422 to RGB
                        let pixel_array = match self.photometric_interpretation.as_str() {
                            "RGB" => pixel_array,
                            "YBR_FULL" | "YBR_FULL_422" => {
                                convert_colorspace_u16(&mut pixel_array);
                                pixel_array
                            }
                            _ => UnsupportedColorSpace.fail()?,
                        };
                        let image_buffer: ImageBuffer<Rgb<u16>, Vec<u16>> =
                            ImageBuffer::from_raw(self.cols, self.rows, pixel_array)
                                .context(InvalidImageBuffer)?;
                        Ok(DynamicImage::ImageRgb16(image_buffer))
                    }
                    _ => InvalidBitsAllocated.fail()?,
                }
            }
            _ => InvalidPixelRepresentation.fail()?,
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
        // Array size is Rows x Cols x SamplesPerPixel (1 for grayscale, 3 for RGB)
        let shape = IxDyn(&[
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
                let converted = converted.context(InvalidDataType)?;
                let ndarray = Array::from_shape_vec(shape, converted).context(ShapeError)?;
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
                    let converted = converted.context(InvalidDataType)?;
                    let ndarray = Array::from_shape_vec(shape, converted).context(ShapeError)?;
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
                    let converted = converted.context(InvalidDataType)?;
                    let ndarray = Array::from_shape_vec(shape, converted).context(ShapeError)?;
                    Ok(ndarray)
                }
                _ => InvalidPixelRepresentation.fail()?,
            },
            _ => InvalidBitsAllocated.fail()?,
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
    result.context(InvalidDataType)
}

pub trait PixelDecoder {
    /// Decode compressed pixel data.
    /// A new buffer (Vec<u8>) is created holding the decoded pixel data.
    fn decode_pixel_data(&self) -> Result<DecodedPixelData>;
}

impl<D> PixelDecoder for FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary + Clone,
{
    fn decode_pixel_data(&self) -> Result<DecodedPixelData> {
        let pixel_data = self
            .element(dicom_dictionary_std::tags::PIXEL_DATA)
            .context(MissingRequiredField)?;
        let cols = cols(self)?;
        let rows = rows(self)?;

        let photometric_interpretation = photometric_interpretation(self)?;
        let pi_type = GDCMPhotometricInterpretation::from_str(&photometric_interpretation)
            .context(GdcmNonSupportedPi {
                pi: &photometric_interpretation,
            })?;

        let transfer_syntax = &self.meta().transfer_syntax;
        let registry =
            TransferSyntaxRegistry
                .get(&&transfer_syntax)
                .context(UnsupportedTransferSyntax {
                    ts: transfer_syntax,
                })?;
        let ts_type =
            GDCMTransferSyntax::from_str(&registry.uid()).context(GdcmNonSupportedTs {
                ts: transfer_syntax,
            })?;

        let samples_per_pixel = samples_per_pixel(self)?;
        let bits_allocated = bits_allocated(self)?;
        let bits_stored = bits_stored(self)?;
        let high_bit = high_bit(self)?;
        let pixel_representation = pixel_representation(self)?;
        let rescale_intercept = rescale_intercept(self);
        let rescale_slope = rescale_slope(self);

        let decoded_pixel_data = match pixel_data.value() {
            Value::PixelSequence {
                fragments,
                offset_table: _,
            } => {
                if fragments.len() > 1 {
                    // Bundle fragments and decode multi-frame dicoms
                    UnsupportedMultiFrame.fail()?
                }
                let decoded_frame = decode_single_frame_compressed(
                    &fragments[0],
                    cols.into(),
                    rows.into(),
                    pi_type,
                    ts_type,
                    samples_per_pixel,
                    bits_allocated,
                    bits_stored,
                    high_bit,
                    pixel_representation,
                )
                .context(UnknownGdcmError)?;
                decoded_frame.to_vec()
            }
            Value::Primitive(p) => {
                // Non-encoded, just return the pixel data
                p.to_bytes().to_vec()
            }
            Value::Sequence { items: _, size: _ } => InvalidPixelData.fail()?,
        };

        Ok(DecodedPixelData {
            data: Cow::from(decoded_pixel_data),
            cols: cols.into(),
            rows: rows.into(),
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
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the Rows of the dicom
fn rows<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::ROWS)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the PhotoMetricInterpretation of the Dicom
fn photometric_interpretation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<String> {
    Ok(obj
        .element(dicom_dictionary_std::tags::PHOTOMETRIC_INTERPRETATION)
        .context(MissingRequiredField)?
        .string()
        .context(CastValueError)?
        .trim()
        .to_string())
}

/// Get the SamplesPerPixel of the Dicom
fn samples_per_pixel<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::SAMPLES_PER_PIXEL)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the BitsAllocated of the Dicom
fn bits_allocated<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_ALLOCATED)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the BitsStored of the Dicom
fn bits_stored<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_STORED)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the HighBit of the Dicom
fn high_bit<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::HIGH_BIT)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the PixelRepresentation of the Dicom
fn pixel_representation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::PIXEL_REPRESENTATION)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
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

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_object::open_file;
    use dicom_test_files;
    use rstest::rstest;
    use std::path::Path;

    #[rstest(value => [
         "pydicom/693_J2KI.dcm",
         "pydicom/693_J2KR.dcm",
         "pydicom/693_UNCI.dcm",
         "pydicom/693_UNCR.dcm",
         "pydicom/CT_small.dcm",
         "pydicom/JPEG-lossy.dcm",
         "pydicom/JPEG2000.dcm",
         "pydicom/JPEG2000_UNC.dcm",
         "pydicom/JPGLosslessP14SV1_1s_1f_8b.dcm",
         "pydicom/MR_small.dcm",
         "pydicom/MR_small_RLE.dcm",
         "pydicom/MR_small_implicit.dcm",
         "pydicom/MR_small_jp2klossless.dcm",
         "pydicom/MR_small_jpeg_ls_lossless.dcm",
         "pydicom/explicit_VR-UN.dcm",
         "pydicom/MR_small_bigendian.dcm",
         "pydicom/MR_small_expb.dcm",
         "pydicom/SC_rgb.dcm",
         "pydicom/SC_rgb_16bit.dcm",
         "pydicom/SC_rgb_dcmtk_+eb+cr.dcm",
         "pydicom/SC_rgb_expb.dcm", 
         "pydicom/SC_rgb_expb_16bit.dcm",
         "pydicom/SC_rgb_gdcm2k_uncompressed.dcm",
         "pydicom/SC_rgb_gdcm_KY.dcm",
         "pydicom/SC_rgb_jpeg_gdcm.dcm",
         "pydicom/SC_rgb_jpeg_lossy_gdcm.dcm",
         "pydicom/SC_rgb_rle.dcm",
         "pydicom/SC_rgb_rle_16bit.dcm",
         "pydicom/color-pl.dcm",
         "pydicom/color-px.dcm",
         "pydicom/SC_ybr_full_uncompressed.dcm",

        // "pydicom/RG1_J2KI.dcm",
        // "pydicom/RG1_J2KR.dcm",
        // "pydicom/RG1_UNCI.dcm",
        // "pydicom/RG1_UNCR.dcm",
        // "pydicom/RG3_J2KI.dcm",
        // "pydicom/RG3_J2KR.dcm",
        // "pydicom/RG3_UNCI.dcm",
        // "pydicom/RG3_UNCR.dcm",
        // "pydicom/ExplVR_BigEnd.dcm",
        // "pydicom/ExplVR_BigEndNoMeta.dcm",
        // "pydicom/ExplVR_LitEndNoMeta.dcm",
        // "pydicom/JPEG-LL.dcm",                       // More than 1 fragment
        // "pydicom/MR-SIEMENS-DICOM-WithOverlays.dcm", // Overlays not supported
        // "pydicom/MR2_J2KI.dcm",  // Multi-frame
        // "pydicom/MR2_J2KR.dcm",
        // "pydicom/MR2_UNCI.dcm",
        // "pydicom/MR2_UNCR.dcm",
        // "pydicom/MR_small_padded.dcm",
        // "pydicom/MR_truncated.dcm",
        // "pydicom/OBXXXX1A.dcm",
        // "pydicom/OBXXXX1A_2frame.dcm",
        // "pydicom/OBXXXX1A_expb.dcm",
        // "pydicom/OBXXXX1A_expb_2frame.dcm",
        // "pydicom/OBXXXX1A_rle.dcm",
        // "pydicom/OBXXXX1A_rle_2frame.dcm",
        // "pydicom/OT-PAL-8-face.dcm",
        // "pydicom/SC_rgb_16bit_2frame.dcm",
        // "pydicom/SC_rgb_2frame.dcm",
        // "pydicom/SC_rgb_32bit.dcm",
        // "pydicom/SC_rgb_32bit_2frame.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+n1.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+n2.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+np.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+s2.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+s4.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcr_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcyn1_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcyn2_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcynp_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcys2_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcys4_dcmd.dcm",
        // "pydicom/SC_rgb_expb_16bit_2frame.dcm",
        // "pydicom/SC_rgb_expb_2frame.dcm",
        // "pydicom/SC_rgb_expb_32bit.dcm",
        // "pydicom/SC_rgb_expb_32bit_2frame.dcm",
        // "pydicom/SC_rgb_rle_16bit_2frame.dcm",
        // "pydicom/SC_rgb_rle_2frame.dcm",
        // "pydicom/SC_rgb_rle_32bit.dcm",
        // "pydicom/SC_rgb_rle_32bit_2frame.dcm",
        // "pydicom/SC_rgb_small_odd.dcm",
        // "pydicom/SC_rgb_small_odd_jpeg.dcm",
        // "pydicom/SC_rgb_jpeg_dcmtk.dcm",
        // "pydicom/SC_ybr_full_422_uncompressed.dcm",
        // "pydicom/US1_J2KI.dcm",
        // "pydicom/US1_J2KR.dcm",
        // "pydicom/US1_UNCI.dcm",
        // "pydicom/US1_UNCR.dcm",
        // "pydicom/badVR.dcm",
        // "pydicom/bad_sequence.dcm",
        // "pydicom/color3d_jpeg_baseline.dcm",
        // "pydicom/eCT_Supplemental.dcm",
        // "pydicom/empty_charset_LEI.dcm",
        // "pydicom/emri_small.dcm",
        // "pydicom/emri_small_RLE.dcm",
        // "pydicom/emri_small_big_endian.dcm",
        // "pydicom/emri_small_jpeg_2k_lossless.dcm",
        // "pydicom/emri_small_jpeg_2k_lossless_too_short.dcm",
        // "pydicom/emri_small_jpeg_ls_lossless.dcm",
        // "pydicom/gdcm-US-ALOKA-16.dcm",
        // "pydicom/gdcm-US-ALOKA-16_big.dcm",
        // "pydicom/image_dfl.dcm",
        // "pydicom/liver.dcm",
        // "pydicom/liver_1frame.dcm",
        // "pydicom/liver_expb.dcm",
        // "pydicom/liver_expb_1frame.dcm",
        // "pydicom/meta_missing_tsyntax.dcm",
        // "pydicom/mlut_18.dcm",
        // "pydicom/nested_priv_SQ.dcm",
        // "pydicom/no_meta.dcm",
        // "pydicom/no_meta_group_length.dcm",
        // "pydicom/priv_SQ.dcm",
        // "pydicom/reportsi.dcm",
        // "pydicom/reportsi_with_empty_number_tags.dcm",
        // "pydicom/rtdose.dcm",
        // "pydicom/rtdose_1frame.dcm",
        // "pydicom/rtdose_expb.dcm",
        // "pydicom/rtdose_expb_1frame.dcm",
        // "pydicom/rtdose_rle.dcm",
        // "pydicom/rtdose_rle_1frame.dcm",
        // "pydicom/rtplan.dcm",
        // "pydicom/rtplan_truncated.dcm",
        // "pydicom/rtstruct.dcm",
        // "pydicom/test-SR.dcm",
        // "pydicom/vlut_04.dcm",
    ])]
    fn test_parse_dicom_pixel_data(value: &str) {
        let test_file = dicom_test_files::path(value).unwrap();
        println!("Parsing pixel data for {:?}", test_file);
        let obj = open_file(test_file).unwrap();
        let image = obj.decode_pixel_data().unwrap().to_dynamic_image().unwrap();
        image
            .save(format!(
                "../target/dicom_test_files/pydicom/{}.png",
                Path::new(value).file_stem().unwrap().to_str().unwrap()
            ))
            .unwrap();
    }

    #[test]
    fn test_to_ndarray_signed_word() {
        let test_file = dicom_test_files::path("pydicom/JPEG2000.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let ndarray = obj
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<i16>()
            .unwrap();
        assert_eq!(ndarray.shape(), &[1024, 256, 1]);
        assert_eq!(ndarray.len(), 262144);
        assert_eq!(ndarray[[260, 0, 0]], -3);
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
        assert_eq!(ndarray.shape(), &[100, 100, 3]);
        assert_eq!(ndarray.len(), 30000);
        assert_eq!(ndarray[[50, 80, 1]], 32896);
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
}
