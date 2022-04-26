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
//! To convert a DICOM object into a dynamic image:
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
//! To convert a DICOM object into an ndarray:
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::PixelDecoder;
//! use ndarray::s;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("rgb_dicom.dcm")?;
//! let pixel_data = obj.decode_pixel_data()?;
//! let ndarray = pixel_data.to_ndarray::<u16>()?;
//! let red_values = ndarray.slice(s![.., .., .., 0]);
//! # Ok(())
//! # }
//! ```

use byteorder::{ByteOrder, NativeEndian};
use dicom_core::{value::Value, DataDictionary};
use dicom_encoding::adapters::DecodeError;
#[cfg(not(feature = "gdcm"))]
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
#[cfg(not(feature = "gdcm"))]
use dicom_encoding::Codec;
use dicom_object::{FileDicomObject, InMemDicomObject};
#[cfg(not(feature = "gdcm"))]
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use image::{DynamicImage, ImageBuffer, Luma, Rgb};
use ndarray::{Array, Ix3, Ix4};
use num_traits::NumCast;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use snafu::OptionExt;
use snafu::{Backtrace, ResultExt, Snafu};
use std::borrow::Cow;

pub use image;
pub use ndarray;

mod attribute;
pub mod lut;

pub(crate) mod transform;

// re-exports
pub use attribute::{PhotometricInterpretation, PixelRepresentation, PlanarConfiguration};
pub use lut::{CreateLutError, Lut};
pub use transform::{Rescale, VoiLutFunction, WindowLevel, WindowLevelTransform};

#[cfg(feature = "gdcm")]
mod gdcm;

/// Error type for most pixel data related operations.
#[derive(Debug, Snafu)]
pub struct Error(InnerError);

/// Inner error type
#[derive(Debug, Snafu)]
pub enum InnerError {
    #[snafu(display("Failed to get required DICOM attribute"))]
    GetAttribute {
        #[snafu(backtrace)]
        source: attribute::GetAttributeError,
    },

    #[snafu(display("PixelData attribute is not a primitive value or pixel sequence"))]
    InvalidPixelData { backtrace: Backtrace },

    #[snafu(display("Invalid BitsAllocated, must be 8 or 16"))]
    InvalidBitsAllocated { backtrace: Backtrace },

    #[snafu(display("Unsupported PhotometricInterpretation `{}`", pi))]
    UnsupportedPhotometricInterpretation {
        pi: PhotometricInterpretation,
        backtrace: Backtrace,
    },

    #[snafu(display("Unsupported SamplesPerPixel `{}`", spp))]
    UnsupportedSamplesPerPixel { spp: u16, backtrace: Backtrace },

    #[snafu(display("Unsupported {} `{}`", name, value))]
    UnsupportedOther {
        name: &'static str,
        value: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Unknown transfer syntax `{}`", ts_uid))]
    UnknownTransferSyntax {
        ts_uid: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Unsupported TransferSyntax `{}`", ts))]
    UnsupportedTransferSyntax { ts: String, backtrace: Backtrace },

    #[snafu(display("Multi-frame DICOM images are not supported"))]
    UnsupportedMultiFrame { backtrace: Backtrace },

    #[snafu(display("Invalid buffer when constructing ImageBuffer"))]
    InvalidImageBuffer { backtrace: Backtrace },

    #[snafu(display("Invalid shape for ndarray"))]
    InvalidShape {
        source: ndarray::ShapeError,
        backtrace: Backtrace,
    },

    /// Could not create LUT for target data type
    CreateLut {
        source: lut::CreateLutError,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid data type for ndarray element"))]
    InvalidDataType { backtrace: Backtrace },

    #[snafu(display("Unsupported color space"))]
    UnsupportedColorSpace { backtrace: Backtrace },

    #[snafu(display("Could not decode pixel data"))]
    DecodePixelData { source: DecodeError },

    #[snafu(display("Frame #{} is out of range", frame_number))]
    FrameOutOfRange {
        frame_number: u32,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Option set for converting decoded pixel data
/// into other common data structures,
/// such as a vector, an image, or a multidimensional array.
#[derive(Debug, Default, Clone, PartialEq)]
#[non_exhaustive]
pub struct ConvertOptions {
    modality_lut: ModalityLutOption,
    voi_lut: VoiLutOption,
}

impl ConvertOptions {
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the modality LUT option.
    pub fn with_modality_lut(mut self, modality_lut: ModalityLutOption) -> Self {
        self.modality_lut = modality_lut;
        self
    }

    /// Set the VOI LUT option.
    pub fn with_voi_lut(mut self, voi_lut: VoiLutOption) -> Self {
        self.voi_lut = voi_lut;
        self
    }
}

/// Modality LUT function specifier.
///
/// See also [`ConvertOptions`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ModalityLutOption {
    /// _Default behavior:_
    /// rescale the pixel data values
    /// as described in the decoded pixel data.
    Default,
    /// Rescale the pixel data values
    /// according to the given rescale parameters
    Override(Rescale),
    /// Do not rescale nor transform the pixel data value samples.
    ///
    /// This also overrides any option to apply VOI LUT transformations
    /// in the decoded pixel data conversion methods.
    /// To assume the identity function for rescaling
    /// and apply the VOI LUT transformations as normal,
    /// use the `Override` variant instead.
    None,
}

impl Default for ModalityLutOption {
    fn default() -> Self {
        ModalityLutOption::Default
    }
}

impl ModalityLutOption {}

/// VOI LUT function specifier.
///
/// Note that the VOI LUT function is only applied
/// alongside a modality LUT function.
///
/// See also [`ConvertOptions`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum VoiLutOption {
    /// _Default behavior:_
    /// apply the first VOI LUT function transformation described in the pixel data
    /// only when converting to an image;
    /// no VOI LUT function is performed
    /// when converting to an ndarray or to bare pixel values.
    Default,
    /// Apply the first VOI LUT function transformation
    /// described in the pixel data.
    First,
    /// Apply a custom window level instead of the one described in the object.
    Custom(WindowLevel),
    /// Perform a min-max normalization instead,
    /// so that the lowest value is 0 and
    /// the highest value is the maximum value of the target type.
    Normalize,
    /// Do not apply any VOI LUT transformation.
    Identity,
}

impl Default for VoiLutOption {
    fn default() -> Self {
        VoiLutOption::Default
    }
}

/// A blob of decoded pixel data.
///
/// This is the outcome of collecting a DICOM object's imaging-related attributes
/// into a decoded form.
/// The decoded pixel data samples will be stored as raw bytes in native form
/// without any LUT transformations applied.
/// Whether to apply such transformations
/// can be specified through one of the various `to_*` methods,
/// such as [`to_dynamic_image`](Self::to_dynamic_image)
/// and [`to_vec`](Self::to_vec).
#[derive(Debug)]
pub struct DecodedPixelData<'a> {
    /// the raw bytes of pixel data
    data: Cow<'a, [u8]>,
    /// the number of rows
    rows: u32,
    /// the number of columns
    cols: u32,
    /// the number of frames
    number_of_frames: u32,
    /// the photometric interpretation
    photometric_interpretation: PhotometricInterpretation,
    /// the number of samples per pixel
    samples_per_pixel: u16,
    /// the planar configuration: 0 for standard, 1 for channel-contiguous
    planar_configuration: PlanarConfiguration,
    /// the number of bits allocated, as a multiple of 8
    bits_allocated: u16,
    /// the number of bits stored
    bits_stored: u16,
    /// the high bit, usually `bits_stored - 1`
    high_bit: u16,
    /// the pixel representation: 0 for unsigned, 1 for signed
    pixel_representation: PixelRepresentation,
    // Enhanced MR Images are not yet supported having
    // a RescaleSlope/RescaleIntercept Per-Frame Functional Group
    /// the pixel value rescale intercept
    rescale_intercept: f64,
    /// the pixel value rescale slope
    rescale_slope: f64,
    // the VOI LUT function
    voi_lut_function: Option<VoiLutFunction>,
    /// the window level specified via width and center
    window: Option<WindowLevel>,
    // TODO(#232): VOI LUT sequence is currently not supported
}

impl DecodedPixelData<'_> {
    // getter methods

    /// Retrieve a slice of all raw pixel data samples as bytes,
    /// irrespective of the expected size of each sample.
    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Retrieve a slice of a frame's raw pixel data samples as bytes,
    /// irrespective of the expected size of each sample.
    pub fn frame_data(&self, frame: u32) -> Result<&[u8]> {
        let bytes_per_sample = self.bits_allocated as usize / 8;
        let frame_length = self.rows as usize
            * self.cols as usize
            * self.samples_per_pixel as usize
            * bytes_per_sample;
        let frame_start = frame_length * frame as usize;
        let frame_end = frame_start + frame_length;
        if frame_end > (*self.data).len() {
            FrameOutOfRangeSnafu {
                frame_number: frame,
            }
            .fail()?
        }

        Ok(&self.data[(frame_start as usize..frame_end as usize)])
    }

    /// Retrieve a copy of a frame's raw pixel data samples
    /// as unsigned 16-bit integers.
    ///
    /// This is useful for retrieving pixel data
    /// with the _OW_ value representation.
    pub fn frame_data_ow(&self, frame: u32) -> Result<Vec<u16>> {
        let data = self.frame_data(frame)?;

        let mut pixel_array: Vec<u16> = vec![0; data.len() / 2];
        NativeEndian::read_u16_into(data, &mut pixel_array);
        Ok(pixel_array)
    }

    /// Retrieves the number of rows of the pixel data.
    #[inline]
    pub fn rows(&self) -> u32 {
        self.rows
    }

    /// Retrieves the number of columns of the pixel data.
    #[inline]
    pub fn columns(&self) -> u32 {
        self.cols
    }

    /// Retrieves the photometric interpretation.
    #[inline]
    pub fn photometric_interpretation(&self) -> &PhotometricInterpretation {
        &self.photometric_interpretation
    }

    /// Retrieves the planar configuration of the pixel data.
    ///
    /// The value returned is only meaningful for
    /// images with more than 1 sample per pixel.
    #[inline]
    pub fn planar_configuration(&self) -> PlanarConfiguration {
        self.planar_configuration
    }

    /// Retrieves the total number of frames
    /// in this piece of decoded pixel data.
    #[inline]
    pub fn number_of_frames(&self) -> u32 {
        self.number_of_frames
    }

    /// Retrieves the number of samples per pixel.
    #[inline]
    pub fn samples_per_pixel(&self) -> u32 {
        self.cols
    }

    /// Retrieve the number of bits effectively used for each sample.
    #[inline]
    pub fn bits_stored(&self) -> u16 {
        self.bits_stored
    }

    /// Retrieve the number of bits allocated for each sample.
    #[inline]
    pub fn bits_allocated(&self) -> u16 {
        self.bits_allocated
    }

    /// Retrieve the high bit index of each sample.
    #[inline]
    pub fn high_bit(&self) -> u16 {
        self.high_bit
    }

    /// Retrieve the pixel representation.
    #[inline]
    pub fn pixel_representation(&self) -> PixelRepresentation {
        self.pixel_representation
    }

    /// Retrieve object's rescale parameters.
    #[inline]
    pub fn rescale(&self) -> Rescale {
        Rescale {
            intercept: self.rescale_intercept,
            slope: self.rescale_slope,
        }
    }

    /// Retrieve the VOI LUT function defined by the object, if any.
    #[inline]
    pub fn voi_lut_function(&self) -> Option<VoiLutFunction> {
        self.voi_lut_function
    }

    // converter methods

    /// Convert the decoded pixel data of a specific frame into a dynamic image.
    ///
    /// The default pixel data process pipeline
    /// applies the Modality LUT function,
    /// followed by the first VOI LUT transformation found in the object.
    /// To change this behavior,
    /// see [`to_dynamic_image_with_options`](Self::to_dynamic_image_with_options).
    pub fn to_dynamic_image(&self, frame: u32) -> Result<DynamicImage> {
        self.to_dynamic_image_with_options(frame, &ConvertOptions::default())
    }

    /// Convert the decoded pixel data of a specific frame into a dynamic image.
    ///
    /// The `options` value allows you to specify
    /// which transformations should be done to the pixel data
    /// (primarily Modality LUT function and VOI LUT function).
    /// By default, both Modality and VOI LUT functions are applied
    /// according to the attributes of the given object.
    /// Note that certain options may be ignored
    /// if they do not apply.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dicom_pixeldata::{ConvertOptions, DecodedPixelData, VoiLutOption, WindowLevel};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let data: DecodedPixelData = unimplemented!();
    /// let options = ConvertOptions::new()
    ///     .with_voi_lut(VoiLutOption::Custom(WindowLevel {
    ///         center: -300.0,
    ///         width: 600.,
    ///     }));
    /// let img = data.to_dynamic_image_with_options(0, &options)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_dynamic_image_with_options(
        &self,
        frame: u32,
        options: &ConvertOptions,
    ) -> Result<DynamicImage> {
        match self.samples_per_pixel {
            1 => self.build_monochrome_image(frame, options),
            3 => {
                // Modality LUT and VOI LUT
                // are currently ignored in this case
                if self.planar_configuration != PlanarConfiguration::Standard {
                    // TODO #129
                    return UnsupportedOtherSnafu {
                        name: "PlanarConfiguration",
                        value: self.planar_configuration.to_string(),
                    }
                    .fail()?;
                }

                // RGB, YBR_FULL or YBR_FULL_422 colors
                match self.bits_allocated {
                    8 => {
                        let mut pixel_array = self.frame_data(frame)?.to_vec();

                        // Convert YBR_FULL or YBR_FULL_422 to RGB
                        let pixel_array = match &self.photometric_interpretation {
                            PhotometricInterpretation::Rgb => pixel_array,
                            PhotometricInterpretation::YbrFull
                            | PhotometricInterpretation::YbrFull422 => {
                                convert_colorspace_u8(&mut pixel_array);
                                pixel_array
                            }
                            pi => UnsupportedPhotometricInterpretationSnafu { pi: pi.clone() }
                                .fail()?,
                        };

                        let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
                            ImageBuffer::from_raw(self.cols, self.rows, pixel_array)
                                .context(InvalidImageBufferSnafu)?;
                        Ok(DynamicImage::ImageRgb8(image_buffer))
                    }
                    16 => {
                        let mut pixel_array: Vec<u16> = self.frame_data_ow(frame)?;

                        // Convert YBR_FULL or YBR_FULL_422 to RGB
                        let pixel_array = match &self.photometric_interpretation {
                            PhotometricInterpretation::Rgb => pixel_array,
                            PhotometricInterpretation::YbrFull
                            | PhotometricInterpretation::YbrFull422 => {
                                convert_colorspace_u16(&mut pixel_array);
                                pixel_array
                            }
                            pi => UnsupportedPhotometricInterpretationSnafu { pi: pi.clone() }
                                .fail()?,
                        };

                        let image_buffer: ImageBuffer<Rgb<u16>, Vec<u16>> =
                            ImageBuffer::from_raw(self.cols, self.rows, pixel_array)
                                .context(InvalidImageBufferSnafu)?;
                        Ok(DynamicImage::ImageRgb16(image_buffer))
                    }
                    _ => InvalidBitsAllocatedSnafu.fail()?,
                }
            }
            spp => UnsupportedSamplesPerPixelSnafu { spp }.fail()?,
        }
    }

    fn build_monochrome_image(&self, frame: u32, options: &ConvertOptions) -> Result<DynamicImage> {
        let ConvertOptions {
            modality_lut,
            voi_lut,
        } = options;

        let mut image = match self.bits_allocated {
            8 => {
                let data = self.frame_data(frame)?;

                match modality_lut {
                    // simplest one, no transformations
                    ModalityLutOption::None => {
                        let buffer: Vec<u8> = data.to_vec();
                        let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                            ImageBuffer::from_raw(self.cols, self.rows, buffer)
                                .context(InvalidImageBufferSnafu)?;
                        DynamicImage::ImageLuma8(image_buffer)
                    }
                    // other
                    ModalityLutOption::Default | ModalityLutOption::Override(..) => {
                        let rescale = if let ModalityLutOption::Override(rescale) = modality_lut {
                            *rescale
                        } else {
                            self.rescale()
                        };

                        let signed = self.pixel_representation == PixelRepresentation::Signed;

                        let lut: Lut<u8> = match (voi_lut, self.window) {
                            (VoiLutOption::Identity, _) => Lut::new_rescale(8, false, rescale)
                                .map_err(|_| snafu::NoneError)
                                .context(InvalidDataTypeSnafu)?,
                            (VoiLutOption::Default | VoiLutOption::First, Some(window)) => {
                                Lut::new_rescale_and_window(
                                    8,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(
                                        self.voi_lut_function.unwrap_or_default(),
                                        window,
                                    ),
                                )
                                .map_err(|_| snafu::NoneError)
                                .context(InvalidDataTypeSnafu)?
                            }
                            (VoiLutOption::Default | VoiLutOption::First, None) => {
                                // log warning (#49)
                                eprintln!("Could not find window level for object");
                                Lut::new_rescale(8, signed, rescale)
                                    .map_err(|_| snafu::NoneError)
                                    .context(InvalidDataTypeSnafu)?
                            }
                            (VoiLutOption::Custom(window), _) => Lut::new_rescale_and_window(
                                8,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    self.voi_lut_function.unwrap_or_default(),
                                    *window,
                                ),
                            )
                            .map_err(|_| snafu::NoneError)
                            .context(InvalidDataTypeSnafu)?,
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                8,
                                signed,
                                rescale,
                                data.into_iter().copied(),
                            )
                            .context(CreateLutSnafu)?,
                        };

                        let data: Vec<u8> = lut.map_par_iter(data.par_iter().copied()).collect();

                        let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                            ImageBuffer::from_raw(self.cols, self.rows, data)
                                .context(InvalidImageBufferSnafu)?;
                        DynamicImage::ImageLuma8(image_buffer)
                    }
                }
            }
            16 => {
                match modality_lut {
                    // only take pixel representation,
                    // convert to image only after shifting values
                    // to an unsigned scale
                    ModalityLutOption::None => {
                        let frame_length = self.rows as usize
                            * self.cols as usize
                            * 2
                            * self.samples_per_pixel as usize;
                        let frame_start = frame_length * frame as usize;
                        let frame_end = frame_start + frame_length;
                        if frame_end > (*self.data).len() {
                            FrameOutOfRangeSnafu {
                                frame_number: frame,
                            }
                            .fail()?
                        }

                        let buffer = match self.pixel_representation {
                            // Unsigned 16-bit representation
                            PixelRepresentation::Unsigned => {
                                let mut buffer = vec![0; frame_length / 2];
                                NativeEndian::read_u16_into(
                                    &self.data[frame_start..frame_end],
                                    &mut buffer,
                                );
                                buffer
                            }
                            // Signed 16-bit representation
                            PixelRepresentation::Signed => {
                                let mut signed_buffer = vec![0; frame_length / 2];
                                NativeEndian::read_i16_into(
                                    &self.data[frame_start..frame_end],
                                    &mut signed_buffer,
                                );
                                // Convert buffer to unsigned by shifting
                                convert_i16_to_u16(&signed_buffer)
                            }
                        };

                        let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                            ImageBuffer::from_raw(self.cols, self.rows, buffer)
                                .context(InvalidImageBufferSnafu)?;
                        DynamicImage::ImageLuma16(image_buffer)
                    }

                    ModalityLutOption::Default | ModalityLutOption::Override(..) => {
                        let rescale = if let ModalityLutOption::Override(rescale) = modality_lut {
                            *rescale
                        } else {
                            Rescale::new(self.rescale_slope, self.rescale_intercept)
                        };

                        // fetch pixel data as a slice of u16 values,
                        // irrespective of pixel signedness
                        // (that is handled by the LUT)
                        let signed = self.pixel_representation == PixelRepresentation::Signed;
                        // Note: samples are not read as `i16` even if signed,
                        // because the LUT takes care of interpreting them properly.

                        let samples = self.frame_data_ow(frame)?;

                        // use 16-bit precision to prevent possible loss of precision in image
                        let lut: Lut<u16> = match (voi_lut, self.window) {
                            (VoiLutOption::Identity, _) => {
                                Lut::new_rescale(self.bits_stored, signed, rescale)
                            }
                            (VoiLutOption::Default | VoiLutOption::First, Some(window)) => {
                                Lut::new_rescale_and_window(
                                    self.bits_stored,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(
                                        self.voi_lut_function.unwrap_or_default(),
                                        window,
                                    ),
                                )
                            }
                            (VoiLutOption::Default | VoiLutOption::First, None) => {
                                // log warning (#49)
                                eprintln!("Could not find window level for object");

                                Lut::new_rescale_and_normalize(
                                    self.bits_stored,
                                    signed,
                                    rescale,
                                    samples.iter().copied(),
                                )
                            }
                            (VoiLutOption::Custom(window), _) => Lut::new_rescale_and_window(
                                self.bits_stored,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    self.voi_lut_function.unwrap_or_default(),
                                    *window,
                                ),
                            ),
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                self.bits_stored,
                                signed,
                                rescale,
                                samples.iter().copied(),
                            ),
                        }
                        .context(CreateLutSnafu)?;

                        let data: Vec<u16> =
                            lut.map_par_iter(samples.par_iter().copied()).collect();

                        let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                            ImageBuffer::from_raw(self.cols, self.rows, data)
                                .context(InvalidImageBufferSnafu)?;
                        DynamicImage::ImageLuma16(image_buffer)
                    }
                }
            }
            _ => InvalidBitsAllocatedSnafu.fail()?,
        };
        // Convert MONOCHROME1 => MONOCHROME2
        if self.photometric_interpretation == PhotometricInterpretation::Monochrome1 {
            image.invert();
        }
        Ok(image)
    }

    /// Convert all of the decoded pixel data into a vector of flat pixels
    /// of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is ignored.
    ///
    /// The default pixel data process pipeline
    /// applies only the Modality LUT function.
    /// To change this behavior,
    /// see [`to_vec_with_options`](Self::to_vec_with_options).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dicom_pixeldata::{ConvertOptions, DecodedPixelData, VoiLutOption, WindowLevel};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let data: DecodedPixelData = unimplemented!();
    /// // get the pixels of all frames as 32-bit modality values
    /// let all_pixels: Vec<f32> = data.to_vec()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_vec<T: 'static>(&self) -> Result<Vec<T>>
    where
        T: NumCast,
        T: Send + Sync,
        T: Copy,
    {
        self.convert_pixel_slice(&self.data[..], &Default::default())
    }

    /// Convert all of the decoded pixel data into a vector of flat pixels
    /// of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is ignored.
    ///
    /// The `options` value allows you to specify
    /// which transformations should be done to the pixel data
    /// (primarily Modality LUT function and VOI LUT function).
    /// By default, only the Modality LUT function is applied.
    pub fn to_vec_with_options<T: 'static>(&self, options: &ConvertOptions) -> Result<Vec<T>>
    where
        T: NumCast,
        T: Send + Sync,
        T: Copy,
    {
        self.convert_pixel_slice(&self.data[..], options)
    }

    /// Convert the decoded pixel data of a frame
    /// into a vector of flat pixels of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is ignored.
    ///
    /// The default pixel data process pipeline
    /// applies only the Modality LUT function.
    /// To change this behavior,
    /// see [`to_vec_frame_with_options`](Self::to_vec_frame_with_options).
    pub fn to_vec_frame<T: 'static>(&self, frame: u32) -> Result<Vec<T>>
    where
        T: NumCast,
        T: Send + Sync,
        T: Copy,
    {
        self.convert_pixel_slice(self.frame_data(frame)?, &Default::default())
    }

    /// Convert the decoded pixel data of a frame
    /// into a vector of flat pixels of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is considered
    /// to identify whether rescaling should be applied.
    /// The pixel values are not inverted
    /// if photometric interpretation is `MONOCHROME1`.
    ///
    /// The `options` value allows you to specify
    /// which transformations should be done to the pixel data
    /// (primarily Modality LUT function and VOI LUT function).
    /// By default, only the Modality LUT function is applied
    /// according to the attributes of the given object.
    /// Note that certain options may be ignored
    /// if they do not apply.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dicom_pixeldata::{ConvertOptions, DecodedPixelData, VoiLutOption, WindowLevel};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let data: DecodedPixelData = unimplemented!();
    /// let options = ConvertOptions::new()
    ///     .with_voi_lut(VoiLutOption::Custom(WindowLevel {
    ///         center: -300.0,
    ///         width: 600.,
    ///     }));
    /// // get the pixels of the first frame with 8 bits per channel
    /// let first_frame_pixels: Vec<u8> = data.to_vec_frame_with_options(0, &options)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_vec_frame_with_options<T: 'static>(
        &self,
        frame: u32,
        options: &ConvertOptions,
    ) -> Result<Vec<T>>
    where
        T: NumCast,
        T: Send + Sync,
        T: Copy,
    {
        self.convert_pixel_slice(self.frame_data(frame)?, options)
    }

    fn convert_pixel_slice<T: 'static>(
        &self,
        data: &[u8],
        options: &ConvertOptions,
    ) -> Result<Vec<T>>
    where
        T: NumCast,
        T: Send + Sync,
        T: Copy,
    {
        let ConvertOptions {
            modality_lut,
            voi_lut,
        } = options;

        if self.samples_per_pixel > 1 && self.planar_configuration != PlanarConfiguration::Standard
        {
            // TODO #129
            return UnsupportedOtherSnafu {
                name: "PlanarConfiguration",
                value: self.planar_configuration.to_string(),
            }
            .fail()?;
        }

        match self.bits_allocated {
            8 => {
                match modality_lut {
                    ModalityLutOption::Default | ModalityLutOption::Override(_)
                        if self.photometric_interpretation.is_monochrome() =>
                    {
                        let rescale = if let ModalityLutOption::Override(rescale) = modality_lut {
                            *rescale
                        } else {
                            self.rescale()
                        };
                        let signed = self.pixel_representation == PixelRepresentation::Signed;

                        let lut: Lut<T> = match (voi_lut, self.window) {
                            (VoiLutOption::Default | VoiLutOption::Identity, _) => {
                                Lut::new_rescale(8, signed, rescale)
                            }
                            (VoiLutOption::First, Some(window)) => Lut::new_rescale_and_window(
                                8,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    self.voi_lut_function.unwrap_or_default(),
                                    window,
                                ),
                            ),
                            (VoiLutOption::First, None) => {
                                // log warning (#49)
                                eprintln!("Could not find window level for object");
                                Lut::new_rescale(8, signed, rescale)
                            }
                            (VoiLutOption::Custom(window), _) => Lut::new_rescale_and_window(
                                8,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    self.voi_lut_function.unwrap_or_default(),
                                    *window,
                                ),
                            ),
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                8,
                                signed,
                                rescale,
                                data.iter().copied(),
                            ),
                        }
                        .context(CreateLutSnafu)?;

                        let data: Vec<T> = lut.map_par_iter(data.par_iter().copied()).collect();

                        Ok(data)
                    }
                    _ => {
                        // 1-channel Grayscale image
                        let converted: Result<Vec<T>, _> = data
                            .par_iter()
                            .map(|v| T::from(*v).ok_or(snafu::NoneError))
                            .collect();
                        converted.context(InvalidDataTypeSnafu).map_err(Error::from)
                    }
                }
            }
            16 => {
                match modality_lut {
                    ModalityLutOption::Default | ModalityLutOption::Override(_)
                        if self.photometric_interpretation.is_monochrome() =>
                    {
                        let rescale = if let ModalityLutOption::Override(rescale) = modality_lut {
                            *rescale
                        } else {
                            self.rescale()
                        };

                        let signed = self.pixel_representation == PixelRepresentation::Signed;

                        let lut: Lut<T> = match (voi_lut, self.window) {
                            (VoiLutOption::Default | VoiLutOption::Identity, _) => {
                                Lut::new_rescale(8, signed, rescale)
                            }
                            (VoiLutOption::First, Some(window)) => Lut::new_rescale_and_window(
                                8,
                                false,
                                rescale,
                                WindowLevelTransform::new(
                                    self.voi_lut_function.unwrap_or_default(),
                                    window,
                                ),
                            ),
                            (VoiLutOption::First, None) => {
                                // log warning (#49)
                                eprintln!("Could not find window level for object");
                                Lut::new_rescale(8, signed, rescale)
                            }
                            (VoiLutOption::Custom(window), _) => Lut::new_rescale_and_window(
                                8,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    self.voi_lut_function.unwrap_or_default(),
                                    *window,
                                ),
                            ),
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                8,
                                signed,
                                rescale,
                                data.iter().copied(),
                            ),
                        }
                        .map_err(|_| snafu::NoneError)
                        .context(InvalidDataTypeSnafu)?;

                        Ok(lut.map_par_iter(data.par_iter().copied()).collect())
                    }
                    _ => {
                        // no transformations
                        match self.pixel_representation {
                            // Unsigned 16 bit representation
                            PixelRepresentation::Unsigned => {
                                let mut dest = vec![0; data.len() / 2];
                                NativeEndian::read_u16_into(data, &mut dest);

                                let converted: Result<Vec<T>, _> = dest
                                    .par_iter()
                                    .map(|v| T::from(*v).ok_or(snafu::NoneError))
                                    .collect();
                                converted.context(InvalidDataTypeSnafu).map_err(Error::from)
                            }
                            // Signed 16 bit 2s complement representation
                            PixelRepresentation::Signed => {
                                let mut signed_buffer = vec![0; data.len() / 2];
                                NativeEndian::read_i16_into(data, &mut signed_buffer);

                                let converted: Result<Vec<T>, _> = signed_buffer
                                    .par_iter()
                                    .map(|v| T::from(*v).ok_or(snafu::NoneError))
                                    .collect();
                                converted.context(InvalidDataTypeSnafu).map_err(Error::from)
                            }
                        }
                    }
                }
            }
            _ => InvalidBitsAllocatedSnafu.fail()?,
        }
    }

    /// Convert all of the decoded pixel data
    /// into a four dimensional array of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is considered
    /// to identify whether rescaling should be applied.
    /// The pixel values are not inverted
    /// if photometric interpretation is `MONOCHROME1`.
    ///
    /// The shape of the array will be `[N, R, C, S]`,
    /// where `N` is the number of frames,
    /// `R` is the number of rows,
    /// `C` is the number of columns,
    /// and `S` is the number of samples per pixel.
    pub fn to_ndarray<T: 'static>(&self) -> Result<Array<T, Ix4>>
    where
        T: NumCast,
        T: Copy,
        T: Send + Sync,
    {
        self.to_ndarray_with_options(&Default::default())
    }

    /// Convert all of the decoded pixel data
    /// into a four dimensional array of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is considered
    /// to identify whether rescaling should be applied.
    /// The pixel values are not inverted
    /// if photometric interpretation is `MONOCHROME1`.
    ///
    /// The shape of the array will be `[N, R, C, S]`,
    /// where `N` is the number of frames,
    /// `R` is the number of rows,
    /// `C` is the number of columns,
    /// and `S` is the number of samples per pixel.
    ///
    /// The `options` value allows you to specify
    /// which transformations should be done to the pixel data
    /// (primarily Modality LUT function and VOI LUT function).
    /// By default, both Modality and VOI LUT functions are applied
    /// according to the attributes of the given object.
    /// Note that certain options may be ignored
    /// if they do not apply.
    pub fn to_ndarray_with_options<T: 'static>(
        &self,
        options: &ConvertOptions,
    ) -> Result<Array<T, Ix4>>
    where
        T: NumCast,
        T: Copy,
        T: Send + Sync,
    {
        // Array shape is NumberOfFrames x Rows x Cols x SamplesPerPixel
        let shape = [
            self.number_of_frames as usize,
            self.rows as usize,
            self.cols as usize,
            self.samples_per_pixel as usize,
        ];

        let converted = self.to_vec_with_options::<T>(options)?;
        Array::from_shape_vec(shape, converted)
            .context(InvalidShapeSnafu)
            .map_err(Error::from)
    }

    /// Convert the decoded pixel data of a single frame
    /// into a three dimensional array of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is considered
    /// to identify whether rescaling should be applied.
    /// The pixel values are not inverted
    /// if photometric interpretation is `MONOCHROME1`.
    ///
    /// The shape of the array will be `[R, C, S]`,
    /// where `R` is the number of rows,
    /// `C` is the number of columns,
    /// and `S` is the number of samples per pixel.
    pub fn to_ndarray_frame<T: 'static>(&self, frame: u32) -> Result<Array<T, Ix3>>
    where
        T: NumCast,
        T: Copy,
        T: Send + Sync,
    {
        self.to_ndarray_frame_with_options(frame, &Default::default())
    }

    /// Convert the decoded pixel data of a single frame
    /// into a three dimensional array of a given type `T`.
    ///
    /// The underlying pixel data type is extracted based on
    /// the bits allocated and pixel representation,
    /// which is then converted to the requested type.
    /// Photometric interpretation is considered
    /// to identify whether rescaling should be applied.
    /// The pixel values are not inverted
    /// if photometric interpretation is `MONOCHROME1`.
    ///
    /// The shape of the array will be `[R, C, S]`,
    /// where `R` is the number of rows,
    /// `C` is the number of columns,
    /// and `S` is the number of samples per pixel.
    pub fn to_ndarray_frame_with_options<T: 'static>(
        &self,
        frame: u32,
        options: &ConvertOptions,
    ) -> Result<Array<T, Ix3>>
    where
        T: NumCast,
        T: Copy,
        T: Send + Sync,
    {
        // Array shape is Rows x Cols x SamplesPerPixel
        let shape = [
            self.rows as usize,
            self.cols as usize,
            self.samples_per_pixel as usize,
        ];

        let converted = self.to_vec_frame_with_options::<T>(frame, options)?;
        Array::from_shape_vec(shape, converted)
            .context(InvalidShapeSnafu)
            .map_err(Error::from)
    }
}

// Convert u8 pixel array from YBR_FULL or YBR_FULL_422 to RGB
// Every pixel is replaced with an RGB value
fn convert_colorspace_u8(i: &mut [u8]) {
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
fn convert_colorspace_u16(i: &mut [u16]) {
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

/// Convert the i16 vector by shifting it up,
/// thus maintaining the order between sample values.
fn convert_i16_to_u16(i: &[i16]) -> Vec<u16> {
    i.par_iter().map(|p| (*p as i32 + 0x8000) as u16).collect()
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
        use attribute::*;
        use std::convert::TryFrom;

        let pixel_data = pixel_data(self).context(GetAttributeSnafu)?;
        let cols = cols(self).context(GetAttributeSnafu)?;
        let rows = rows(self).context(GetAttributeSnafu)?;

        let photometric_interpretation =
            photometric_interpretation(self).context(GetAttributeSnafu)?;
        let samples_per_pixel = samples_per_pixel(self).context(GetAttributeSnafu)?;
        let planar_configuration = planar_configuration(self).context(GetAttributeSnafu)?;
        let bits_allocated = bits_allocated(self).context(GetAttributeSnafu)?;
        let bits_stored = bits_stored(self).context(GetAttributeSnafu)?;
        let high_bit = high_bit(self).context(GetAttributeSnafu)?;
        let pixel_representation = pixel_representation(self).context(GetAttributeSnafu)?;
        let rescale_intercept = rescale_intercept(self);
        let rescale_slope = rescale_slope(self);
        let number_of_frames = number_of_frames(self).context(GetAttributeSnafu)?;
        let voi_lut_function = voi_lut_function(self).context(GetAttributeSnafu)?;
        let voi_lut_function = voi_lut_function.and_then(|v| VoiLutFunction::try_from(&*v).ok());

        let window = if let Some(window_center) = window_center(self).context(GetAttributeSnafu)? {
            let window_width = window_width(self).context(GetAttributeSnafu)?;

            window_width.map(|width| WindowLevel {
                center: window_center,
                width,
            })
        } else {
            None
        };

        let transfer_syntax = &self.meta().transfer_syntax;
        let ts = TransferSyntaxRegistry
            .get(transfer_syntax)
            .with_context(|| UnknownTransferSyntaxSnafu {
                ts_uid: transfer_syntax,
            })?;

        if !ts.fully_supported() {
            return UnsupportedTransferSyntaxSnafu {
                ts: transfer_syntax,
            }
            .fail()?;
        }

        // Try decoding it using a native Rust decoder
        if let Codec::PixelData(decoder) = ts.codec() {
            let mut data: Vec<u8> = Vec::new();
            (*decoder)
                .decode(self, &mut data)
                .context(DecodePixelDataSnafu)?;

            // pixels are already interpreted,
            // set new photometric interpretation
            let new_pi = match samples_per_pixel {
                1 => PhotometricInterpretation::Monochrome2,
                3 => PhotometricInterpretation::Rgb,
                _ => photometric_interpretation,
            };

            return Ok(DecodedPixelData {
                data: Cow::from(data),
                cols: cols.into(),
                rows: rows.into(),
                number_of_frames,
                photometric_interpretation: new_pi,
                samples_per_pixel,
                planar_configuration: PlanarConfiguration::Standard,
                bits_allocated,
                bits_stored,
                high_bit,
                pixel_representation,
                rescale_intercept,
                rescale_slope,
                voi_lut_function,
                window,
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
            planar_configuration,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            rescale_intercept,
            rescale_slope,
            voi_lut_function,
            window,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_object::open_file;
    use dicom_test_files;

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
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        assert!(matches!(
            obj.decode_pixel_data().unwrap().to_ndarray::<u8>(),
            Err(Error(InnerError::InvalidDataType { .. }))
        ));
    }

    #[test]
    fn test_correct_ri_extracted() {
        // Rescale Slope and Intercept exist for this scan
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(pixel_data.rescale(), Rescale::new(1., -1024.));
    }

    #[test]
    fn test_correct_rescale_extracted_without_element() {
        // RescaleIntercept does not exists for this scan
        let test_file = dicom_test_files::path("pydicom/MR_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(pixel_data.rescale(), Rescale::new(1., 0.));
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
            Err(Error(InnerError::FrameOutOfRange {
                frame_number: 1, ..
            })) => {}
            _ => panic!("Unexpected positive outcome for out of range access"),
        }
    }
    #[cfg(not(feature = "gdcm"))]
    mod not_gdcm {
        use super::*;
        use rstest::rstest;
        use std::fs;
        use std::path::Path;

        #[test]
        fn test_native_decoding_pixel_data_rle_8bit_1frame() {
            let path = dicom_test_files::path("pydicom/SC_rgb_rle.dcm")
                .expect("test DICOM file should exist");
            let object = open_file(&path).unwrap();

            let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
            let ndarray = object
                .decode_pixel_data()
                .unwrap()
                .to_ndarray_with_options::<u8>(&options)
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

        #[test]
        fn test_native_decoding_pixel_data_rle_8bit_2frame() {
            let path = dicom_test_files::path("pydicom/SC_rgb_rle_2frame.dcm")
                .expect("test DICOM file should exist");
            let object = open_file(&path).unwrap();
            let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
            let ndarray = object
                .decode_pixel_data()
                .unwrap()
                .to_ndarray_with_options::<u8>(&options)
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

        #[test]
        fn test_native_decoding_pixel_data_rle_16bit_1frame() {
            let path = dicom_test_files::path("pydicom/SC_rgb_rle_16bit.dcm")
                .expect("test DICOM file should exist");
            let object = open_file(&path).unwrap();
            let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
            let ndarray = object
                .decode_pixel_data()
                .unwrap()
                .to_ndarray_with_options::<u16>(&options)
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

        const MAX_TEST_FRAMES: u32 = 16;

        #[rstest]
        // jpeg2000 encoding not supported
        #[should_panic(expected = "UnsupportedTransferSyntax { ts: \"1.2.840.10008.1.2.4.91\"")]
        #[case("pydicom/693_J2KI.dcm", 1)]
        #[should_panic(expected = "UnsupportedTransferSyntax { ts: \"1.2.840.10008.1.2.4.90\"")]
        #[case("pydicom/693_J2KR.dcm", 1)]
        //
        // jpeg-ls encoding not supported
        #[should_panic(expected = "UnsupportedTransferSyntax { ts: \"1.2.840.10008.1.2.4.80\"")]
        #[case("pydicom/emri_small_jpeg_ls_lossless.dcm", 10)]
        #[should_panic(expected = "UnsupportedTransferSyntax { ts: \"1.2.840.10008.1.2.4.80\"")]
        #[case("pydicom/MR_small_jpeg_ls_lossless.dcm", 1)]
        //
        // sample precicion of 12 not supported
        #[should_panic(expected = "Unsupported(SamplePrecision(12))")]
        #[case("pydicom/JPEG-lossy.dcm", 1)]
        //
        // works fine
        #[case("pydicom/color3d_jpeg_baseline.dcm", 120)]
        //
        // works fine
        #[case("pydicom/JPEG-LL.dcm", 1)]
        #[case("pydicom/JPGLosslessP14SV1_1s_1f_8b.dcm", 1)]
        #[case("pydicom/SC_rgb_jpeg_gdcm.dcm", 1)]
        #[case("pydicom/SC_rgb_jpeg_lossy_gdcm.dcm", 1)]

        fn test_parse_jpeg_encoded_dicom_pixel_data(#[case] value: &str, #[case] frames: u32) {
            let test_file = dicom_test_files::path(value).unwrap();
            println!("Parsing pixel data for {}", test_file.display());
            let obj = open_file(test_file).unwrap();
            let pixel_data = obj.decode_pixel_data().unwrap();
            assert_eq!(pixel_data.number_of_frames(), frames);

            let output_dir = Path::new(
                "../target/dicom_test_files/_out/test_parse_jpeg_encoded_dicom_pixel_data",
            );
            fs::create_dir_all(output_dir).unwrap();

            for i in 0..pixel_data.number_of_frames().min(MAX_TEST_FRAMES) {
                let image = pixel_data.to_dynamic_image(i).unwrap();
                let image_path = output_dir.join(format!(
                    "{}-{}.png",
                    Path::new(value).file_stem().unwrap().to_str().unwrap(),
                    i,
                ));
                image.save(image_path).unwrap();
            }
        }
    }
}
