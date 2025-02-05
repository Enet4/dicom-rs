#![allow(clippy::derive_partial_eq_without_eq)]
//! This crate contains the DICOM pixel data handlers and is
//! responsible for decoding various forms of native and compressed pixel data,
//! such as JPEG lossless,
//! and convert it into more usable data structures.
//!
//! `dicom-pixeldata` currently supports a small,
//! but increasing number of DICOM image encodings in pure Rust.
//! As a way to mitigate the current gap,
//! this library has an integration with [GDCM bindings]
//! for an extended range of encodings.
//! This integration is behind the Cargo feature "gdcm",
//! which requires CMake and a C++ compiler.
//!
//! [GDCM bindings]: https://crates.io/crates/gdcm-rs
//!
//! ```toml
//! dicom-pixeldata = { version = "0.7", features = ["gdcm"] }
//! ```
//!
//! Once the pixel data is decoded,
//! the decoded data can be converted to:
//! - a vector of flat pixel data values;
//! - a [multi-dimensional array](ndarray::Array), using [`ndarray`];
//! - or a [dynamic image object](image::DynamicImage), using [`image`].
//!
//! This conversion includes
//! eventual Modality and value of interest (VOI) transformations.
//!
//! # WebAssembly support
//! This library works in WebAssembly with the following two measures:
//!  - Ensure that the "gdcm" feature is disabled.
//!    This allows the crate to be compiled for WebAssembly
//!    albeit at the cost of supporting a lesser variety of compression algorithms.
//!  - And either set up [`wasm-bindgen-rayon`][1]
//!    or disable the `rayon` feature.
//!
//! [1]: https://crates.io/crates/wasm-bindgen-rayon
//!
//! # Examples
//!
//! To convert a DICOM object into a dynamic image
//! (requires the `image` feature):
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::PixelDecoder;
//! # #[cfg(feature = "image")]
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("dicom.dcm")?;
//! let image = obj.decode_pixel_data()?;
//! let dynamic_image = image.to_dynamic_image(0)?;
//! dynamic_image.save("out.png")?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "image"))]
//! # fn main() {}
//! ```
//!
//! To convert a DICOM object into an ndarray
//! (requires the `ndarray` feature):
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::PixelDecoder;
//! # #[cfg(feature = "ndarray")]
//! use ndarray::s;
//! # #[cfg(feature = "ndarray")]
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("rgb_dicom.dcm")?;
//! let pixel_data = obj.decode_pixel_data()?;
//! let ndarray = pixel_data.to_ndarray::<u16>()?;
//! let red_values = ndarray.slice(s![.., .., .., 0]);
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "ndarray"))]
//! # fn main() {}
//! ```
//!
//! In order to parameterize the conversion,
//! pass a conversion options value to the `_with_options` variant methods.
//!
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::{ConvertOptions, PixelDecoder, VoiLutOption};
//! # #[cfg(feature = "image")]
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("dicom.dcm")?;
//! let image = obj.decode_pixel_data()?;
//! let options = ConvertOptions::new()
//!     .with_voi_lut(VoiLutOption::Normalize)
//!     .force_8bit();
//! let dynamic_image = image.to_dynamic_image_with_options(0, &options)?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "image"))]
//! # fn main() {}
//! ```
//!
//! See [`ConvertOptions`] for the options available,
//! including the default behavior for each method.
//!

use byteorder::{ByteOrder, NativeEndian};
#[cfg(not(feature = "gdcm"))]
use dicom_core::{DataDictionary, DicomValue};
use dicom_encoding::adapters::DecodeError;
#[cfg(not(feature = "gdcm"))]
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
#[cfg(not(feature = "gdcm"))]
use dicom_encoding::Codec;
#[cfg(not(feature = "gdcm"))]
use dicom_object::{FileDicomObject, InMemDicomObject};
#[cfg(not(feature = "gdcm"))]
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
#[cfg(feature = "image")]
use image::{DynamicImage, ImageBuffer, Luma, Rgb};
#[cfg(feature = "ndarray")]
use ndarray::{Array, Ix3, Ix4};
use num_traits::NumCast;
#[cfg(feature = "rayon")]
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
#[cfg(all(feature = "rayon", feature = "image"))]
use rayon::slice::ParallelSliceMut;
#[cfg(not(feature = "gdcm"))]
use snafu::ensure;
#[cfg(any(not(feature = "gdcm"), feature = "image"))]
use snafu::OptionExt;
use snafu::{Backtrace, ResultExt, Snafu};
use std::borrow::Cow;
#[cfg(not(feature = "gdcm"))]
use std::iter::zip;

#[cfg(feature = "image")]
pub use image;
#[cfg(feature = "ndarray")]
pub use ndarray;

mod attribute;
mod lut;
mod transcode;

pub mod encapsulation;
pub(crate) mod transform;

// re-exports
pub use attribute::{PhotometricInterpretation, PixelRepresentation, PlanarConfiguration};
pub use lut::{CreateLutError, Lut};
pub use transcode::{Error as TranscodeError, Result as TranscodeResult, Transcode};
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

    #[snafu(display("Invalid buffer when constructing ImageBuffer"))]
    InvalidImageBuffer { backtrace: Backtrace },

    #[cfg(feature = "ndarray")]
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
    #[snafu(display("Value multiplicity of VOI LUT Function must match the number of frames. Expected `{:?}`, found `{:?}`", nr_frames, vm))]
    LengthMismatchVoiLutFunction {
        vm: u32,
        nr_frames: u32,
        backtrace: Backtrace,
    },
    #[snafu(display("Value multiplicity of Rescale Slope/Intercept must match. Found `{:?}` (slope), `{:?}` (intercept)", slope_vm, intercept_vm))]
    LengthMismatchRescale {
        intercept_vm: u32,
        slope_vm: u32,
        backtrace: Backtrace,
    },
    #[snafu(display("Value multiplicity of Window Center/Width must match. Found `{:?}` (center), `{:?}` (width)", wc_vm, ww_vm))]
    LengthMismatchWindowLevel {
        wc_vm: u32,
        ww_vm: u32,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Option set for converting decoded pixel data
/// into other common data structures,
/// such as a vector, an image, or a multidimensional array.
///
/// Each option listed affects the transformation in this order:
/// 1. The Modality LUT function (`modality_lut`)
///    is applied to the raw pixel data sample values.
///    This is usually an affine function based on the
///    _Rescale Slope_ and _Rescale Intercept_ attributes.
///    If this option is set to [`None`](ModalityLutOption::None),
///    the VOI LUT function is ignored.
/// 2. The VOI LUT function (`voi_lut`)
///    is applied to the rescaled values,
///    such as a window level.
/// 3. In the case of converting to an image,
///    the transformed values are extended or narrowed
///    to the range of the target bit depth (`bit_depth`).
#[derive(Debug, Default, Clone, PartialEq)]
#[non_exhaustive]
pub struct ConvertOptions {
    /// Modality LUT option
    pub modality_lut: ModalityLutOption,
    /// VOI LUT option
    pub voi_lut: VoiLutOption,
    /// Output image bit depth
    pub bit_depth: BitDepthOption,
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

    /// Set the output bit depth option.
    pub fn with_bit_depth(mut self, bit_depth: BitDepthOption) -> Self {
        self.bit_depth = bit_depth;
        self
    }

    /// Set the output bit depth option to force 8 bits.
    ///
    /// This is equivalent to `self.with_bit_depth(BitDepthOption::Force8Bit)`.
    pub fn force_8bit(mut self) -> Self {
        self.bit_depth = BitDepthOption::Force8Bit;
        self
    }

    /// Set the output bit depth option to force 16 bits.
    ///
    /// This is equivalent to `self.with_bit_depth(BitDepthOption::Force16Bit)`.
    pub fn force_16bit(mut self) -> Self {
        self.bit_depth = BitDepthOption::Force16Bit;
        self
    }
}

/// Modality LUT function specifier.
///
/// See also [`ConvertOptions`].
#[derive(Debug, Default, Clone, PartialEq)]
#[non_exhaustive]
pub enum ModalityLutOption {
    /// _Default behavior:_
    /// rescale the pixel data values
    /// as described in the decoded pixel data.
    #[default]
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

/// VOI LUT function specifier.
///
/// Note that the VOI LUT function is only applied
/// alongside a modality LUT function.
///
/// See also [`ConvertOptions`].
#[derive(Debug, Default, Clone, PartialEq)]
#[non_exhaustive]
pub enum VoiLutOption {
    /// _Default behavior:_
    /// apply the first VOI LUT function transformation described in the pixel data
    /// only when converting to an image;
    /// no VOI LUT function is performed
    /// when converting to an ndarray or to bare pixel values.
    #[default]
    Default,
    /// Apply the first VOI LUT function transformation
    /// described in the pixel data.
    First,
    /// Apply a custom window level instead of the one described in the object.
    Custom(WindowLevel),
    /// Apply a custom window level and a custom function instead of the one described in the object.
    CustomWithFunction(WindowLevel, VoiLutFunction),
    /// Perform a min-max normalization instead,
    /// so that the lowest value is 0 and
    /// the highest value is the maximum value of the target type.
    Normalize,
    /// Do not apply any VOI LUT transformation.
    Identity,
}

/// Output image bit depth specifier.
///
/// Note that this is only applied
/// when converting to an image.
/// In the other cases,
/// output narrowing is already done by the caller
/// when specifying the intended output element type.
///
/// See also [`ConvertOptions`].
#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[non_exhaustive]
pub enum BitDepthOption {
    /// _Default behavior:_
    /// infer the bit depth based on the input's number of bits per sample.
    #[default]
    Auto,
    /// Force the output image to have 8 bits per sample.
    Force8Bit,
    /// Force the output image to have 16 bits per sample.
    Force16Bit,
}

/// A blob of decoded pixel data.
///
/// This is the outcome of collecting a DICOM object's imaging-related attributes
/// into a decoded form
/// (see [`PixelDecoder`]).
/// The decoded pixel data samples will be stored as raw bytes in native form
/// without any LUT transformations applied.
/// Whether to apply such transformations
/// can be specified through one of the various `to_*` methods,
/// such as [`to_dynamic_image`](Self::to_dynamic_image)
/// and [`to_vec`](Self::to_vec).
#[derive(Debug, Clone)]
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
    /// Multiframe dicom objects can have rescale information, voi LUT and
    /// window level information once in the shared functional group sequence,
    /// or multiple times in the per-frame functional group sequence. This is a
    /// vector of intercepts and slopes, one for each frame.
    ///
    /// the pixel value rescale slope and intercept
    rescale: Vec<Rescale>,
    // the VOI LUT function
    voi_lut_function: Option<Vec<VoiLutFunction>>,
    /// the window level specified via width and center
    window: Option<Vec<WindowLevel>>,

    /// Enforce frame functional groups VMs match `number_of_frames`
    enforce_frame_fg_vm_match: bool,
}

impl DecodedPixelData<'_> {
    // getter methods

    /// Retrieve a slice of all raw pixel data samples as bytes,
    /// irrespective of the expected size of each sample.
    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Retrieve a copy of all raw pixel data samples
    /// as unsigned 16-bit integers.
    ///
    /// This is useful for retrieving pixel data
    /// with the _OW_ value representation.
    #[inline]
    pub fn data_ow(&self) -> Vec<u16> {
        bytes_to_vec_u16(&self.data)
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

        Ok(&self.data[frame_start..frame_end])
    }

    /// Retrieve a copy of a frame's raw pixel data samples
    /// as unsigned 16-bit integers.
    ///
    /// This is useful for retrieving pixel data
    /// with the _OW_ value representation.
    pub fn frame_data_ow(&self, frame: u32) -> Result<Vec<u16>> {
        let data = self.frame_data(frame)?;

        Ok(bytes_to_vec_u16(data))
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
    pub fn samples_per_pixel(&self) -> u16 {
        self.samples_per_pixel
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
    pub fn rescale(&self) -> Result<&[Rescale]> {
        match &self.rescale.len() {
            0 => Ok(&[Rescale {
                slope: 1.,
                intercept: 0.,
            }]),
            1 => Ok(&self.rescale),
            len => {
                if *len == self.number_of_frames as usize {
                    Ok(&self.rescale)
                } else {
                    if self.enforce_frame_fg_vm_match {
                        LengthMismatchRescaleSnafu {
                            slope_vm: *len as u32,
                            intercept_vm: *len as u32,
                        }
                        .fail()?
                    }
                    tracing::warn!("Expected `{:?}` rescale parameters, found `{:?}`, using first value for all", self.number_of_frames, len);
                    Ok(&self.rescale[0..1])
                }
            }
        }
    }

    /// Retrieve the VOI LUT function defined by the object, if any.
    #[inline]
    pub fn voi_lut_function(&self) -> Result<Option<&[VoiLutFunction]>> {
        if let Some(inner) = &self.voi_lut_function {
            let res = match &inner.len() {
                0 => Ok(None),
                1 => Ok(Some(inner.as_slice())),
                len => {
                    if *len == self.number_of_frames as usize {
                        Ok(Some(inner.as_slice()))
                    } else {
                        if self.enforce_frame_fg_vm_match {
                            LengthMismatchVoiLutFunctionSnafu {
                                vm: *len as u32,
                                nr_frames: self.number_of_frames,
                            }
                            .fail()?
                        }
                        tracing::warn!("Expected `{:?}` VOI LUT functions, found `{:?}`, using first value for all", self.number_of_frames, len);
                        Ok(Some(&inner[0..1]))
                    }
                }
            };
            res
        } else {
            Ok(None)
        }
    }

    #[inline]
    pub fn window(&self) -> Result<Option<&[WindowLevel]>> {
        if let Some(inner) = &self.window {
            let res = match &inner.len() {
                0 => Ok(None),
                1 => Ok(Some(inner.as_slice())),
                len => {
                    if *len == self.number_of_frames as usize {
                        Ok(Some(inner.as_slice()))
                    } else {
                        if self.enforce_frame_fg_vm_match {
                            LengthMismatchWindowLevelSnafu {
                                ww_vm: *len as u32,
                                wc_vm: *len as u32,
                            }
                            .fail()?
                        }
                        tracing::warn!("Expected `{:?}` Window Levels, found `{:?}`, using first value for all", self.number_of_frames, len);
                        Ok(Some(&inner[0..1]))
                    }
                }
            };
            res
        } else {
            Ok(None)
        }
    }

    // converter methods

    /// Convert the decoded pixel data of a specific frame into a dynamic image.
    ///
    /// The default pixel data process pipeline
    /// applies the Modality LUT function,
    /// followed by the first VOI LUT transformation found in the object.
    /// To change this behavior,
    /// see [`to_dynamic_image_with_options`](Self::to_dynamic_image_with_options).
    #[cfg(feature = "image")]
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
    #[cfg(feature = "image")]
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

                // RGB, YBR_FULL or YBR_FULL_422 colors
                match self.bits_allocated {
                    8 => {
                        let mut pixel_array = match self.planar_configuration {
                            PlanarConfiguration::Standard => self.frame_data(frame)?.to_vec(),
                            PlanarConfiguration::PixelFirst => interleave(self.frame_data(frame)?),
                        };

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

                        self.rgb_image_with_extend(pixel_array, options.bit_depth)
                    }
                    16 => {
                        let mut pixel_array: Vec<u16> = match self.planar_configuration {
                            PlanarConfiguration::Standard => self.frame_data_ow(frame)?,
                            PlanarConfiguration::PixelFirst => {
                                // Would there be a way to avoid copying the data twice
                                // here (once in frame_data_ow and once in interleave)?
                                interleave(&(self.frame_data_ow(frame)?))
                            }
                        };

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

                        self.rgb_image_with_narrow(pixel_array, options.bit_depth)
                    }
                    _ => InvalidBitsAllocatedSnafu.fail()?,
                }
            }
            spp => UnsupportedSamplesPerPixelSnafu { spp }.fail()?,
        }
    }

    #[cfg(feature = "image")]
    fn mono_image_with_narrow(
        &self,
        pixel_values: impl IntoIterator<Item = u16>,
        bit_depth: BitDepthOption,
    ) -> Result<DynamicImage> {
        if bit_depth == BitDepthOption::Force8Bit {
            // user requested 8 bits, narrow
            let data: Vec<u8> = pixel_values.into_iter().map(|x| (x >> 8) as u8).collect();
            let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma8(image_buffer))
        } else {
            let data: Vec<u16> = pixel_values.into_iter().collect();
            let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma16(image_buffer))
        }
    }

    #[cfg(all(feature = "image", feature = "rayon"))]
    fn mono_image_with_narrow_par(
        &self,
        pixel_values: impl ParallelIterator<Item = u16>,
        bit_depth: BitDepthOption,
    ) -> Result<DynamicImage> {
        if bit_depth == BitDepthOption::Force8Bit {
            // user requested 8 bits, narrow
            let data: Vec<u8> = pixel_values.map(|x| (x >> 8) as u8).collect();
            let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma8(image_buffer))
        } else {
            let data: Vec<u16> = pixel_values.collect();
            let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma16(image_buffer))
        }
    }

    #[cfg(feature = "image")]
    fn mono_image_with_extend(
        &self,
        pixel_values: impl IntoIterator<Item = u8>,
        bit_depth: BitDepthOption,
    ) -> Result<DynamicImage> {
        if bit_depth == BitDepthOption::Force16Bit {
            // user requested 16 bits, extend
            let data = pixel_values
                .into_iter()
                .map(|x| x as u16)
                .map(|x| (x << 8) + x)
                .collect();
            let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma16(image_buffer))
        } else {
            let data: Vec<u8> = pixel_values.into_iter().collect();
            let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma8(image_buffer))
        }
    }

    #[cfg(all(feature = "image", feature = "rayon"))]
    fn mono_image_with_extend_par(
        &self,
        pixel_values: impl ParallelIterator<Item = u8>,
        bit_depth: BitDepthOption,
    ) -> Result<DynamicImage> {
        if bit_depth == BitDepthOption::Force16Bit {
            // user requested 16 bits, extend
            let data = pixel_values
                .map(|x| x as u16)
                .map(|x| (x << 8) + x)
                .collect();
            let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma16(image_buffer))
        } else {
            let data: Vec<u8> = pixel_values.collect();
            let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageLuma8(image_buffer))
        }
    }

    #[cfg(feature = "image")]
    fn rgb_image_with_extend(
        &self,
        pixels: Vec<u8>,
        bit_depth: BitDepthOption,
    ) -> Result<DynamicImage> {
        if bit_depth == BitDepthOption::Force16Bit {
            // user requested 16 bits, extend
            let data: Vec<u16> = pixels
                .into_iter()
                .map(|x| x as u16)
                .map(|x| (x << 8) + x)
                .collect();
            let image_buffer: ImageBuffer<Rgb<u16>, Vec<u16>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageRgb16(image_buffer))
        } else {
            let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
                ImageBuffer::from_raw(self.cols, self.rows, pixels)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageRgb8(image_buffer))
        }
    }

    #[cfg(feature = "image")]
    fn rgb_image_with_narrow(
        &self,
        pixels: Vec<u16>,
        bit_depth: BitDepthOption,
    ) -> Result<DynamicImage> {
        if bit_depth == BitDepthOption::Force8Bit {
            // user requested 8 bits, narrow
            let data: Vec<u8> = pixels.into_iter().map(|x| (x >> 8) as u8).collect();
            let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
                ImageBuffer::from_raw(self.cols, self.rows, data)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageRgb8(image_buffer))
        } else {
            let image_buffer: ImageBuffer<Rgb<u16>, Vec<u16>> =
                ImageBuffer::from_raw(self.cols, self.rows, pixels)
                    .context(InvalidImageBufferSnafu)?;
            Ok(DynamicImage::ImageRgb16(image_buffer))
        }
    }

    #[cfg(feature = "image")]
    fn build_monochrome_image(&self, frame: u32, options: &ConvertOptions) -> Result<DynamicImage> {
        let ConvertOptions {
            modality_lut,
            voi_lut,
            bit_depth,
        } = options;

        let mut image = match self.bits_allocated {
            8 => {
                let data = self.frame_data(frame)?;

                match modality_lut {
                    // simplest one, no transformations
                    ModalityLutOption::None => {
                        self.mono_image_with_extend(data.iter().copied(), *bit_depth)?
                    }
                    // other
                    ModalityLutOption::Default | ModalityLutOption::Override(..) => {
                        let rescale = {
                            let default = self.rescale()?;
                            if let ModalityLutOption::Override(rescale) = modality_lut {
                                *rescale
                            } else if default.len() > 1 {
                                default[frame as usize]
                            } else {
                                default[0]
                            }
                        };

                        let signed = self.pixel_representation == PixelRepresentation::Signed;

                        let lut: Lut<u8> = match (voi_lut, self.window()?) {
                            (VoiLutOption::Identity, _) => {
                                Lut::new_rescale(8, false, rescale).context(CreateLutSnafu)?
                            }
                            (VoiLutOption::Default | VoiLutOption::First, Some(window)) => {
                                Lut::new_rescale_and_window(
                                    8,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(
                                        match self.voi_lut_function()? {
                                            Some(lut) => {
                                                if lut.len() > 1 {
                                                    lut[frame as usize]
                                                } else {
                                                    lut[0]
                                                }
                                            }
                                            None => VoiLutFunction::Linear,
                                        },
                                        if window.len() > 1 {
                                            window[frame as usize]
                                        } else {
                                            window[0]
                                        },
                                    ),
                                )
                                .context(CreateLutSnafu)?
                            }
                            (VoiLutOption::Default | VoiLutOption::First, None) => {
                                tracing::warn!("Could not find window level for object");
                                Lut::new_rescale_and_normalize(
                                    8,
                                    signed,
                                    rescale,
                                    data.iter().copied(),
                                )
                                .context(CreateLutSnafu)?
                            }
                            (VoiLutOption::Custom(window), _) => Lut::new_rescale_and_window(
                                8,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    match self.voi_lut_function()? {
                                        Some(lut) => {
                                            if lut.len() > 1 {
                                                lut[frame as usize]
                                            } else {
                                                lut[0]
                                            }
                                        }
                                        None => VoiLutFunction::Linear,
                                    },
                                    *window,
                                ),
                            )
                            .context(CreateLutSnafu)?,
                            (VoiLutOption::CustomWithFunction(window, function), _) => {
                                Lut::new_rescale_and_window(
                                    8,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(*function, *window),
                                )
                                .context(CreateLutSnafu)?
                            }
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                8,
                                signed,
                                rescale,
                                data.iter().copied(),
                            )
                            .context(CreateLutSnafu)?,
                        };

                        #[cfg(feature = "rayon")]
                        {
                            let pixel_values = lut.map_par_iter(data.par_iter().copied());
                            self.mono_image_with_extend_par(pixel_values, *bit_depth)?
                        }
                        #[cfg(not(feature = "rayon"))]
                        {
                            let pixel_values = lut.map_iter(data.iter().copied());
                            self.mono_image_with_extend(pixel_values, *bit_depth)?
                        }
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
                                bytes_to_vec_u16(&self.data[frame_start..frame_end])
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

                        self.mono_image_with_narrow(buffer.into_iter(), *bit_depth)?
                    }

                    ModalityLutOption::Default | ModalityLutOption::Override(..) => {
                        let rescale = {
                            let default = self.rescale()?;
                            if let ModalityLutOption::Override(rescale) = modality_lut {
                                *rescale
                            } else if default.len() > 1 {
                                self.rescale[frame as usize]
                            } else {
                                default[0]
                            }
                        };

                        // fetch pixel data as a slice of u16 values,
                        // irrespective of pixel signedness
                        // (that is handled by the LUT)
                        let signed = self.pixel_representation == PixelRepresentation::Signed;
                        // Note: samples are not read as `i16` even if signed,
                        // because the LUT takes care of interpreting them properly.

                        let samples = self.frame_data_ow(frame)?;

                        // use 16-bit precision to prevent possible loss of precision in image
                        let lut: Lut<u16> = match (voi_lut, self.window()?) {
                            (VoiLutOption::Identity, _) => {
                                Lut::new_rescale(self.bits_stored, signed, rescale)
                            }
                            (VoiLutOption::Default | VoiLutOption::First, Some(window)) => {
                                Lut::new_rescale_and_window(
                                    self.bits_stored,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(
                                        match self.voi_lut_function()? {
                                            Some(lut) => {
                                                if lut.len() > 1 {
                                                    lut[frame as usize]
                                                } else {
                                                    lut[0]
                                                }
                                            }
                                            None => VoiLutFunction::Linear,
                                        },
                                        if window.len() > 1 {
                                            window[frame as usize]
                                        } else {
                                            window[0]
                                        },
                                    ),
                                )
                            }
                            (VoiLutOption::Default | VoiLutOption::First, None) => {
                                tracing::warn!("Could not find window level for object");

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
                                    match self.voi_lut_function()? {
                                        Some(lut) => {
                                            if lut.len() > 1 {
                                                lut[frame as usize]
                                            } else {
                                                lut[0]
                                            }
                                        }
                                        None => VoiLutFunction::Linear,
                                    },
                                    *window,
                                ),
                            ),
                            (VoiLutOption::CustomWithFunction(window, function), _) => {
                                Lut::new_rescale_and_window(
                                    self.bits_stored,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(*function, *window),
                                )
                            }
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                self.bits_stored,
                                signed,
                                rescale,
                                samples.iter().copied(),
                            ),
                        }
                        .context(CreateLutSnafu)?;

                        #[cfg(feature = "rayon")]
                        {
                            let pixel_values = lut.map_par_iter(samples.par_iter().copied());
                            self.mono_image_with_narrow_par(pixel_values, *bit_depth)?
                        }
                        #[cfg(not(feature = "rayon"))]
                        {
                            let pixel_values = lut.map_iter(samples.iter().copied());
                            self.mono_image_with_narrow(pixel_values, *bit_depth)?
                        }
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
    /// The values are provided in standard order and layout:
    /// pixels first, then columns, then rows, then frames.
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
    pub fn to_vec<T>(&self) -> Result<Vec<T>>
    where
        T: NumCast + Send + Sync + Copy + 'static,
    {
        let mut res: Vec<T> = Vec::new();
        for frame in 0..self.number_of_frames {
            let frame_data: Vec<T> =
                self.convert_pixel_slice(self.frame_data(frame)?, frame, &Default::default())?;
            res.extend(frame_data)
        }
        Ok(res)
    }

    /// Convert all of the decoded pixel data into a vector of flat pixels
    /// of a given type `T`.
    ///
    /// The values are provided in standard order and layout:
    /// pixel first, then column, then row, with frames traversed last.
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
    pub fn to_vec_with_options<T>(&self, options: &ConvertOptions) -> Result<Vec<T>>
    where
        T: NumCast + Send + Sync + Copy + 'static,
    {
        let mut res: Vec<T> = Vec::new();
        for frame in 0..self.number_of_frames {
            let frame_data: Vec<T> =
                self.convert_pixel_slice(self.frame_data(frame)?, frame, options)?;
            res.extend(frame_data)
        }
        Ok(res)
    }

    /// Convert the decoded pixel data of a frame
    /// into a vector of flat pixels of a given type `T`.
    ///
    /// The values are provided in standard order and layout:
    /// pixels first, then columns, then rows.
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
    pub fn to_vec_frame<T>(&self, frame: u32) -> Result<Vec<T>>
    where
        T: NumCast + Send + Sync + Copy + 'static,
    {
        self.convert_pixel_slice(self.frame_data(frame)?, frame, &Default::default())
    }

    /// Convert the decoded pixel data of a frame
    /// into a vector of flat pixels of a given type `T`.
    ///
    /// The values are provided in standard order and layout:
    /// pixels first, then columns, then rows.
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
    pub fn to_vec_frame_with_options<T>(
        &self,
        frame: u32,
        options: &ConvertOptions,
    ) -> Result<Vec<T>>
    where
        T: NumCast + Send + Sync + Copy + 'static,
    {
        self.convert_pixel_slice(self.frame_data(frame)?, frame, options)
    }

    fn convert_pixel_slice<T>(
        &self,
        data: &[u8],
        frame: u32,
        options: &ConvertOptions,
    ) -> Result<Vec<T>>
    where
        T: NumCast + Send + Sync + Copy + 'static,
    {
        let ConvertOptions {
            modality_lut,
            voi_lut,
            bit_depth: _,
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
                        let rescale = {
                            let default = self.rescale()?;
                            if let ModalityLutOption::Override(rescale) = modality_lut {
                                *rescale
                            } else if default.len() > 1 {
                                default[frame as usize]
                            } else {
                                default[0]
                            }
                        };
                        let signed = self.pixel_representation == PixelRepresentation::Signed;

                        let lut: Lut<T> = match (voi_lut, self.window()?) {
                            (VoiLutOption::Default | VoiLutOption::Identity, _) => {
                                Lut::new_rescale(8, signed, rescale)
                            }
                            (VoiLutOption::First, Some(window)) => Lut::new_rescale_and_window(
                                8,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    match self.voi_lut_function()? {
                                        Some(lut) => {
                                            if lut.len() > 1 {
                                                lut[frame as usize]
                                            } else {
                                                lut[0]
                                            }
                                        }
                                        None => VoiLutFunction::Linear,
                                    },
                                    if window.len() > 1 {
                                        window[frame as usize]
                                    } else {
                                        window[0]
                                    },
                                ),
                            ),
                            (VoiLutOption::First, None) => {
                                tracing::warn!("Could not find window level for object");
                                Lut::new_rescale(8, signed, rescale)
                            }
                            (VoiLutOption::Custom(window), _) => Lut::new_rescale_and_window(
                                8,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    match self.voi_lut_function()? {
                                        Some(lut) => {
                                            if lut.len() > 1 {
                                                lut[frame as usize]
                                            } else {
                                                lut[0]
                                            }
                                        }
                                        None => VoiLutFunction::Linear,
                                    },
                                    *window,
                                ),
                            ),
                            (VoiLutOption::CustomWithFunction(window, function), _) => {
                                Lut::new_rescale_and_window(
                                    8,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(*function, *window),
                                )
                            }
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                8,
                                signed,
                                rescale,
                                data.iter().copied(),
                            ),
                        }
                        .context(CreateLutSnafu)?;

                        #[cfg(feature = "rayon")]
                        let out = lut.map_par_iter(data.par_iter().copied()).collect();

                        #[cfg(not(feature = "rayon"))]
                        let out = lut.map_iter(data.iter().copied()).collect();

                        Ok(out)
                    }
                    _ => {
                        #[cfg(feature = "rayon")]
                        // 1-channel Grayscale image
                        let converted: Result<Vec<T>, _> = data
                            .par_iter()
                            .map(|v| T::from(*v).ok_or(snafu::NoneError))
                            .collect();
                        #[cfg(not(feature = "rayon"))]
                        // 1-channel Grayscale image
                        let converted: Result<Vec<T>, _> = data
                            .iter()
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
                        let samples = bytes_to_vec_u16(data);

                        let rescale = {
                            let default = self.rescale()?;
                            if let ModalityLutOption::Override(rescale) = modality_lut {
                                *rescale
                            } else if default.len() > 1 {
                                default[frame as usize]
                            } else {
                                default[0]
                            }
                        };

                        let signed = self.pixel_representation == PixelRepresentation::Signed;

                        let lut: Lut<T> = match (voi_lut, self.window()?) {
                            (VoiLutOption::Default | VoiLutOption::Identity, _) => {
                                Lut::new_rescale(self.bits_stored, signed, rescale)
                            }
                            (VoiLutOption::First, Some(window)) => Lut::new_rescale_and_window(
                                self.bits_stored,
                                signed,
                                rescale,
                                WindowLevelTransform::new(
                                    match self.voi_lut_function()? {
                                        Some(lut) => {
                                            if lut.len() > 1 {
                                                lut[frame as usize]
                                            } else {
                                                lut[0]
                                            }
                                        }
                                        None => VoiLutFunction::Linear,
                                    },
                                    if window.len() > 1 {
                                        window[frame as usize]
                                    } else {
                                        window[0]
                                    },
                                ),
                            ),
                            (VoiLutOption::First, None) => {
                                tracing::warn!("Could not find window level for object");
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
                                    match self.voi_lut_function()? {
                                        Some(lut) => {
                                            if lut.len() > 1 {
                                                lut[frame as usize]
                                            } else {
                                                lut[0]
                                            }
                                        }
                                        None => VoiLutFunction::Linear,
                                    },
                                    *window,
                                ),
                            ),
                            (VoiLutOption::CustomWithFunction(window, function), _) => {
                                Lut::new_rescale_and_window(
                                    self.bits_stored,
                                    signed,
                                    rescale,
                                    WindowLevelTransform::new(*function, *window),
                                )
                            }
                            (VoiLutOption::Normalize, _) => Lut::new_rescale_and_normalize(
                                self.bits_stored,
                                signed,
                                rescale,
                                samples.iter().copied(),
                            ),
                        }
                        .context(CreateLutSnafu)?;

                        #[cfg(feature = "rayon")]
                        {
                            Ok(lut.map_par_iter(samples.into_par_iter()).collect())
                        }

                        #[cfg(not(feature = "rayon"))]
                        {
                            Ok(lut.map_iter(samples.into_iter()).collect())
                        }
                    }
                    _ => {
                        // no transformations
                        match self.pixel_representation {
                            // Unsigned 16 bit representation
                            PixelRepresentation::Unsigned => {
                                let dest = bytes_to_vec_u16(data);

                                #[cfg(feature = "rayon")]
                                let converted: Result<Vec<T>, _> = dest
                                    .par_iter()
                                    .map(|v| T::from(*v).ok_or(snafu::NoneError))
                                    .collect();
                                #[cfg(not(feature = "rayon"))]
                                let converted: Result<Vec<T>, _> = dest
                                    .iter()
                                    .map(|v| T::from(*v).ok_or(snafu::NoneError))
                                    .collect();
                                converted.context(InvalidDataTypeSnafu).map_err(Error::from)
                            }
                            // Signed 16 bit 2s complement representation
                            PixelRepresentation::Signed => {
                                let mut signed_buffer = vec![0; data.len() / 2];
                                NativeEndian::read_i16_into(data, &mut signed_buffer);

                                #[cfg(feature = "rayon")]
                                let converted: Result<Vec<T>, _> = signed_buffer
                                    .par_iter()
                                    .map(|v| T::from(*v).ok_or(snafu::NoneError))
                                    .collect();
                                #[cfg(not(feature = "rayon"))]
                                let converted: Result<Vec<T>, _> = signed_buffer
                                    .iter()
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
    ///
    /// The default pixel data process pipeline
    /// applies only the Modality LUT function described in the object,
    /// To change this behavior,
    /// see [`to_ndarray_with_options`](Self::to_ndarray_with_options).
    #[cfg(feature = "ndarray")]
    pub fn to_ndarray<T>(&self) -> Result<Array<T, Ix4>>
    where
        T: 'static,
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
    /// By default,
    /// only the Modality LUT function described in the object is applied.
    /// Note that certain options may be ignored
    /// if they do not apply.
    #[cfg(feature = "ndarray")]
    pub fn to_ndarray_with_options<T>(&self, options: &ConvertOptions) -> Result<Array<T, Ix4>>
    where
        T: 'static,
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
    ///
    /// The default pixel data process pipeline
    /// applies only the Modality LUT function described in the object,
    /// To change this behavior,
    /// see [`to_ndarray_frame_with_options`](Self::to_ndarray_frame_with_options).
    #[cfg(feature = "ndarray")]
    pub fn to_ndarray_frame<T>(&self, frame: u32) -> Result<Array<T, Ix3>>
    where
        T: 'static,
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
    ///
    /// The `options` value allows you to specify
    /// which transformations should be done to the pixel data
    /// (primarily Modality LUT function and VOI LUT function).
    /// By default,
    /// only the Modality LUT function described in the object is applied.
    /// Note that certain options may be ignored
    /// if they do not apply.
    #[cfg(feature = "ndarray")]
    pub fn to_ndarray_frame_with_options<T>(
        &self,
        frame: u32,
        options: &ConvertOptions,
    ) -> Result<Array<T, Ix3>>
    where
        T: 'static,
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

    /// Obtain a version of the decoded pixel data
    /// that is independent from the original DICOM object,
    /// by making copies of any necessary data.
    ///
    /// This is useful when you only need the imaging data,
    /// or when you want a composition of the object and decoded pixel data
    /// within the same value type.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dicom_object::open_file;
    /// # use dicom_pixeldata::{DecodedPixelData, PixelDecoder};
    /// # type Error = Box<dyn std::error::Error>;
    /// fn get_pixeldata_only(path: &str) -> Result<DecodedPixelData<'static>, Error> {
    ///     let obj = open_file(path)?;
    ///     let pixeldata = obj.decode_pixel_data()?;
    ///     // can freely return from function
    ///     Ok(pixeldata.to_owned())
    /// }
    /// ```
    pub fn to_owned(&self) -> DecodedPixelData<'static> {
        DecodedPixelData {
            data: Cow::Owned(self.data.to_vec()),
            bits_allocated: self.bits_allocated,
            bits_stored: self.bits_stored,
            high_bit: self.high_bit,
            pixel_representation: self.pixel_representation,
            photometric_interpretation: self.photometric_interpretation.clone(),
            planar_configuration: self.planar_configuration,
            number_of_frames: self.number_of_frames,
            rows: self.rows,
            cols: self.cols,
            samples_per_pixel: self.samples_per_pixel,
            rescale: self.rescale.to_vec(),
            voi_lut_function: self.voi_lut_function.clone(),
            window: self.window.clone(),
            enforce_frame_fg_vm_match: self.enforce_frame_fg_vm_match,
        }
    }
}

fn bytes_to_vec_u16(data: &[u8]) -> Vec<u16> {
    debug_assert!(data.len() % 2 == 0);
    let mut pixel_array: Vec<u16> = vec![0; data.len() / 2];
    NativeEndian::read_u16_into(data, &mut pixel_array);
    pixel_array
}

// Convert u8 pixel array from YBR_FULL or YBR_FULL_422 to RGB
// Every pixel is replaced with an RGB value
#[cfg(feature = "image")]
fn convert_colorspace_u8(i: &mut [u8]) {
    #[cfg(feature = "rayon")]
    let iter = i.par_chunks_mut(3);
    #[cfg(not(feature = "rayon"))]
    let iter = i.chunks_mut(3);

    // Matrix multiplication taken from
    // https://github.com/pydicom/pydicom/blob/f36517e10/pydicom/pixel_data_handlers/util.py#L576
    iter.for_each(|pixel| {
        let y = pixel[0] as f32;
        let b: f32 = pixel[1] as f32;
        let r: f32 = pixel[2] as f32;
        let b = b - 128.0;
        let r = r - 128.0;

        let cr = (y + 1.402 * r) + 0.5;
        let cg = (y + (0.114 * 1.772 / 0.587) * b + (-0.299 * 1.402 / 0.587) * r) + 0.5;
        let cb = (y + 1.772 * b) + 0.5;

        let cr = cr.floor().clamp(0.0, u8::MAX as f32) as u8;
        let cg = cg.floor().clamp(0.0, u8::MAX as f32) as u8;
        let cb = cb.floor().clamp(0.0, u8::MAX as f32) as u8;

        pixel[0] = cr;
        pixel[1] = cg;
        pixel[2] = cb;
    });
}

#[cfg(feature = "image")]
fn interleave<T: Copy>(data: &[T]) -> Vec<T> {
    debug_assert_eq!(data.len() % 3, 0);
    let component_len = data.len() / 3;
    let r = &data[..component_len];
    let g = &data[component_len..2 * component_len];
    let b = &data[2 * component_len..];
    r.iter()
        .zip(g.iter())
        .zip(b.iter())
        .flat_map(|((r, g), b)| [*r, *g, *b])
        .collect()
}

// Convert u16 pixel array from YBR_FULL or YBR_FULL_422 to RGB
// Every pixel is replaced with an RGB value
#[cfg(feature = "image")]
fn convert_colorspace_u16(i: &mut [u16]) {
    #[cfg(feature = "rayon")]
    let iter = i.par_chunks_mut(3);
    #[cfg(not(feature = "rayon"))]
    let iter = i.chunks_mut(3);

    // Matrix multiplication taken from
    // https://github.com/pydicom/pydicom/blob/f36517e10/pydicom/pixel_data_handlers/util.py#L576
    iter.for_each(|pixel| {
        let y = pixel[0] as f32;
        let b: f32 = pixel[1] as f32;
        let r: f32 = pixel[2] as f32;
        let b = b - 32768.0;
        let r = r - 32768.0;

        let cr = (y + 1.402 * r) + 0.5;
        let cg = (y + (0.114 * 1.772 / 0.587) * b + (-0.299 * 1.402 / 0.587) * r) + 0.5;
        let cb = (y + 1.772 * b) + 0.5;

        let cr = cr.floor().clamp(0.0, u16::MAX as f32) as u16;
        let cg = cg.floor().clamp(0.0, u16::MAX as f32) as u16;
        let cb = cb.floor().clamp(0.0, u16::MAX as f32) as u16;

        pixel[0] = cr;
        pixel[1] = cg;
        pixel[2] = cb;
    });
}

/// Convert the i16 vector by shifting it up,
/// thus maintaining the order between sample values.
#[cfg(feature = "image")]
fn convert_i16_to_u16(i: &[i16]) -> Vec<u16> {
    #[cfg(feature = "rayon")]
    let iter = i.par_iter();
    #[cfg(not(feature = "rayon"))]
    let iter = i.iter();
    iter.map(|p| (*p as i32 + 0x8000) as u16).collect()
}

/// Trait for objects which can be decoded into
/// blobs of easily consumable pixel data.
///
/// This is the main trait which extends the capability of DICOM objects
/// (such as [`DefaultDicomObject`](dicom_object::DefaultDicomObject) from [`dicom_object`])
/// with a pathway to retrieve the imaging data.
///
/// See examples of use in the [root crate documentation](crate).
pub trait PixelDecoder {
    /// Decode the full pixel data in this object,
    /// yielding a base set of imaging properties
    /// and pixel data in native form.
    ///
    /// The resulting pixel data will be tied to
    /// the original object's lifetime.
    /// In the event that the pixel data is in an encapsulated form,
    /// new byte buffers are allocated for holding their native form.
    fn decode_pixel_data(&self) -> Result<DecodedPixelData<'_>>;

    /// Decode the pixel data of a single frame in this object,
    /// yielding a base set of imaging properties
    /// and pixel data in native form.
    ///
    /// The resulting pixel data will be tied to
    /// the original object's lifetime.
    /// In the event that the pixel data is in an encapsulated form,
    /// new byte buffers are allocated for holding their native form.
    /// The number of frames recorded will be always 1,
    /// and the existence of other frames is ignored.
    /// When calling single frame retrieval methods afterwards,
    /// such as [`to_vec_frame`](DecodedPixelData::to_vec_frame),
    /// assume the intended frame number to be `0`.
    ///
    /// ---
    ///
    /// The default implementation decodes the full pixel data
    /// and then provides a crop containing only the frame of interest.
    /// Implementers are advised to write their own implementation for efficiency.
    fn decode_pixel_data_frame(&self, frame: u32) -> Result<DecodedPixelData<'_>> {
        let mut px = self.decode_pixel_data()?;

        // calculate frame offset and size
        let frame_size = ((px.bits_allocated + 7) / 8) as usize
            * px.samples_per_pixel as usize
            * px.rows as usize
            * px.cols as usize;
        let frame_offset = frame_size * frame as usize;

        // crop to frame
        match &mut px.data {
            Cow::Owned(data) => *data = data[frame_offset..frame_offset + frame_size].to_vec(),
            Cow::Borrowed(data) => {
                *data = &data[frame_offset..frame_offset + frame_size];
            }
        }

        // reset number of frames
        px.number_of_frames = 1;

        Ok(px)
    }
}

/// Aggregator of key properties for imaging data,
/// without the pixel data proper.
///
/// Currently kept private,
/// might become part of the public API in the future.
#[derive(Debug)]
#[cfg(not(feature = "gdcm"))]
pub(crate) struct ImagingProperties {
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    pub(crate) samples_per_pixel: u16,
    pub(crate) bits_allocated: u16,
    pub(crate) bits_stored: u16,
    pub(crate) high_bit: u16,
    pub(crate) pixel_representation: PixelRepresentation,
    pub(crate) planar_configuration: PlanarConfiguration,
    pub(crate) photometric_interpretation: PhotometricInterpretation,
    pub(crate) rescale_intercept: Vec<f64>,
    pub(crate) rescale_slope: Vec<f64>,
    pub(crate) number_of_frames: u32,
    pub(crate) voi_lut_function: Option<Vec<VoiLutFunction>>,
    pub(crate) window: Option<Vec<WindowLevel>>,
}

#[cfg(not(feature = "gdcm"))]
impl ImagingProperties {
    fn from_obj<D>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<Self>
    where
        D: Clone + DataDictionary,
    {
        use attribute::*;
        use std::convert::TryFrom;

        let cols = cols(obj).context(GetAttributeSnafu)?;
        let rows = rows(obj).context(GetAttributeSnafu)?;
        let photometric_interpretation =
            photometric_interpretation(obj).context(GetAttributeSnafu)?;
        let samples_per_pixel = samples_per_pixel(obj).context(GetAttributeSnafu)?;
        let planar_configuration = planar_configuration(obj).context(GetAttributeSnafu)?;
        let bits_allocated = bits_allocated(obj).context(GetAttributeSnafu)?;
        let bits_stored = bits_stored(obj).context(GetAttributeSnafu)?;
        let high_bit = high_bit(obj).context(GetAttributeSnafu)?;
        let pixel_representation = pixel_representation(obj).context(GetAttributeSnafu)?;
        let rescale_intercept = rescale_intercept(obj);
        let rescale_slope = rescale_slope(obj);
        let number_of_frames = number_of_frames(obj).context(GetAttributeSnafu)?;
        let voi_lut_function = voi_lut_function(obj).context(GetAttributeSnafu)?;
        let voi_lut_function: Option<Vec<VoiLutFunction>> = voi_lut_function.and_then(|fns| {
            fns.iter()
                .map(|v| VoiLutFunction::try_from((*v).as_str()).ok())
                .collect()
        });

        ensure!(
            rescale_intercept.len() == rescale_slope.len(),
            LengthMismatchRescaleSnafu {
                slope_vm: rescale_slope.len() as u32,
                intercept_vm: rescale_intercept.len() as u32,
            }
        );

        let window = if let Some(wcs) = window_center(obj) {
            let width = window_width(obj);
            if let Some(wws) = width {
                ensure!(
                    wcs.len() == wws.len(),
                    LengthMismatchWindowLevelSnafu {
                        wc_vm: wcs.len() as u32,
                        ww_vm: wws.len() as u32,
                    }
                );
                Some(
                    zip(wcs, wws)
                        .map(|(wc, ww)| WindowLevel {
                            center: wc,
                            width: ww,
                        })
                        .collect(),
                )
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            cols,
            rows,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            planar_configuration,
            photometric_interpretation,
            rescale_intercept,
            rescale_slope,
            number_of_frames,
            voi_lut_function,
            window,
        })
    }
}

#[cfg(not(feature = "gdcm"))]
impl<D> PixelDecoder for FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary + Clone,
{
    fn decode_pixel_data(&self) -> Result<DecodedPixelData> {
        let pixel_data = attribute::pixel_data(self).context(GetAttributeSnafu)?;

        let ImagingProperties {
            cols,
            rows,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            planar_configuration,
            photometric_interpretation,
            rescale_intercept,
            rescale_slope,
            number_of_frames,
            voi_lut_function,
            window,
        } = ImagingProperties::from_obj(self)?;

        let transfer_syntax = &self.meta().transfer_syntax;
        let ts = TransferSyntaxRegistry
            .get(transfer_syntax)
            .with_context(|| UnknownTransferSyntaxSnafu {
                ts_uid: transfer_syntax,
            })?;

        if !ts.can_decode_all() {
            return UnsupportedTransferSyntaxSnafu {
                ts: transfer_syntax,
            }
            .fail()?;
        }

        let rescale = zip(&rescale_intercept, &rescale_slope)
            .map(|(intercept, slope)| Rescale {
                intercept: *intercept,
                slope: *slope,
            })
            .collect();

        // Try decoding it using a registered pixel data decoder
        if let Codec::EncapsulatedPixelData(Some(decoder), _) = ts.codec() {
            let mut data: Vec<u8> = Vec::new();
            (*decoder)
                .decode(self, &mut data)
                .context(DecodePixelDataSnafu)?;

            // pixels are already interpreted,
            // set new photometric interpretation if necessary
            let new_pi = match samples_per_pixel {
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
                rescale,
                voi_lut_function,
                window,
                enforce_frame_fg_vm_match: false,
            });
        }

        let decoded_pixel_data = match pixel_data.value() {
            DicomValue::PixelSequence(v) => {
                // Return all fragments concatenated
                // (should only happen for Encapsulated Uncompressed)
                v.fragments().iter().flatten().copied().collect()
            }
            DicomValue::Primitive(p) => {
                // Non-encoded, just return the pixel data for all frames
                p.to_bytes().to_vec()
            }
            DicomValue::Sequence(..) => InvalidPixelDataSnafu.fail()?,
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
            rescale,
            voi_lut_function,
            window,
            enforce_frame_fg_vm_match: false,
        })
    }

    fn decode_pixel_data_frame(&self, frame: u32) -> Result<DecodedPixelData<'_>> {
        let pixel_data = attribute::pixel_data(self).context(GetAttributeSnafu)?;

        let ImagingProperties {
            cols,
            rows,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            planar_configuration,
            photometric_interpretation,
            rescale_intercept,
            rescale_slope,
            number_of_frames,
            voi_lut_function,
            window,
        } = ImagingProperties::from_obj(self)?;

        let transfer_syntax = &self.meta().transfer_syntax;
        let ts = TransferSyntaxRegistry
            .get(transfer_syntax)
            .with_context(|| UnknownTransferSyntaxSnafu {
                ts_uid: transfer_syntax,
            })?;

        if !ts.can_decode_all() {
            return UnsupportedTransferSyntaxSnafu {
                ts: transfer_syntax,
            }
            .fail()?;
        }

        let rescale_data = zip(&rescale_intercept, &rescale_slope)
            .map(|(intercept, slope)| Rescale {
                intercept: *intercept,
                slope: *slope,
            })
            .collect::<Vec<Rescale>>();

        let rescale = rescale_data
            .get(frame as usize)
            .or(rescale_data.first())
            .copied()
            .map(|inner| vec![inner])
            .unwrap_or_default();

        let window = window
            .and_then(|inner| {
                inner
                    .get(frame as usize)
                    .or(inner.first())
                    .copied()
                    .map(|el| vec![el])
            });

        let voi_lut_function = voi_lut_function
            .and_then(|inner| {
                inner
                    .get(frame as usize)
                    .or(inner.first())
                    .copied()
                    .map(|el| vec![el])
            });

        // Try decoding it using a registered pixel data decoder
        if let Codec::EncapsulatedPixelData(Some(decoder), _) = ts.codec() {
            let mut data: Vec<u8> = Vec::new();
            (*decoder)
                .decode_frame(self, frame, &mut data)
                .context(DecodePixelDataSnafu)?;

            // pixels are already interpreted,
            // set new photometric interpretation if necessary
            let new_pi = match samples_per_pixel {
                3 => PhotometricInterpretation::Rgb,
                _ => photometric_interpretation,
            };

            return Ok(DecodedPixelData {
                data: Cow::from(data),
                cols: cols.into(),
                rows: rows.into(),
                number_of_frames: 1,
                photometric_interpretation: new_pi,
                samples_per_pixel,
                planar_configuration: PlanarConfiguration::Standard,
                bits_allocated,
                bits_stored,
                high_bit,
                pixel_representation,
                rescale,
                voi_lut_function,
                window,
                enforce_frame_fg_vm_match: false,
            });
        }

        let decoded_pixel_data = match pixel_data.value() {
            DicomValue::PixelSequence(v) => {
                let fragments = v.fragments();
                if number_of_frames as usize == fragments.len() {
                    // return a single fragment
                    fragments[frame as usize].to_vec()
                } else {
                    // not supported, return an error
                    InvalidPixelDataSnafu.fail()?
                }
            }
            DicomValue::Primitive(p) => {
                // Non-encoded, just return the pixel data for a single frame
                let frame_size = ((bits_allocated + 7) / 8) as usize
                    * samples_per_pixel as usize
                    * rows as usize
                    * cols as usize;
                let frame_offset = frame_size * frame as usize;
                let data = p.to_bytes();
                data.get(frame_offset..frame_offset + frame_size)
                    .with_context(|| FrameOutOfRangeSnafu {
                        frame_number: frame,
                    })?
                    .to_vec()
            }
            DicomValue::Sequence(..) => InvalidPixelDataSnafu.fail()?,
        };

        Ok(DecodedPixelData {
            data: Cow::from(decoded_pixel_data),
            cols: cols.into(),
            rows: rows.into(),
            number_of_frames: 1,
            photometric_interpretation,
            samples_per_pixel,
            planar_configuration,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            rescale,
            voi_lut_function,
            window,
            enforce_frame_fg_vm_match: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_object::open_file;

    fn is_send_and_sync<T>()
    where
        T: Send + Sync,
    {
    }

    #[test]
    fn error_is_send_and_sync() {
        is_send_and_sync::<Error>();
    }

    #[test]
    fn test_to_vec_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_16bit.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let decoded = obj.decode_pixel_data().unwrap();

        let rows = decoded.rows();

        let values = decoded.to_vec::<u16>().unwrap();
        assert_eq!(values.len(), 30000);

        // 50, 80, 1
        assert_eq!(values[50 * rows as usize * 3 + 80 * 3 + 1], 32896);
    }

    #[test]
    #[cfg(feature = "ndarray")]
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

    /// to_ndarray fails if the target type cannot represent the transformed values
    #[cfg(feature = "ndarray")]
    #[test]
    fn test_to_ndarray_error() {
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        assert!(matches!(
            obj.decode_pixel_data().unwrap().to_ndarray::<u8>(),
            Err(Error(InnerError::InvalidDataType { .. }))
                | Err(Error(InnerError::CreateLut { .. }))
        ));
    }

    /// conversion to ndarray in 16-bit
    /// retains the original data of a 16-bit image
    #[cfg(feature = "ndarray")]
    #[test]
    fn test_to_ndarray_16bit() {
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();

        let decoded = obj.decode_pixel_data().unwrap();
        let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
        let ndarray = decoded.to_ndarray_with_options::<u16>(&options).unwrap();

        assert_eq!(ndarray.shape(), &[1, 128, 128, 1]);

        // sample value retrieved from the original image file
        assert_eq!(ndarray[[0, 127, 127, 0]], 0x038D);
    }

    /// conversion of a 16-bit image to a vector of 16-bit processed pixel values
    /// takes advantage of the output's full spectrum
    #[test]
    fn test_to_vec_16bit_to_window() {
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();

        let decoded = obj.decode_pixel_data().unwrap();
        let options = ConvertOptions::new()
            .with_modality_lut(ModalityLutOption::Default)
            .with_voi_lut(VoiLutOption::First);
        let values = decoded.to_vec_with_options::<u16>(&options).unwrap();

        assert_eq!(values.len(), 128 * 128);

        // values are in the full spectrum

        let max = values.iter().max().unwrap();
        let min = values.iter().min().unwrap();

        assert_eq!(*max, 0xFFFF, "maximum in window should be 65535");
        assert_eq!(*min, 0, "minimum in window should be 0");
    }

    #[test]
    fn test_correct_ri_extracted() {
        // Rescale Slope and Intercept exist for this scan
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(pixel_data.rescale().unwrap()[0], Rescale::new(1., -1024.));
    }

    #[test]
    fn test_correct_rescale_extracted_without_element() {
        // RescaleIntercept does not exists for this scan
        let test_file = dicom_test_files::path("pydicom/MR_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(pixel_data.rescale().unwrap()[0], Rescale::new(1., 0.));
    }

    #[test]
    fn test_general_properties_from_16bit() {
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();

        assert_eq!(pixel_data.columns(), 128, "Unexpected Columns");
        assert_eq!(pixel_data.rows(), 128, "Unexpected Rows");
        assert_eq!(
            pixel_data.number_of_frames(),
            1,
            "Unexpected Number of Frames"
        );
        assert_eq!(
            pixel_data.photometric_interpretation(),
            &PhotometricInterpretation::Monochrome2,
            "Unexpected Photometric Interpretation"
        );
        assert_eq!(
            pixel_data.samples_per_pixel(),
            1,
            "Unexpected Samples per Pixel"
        );
        assert_eq!(pixel_data.bits_allocated(), 16, "Unexpected Bits Allocated");
        assert_eq!(pixel_data.bits_stored(), 16, "Unexpected Bits Stored");
        assert_eq!(pixel_data.high_bit(), 15, "Unexpected High Bit");
        assert_eq!(
            pixel_data.pixel_representation(),
            PixelRepresentation::Signed
        );
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_force_bit_depth_from_16bit() {
        let test_file = dicom_test_files::path("pydicom/CT_small.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();

        // original image has 16 bits stored
        {
            let image = pixel_data
                .to_dynamic_image(0)
                .expect("Failed to convert to image");

            assert!(image.as_luma16().is_some());
        }

        // force to 16 bits
        {
            let options = ConvertOptions::new().force_16bit();
            let image = pixel_data
                .to_dynamic_image_with_options(0, &options)
                .expect("Failed to convert to image");

            assert!(image.as_luma16().is_some());
        }

        // force to 8 bits
        {
            let options = ConvertOptions::new().force_8bit();
            let image = pixel_data
                .to_dynamic_image_with_options(0, &options)
                .expect("Failed to convert to image");

            assert!(image.as_luma8().is_some());
        }
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_force_bit_depth_from_rgb() {
        let test_file = dicom_test_files::path("pydicom/color-px.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();

        // original image is RGB with 8 bits per sample
        {
            let image = pixel_data
                .to_dynamic_image(0)
                .expect("Failed to convert to image");

            assert!(image.as_rgb8().is_some());
        }

        // force to 16 bits
        {
            let options = ConvertOptions::new().force_16bit();
            let image = pixel_data
                .to_dynamic_image_with_options(0, &options)
                .expect("Failed to convert to image");

            assert!(image.as_rgb16().is_some());
        }

        // force to 8 bits
        {
            let options = ConvertOptions::new().force_8bit();
            let image = pixel_data
                .to_dynamic_image_with_options(0, &options)
                .expect("Failed to convert to image");

            assert!(image.as_rgb8().is_some());
        }
    }

    #[cfg(feature = "image")]
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

    #[test]
    #[ignore = "test is unsound"]
    fn test_can_read_deflated(){
        
        let path = dicom_test_files::path("pydicom/image_dfl.dcm").expect("test DICOM file should exist");
    
        // should read preamble even though it's from a reader
        let obj = open_file(path.clone()).expect("Should read file");
    
        let res = obj.decode_pixel_data().expect("Should decode pixel data.");
        assert_eq!(res.to_vec::<u8>().unwrap().len(), (res.rows() as usize * res.columns() as usize));
        let mut buf = Vec::<u8>::new();
        obj.write_all(&mut buf).expect("Should write deflated");

        assert_eq!(std::fs::metadata(path).unwrap().len() as usize, buf.len())
    }

    #[cfg(not(feature = "gdcm"))]
    mod not_gdcm {
        #[cfg(feature = "ndarray")]
        use crate::PixelDecoder;
        #[cfg(any(feature = "rle", feature = "image"))]
        #[cfg(feature = "image")]
        use rstest::rstest;

        #[cfg(feature = "rle")]
        #[test]
        fn test_native_decoding_pixel_data_rle_8bit_1frame_vec() {
            use crate::{ConvertOptions, ModalityLutOption, PixelDecoder as _};

            let path = dicom_test_files::path("pydicom/SC_rgb_rle.dcm")
                .expect("test DICOM file should exist");
            let object = dicom_object::open_file(&path).unwrap();

            let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
            let decoded = object.decode_pixel_data().unwrap();
            let values = decoded.to_vec_with_options::<u8>(&options).unwrap();

            let columns = decoded.columns() as usize;
            // validated through manual inspection of ground-truth
            assert_eq!(values.len(), 30_000);
            // 0,0,r
            assert_eq!(values[0], 255);
            // 0,0,g
            assert_eq!(values[1], 0);
            // 0,0,b
            assert_eq!(values[2], 0);
            // 50,50,r
            assert_eq!(values[50 * columns * 3 + 50 * 3], 128);
            // 50,50,g
            assert_eq!(values[50 * columns * 3 + 50 * 3 + 1], 128);
            // 50,50,b
            assert_eq!(values[50 * columns * 3 + 50 * 3 + 2], 255);
            // 75,75,r
            assert_eq!(values[75 * columns * 3 + 75 * 3], 64);
            // 75,75,g
            assert_eq!(values[75 * columns * 3 + 75 * 3 + 1], 64);
            // 75,75,b
            assert_eq!(values[75 * columns * 3 + 75 * 3 + 2], 64);
            // 16,49,r
            assert_eq!(values[49 * columns * 3 + 16 * 3], 0);
            // 16,49,g
            assert_eq!(values[49 * columns * 3 + 16 * 3 + 1], 0);
            // 16,49,b
            assert_eq!(values[49 * columns * 3 + 16 * 3 + 2], 255);
        }

        #[cfg(feature = "ndarray")]
        #[test]
        fn test_native_decoding_pixel_data_rle_8bit_1frame_ndarray() {
            use crate::{ConvertOptions, ModalityLutOption};

            let path = dicom_test_files::path("pydicom/SC_rgb_rle.dcm")
                .expect("test DICOM file should exist");
            let object = dicom_object::open_file(&path).unwrap();

            let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
            let ndarray = object
                .decode_pixel_data()
                .unwrap()
                .to_ndarray_with_options::<u8>(&options)
                .unwrap();
            // validated through manual inspection of ground-truth
            assert_eq!(ndarray.shape(), &[1, 100, 100, 3]);
            assert_eq!(ndarray.len(), 30_000);
            // 0, 0
            assert_eq!(ndarray[[0, 0, 0, 0]], 255);
            assert_eq!(ndarray[[0, 0, 0, 1]], 0);
            assert_eq!(ndarray[[0, 0, 0, 2]], 0);
            // 50, 50
            assert_eq!(ndarray[[0, 50, 50, 0]], 128);
            assert_eq!(ndarray[[0, 50, 50, 1]], 128);
            assert_eq!(ndarray[[0, 50, 50, 2]], 255);
            // 75, 75
            assert_eq!(ndarray[[0, 75, 75, 0]], 64);
            assert_eq!(ndarray[[0, 75, 75, 1]], 64);
            assert_eq!(ndarray[[0, 75, 75, 2]], 64);
            // 16, 49
            assert_eq!(ndarray[[0, 49, 16, 0]], 0);
            assert_eq!(ndarray[[0, 49, 16, 1]], 0);
            assert_eq!(ndarray[[0, 49, 16, 2]], 255);
        }

        #[cfg(feature = "ndarray")]
        #[test]
        fn test_native_decoding_pixel_data_rle_8bit_2frame() {
            use crate::{ConvertOptions, ModalityLutOption};

            let path = dicom_test_files::path("pydicom/SC_rgb_rle_2frame.dcm")
                .expect("test DICOM file should exist");
            let object = dicom_object::open_file(&path).unwrap();
            let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
            let ndarray = object
                .decode_pixel_data()
                .unwrap()
                .to_ndarray_with_options::<u8>(&options)
                .unwrap();
            // validated through manual inspection of ground-truth
            assert_eq!(ndarray.shape(), &[2, 100, 100, 3]);
            assert_eq!(ndarray.len(), 60_000);
            // 0, 0
            assert_eq!(ndarray[[0, 0, 0, 0]], 255);
            assert_eq!(ndarray[[0, 0, 0, 1]], 0);
            assert_eq!(ndarray[[0, 0, 0, 2]], 0);
            // 50, 50
            assert_eq!(ndarray[[0, 50, 50, 0]], 128);
            assert_eq!(ndarray[[0, 50, 50, 1]], 128);
            assert_eq!(ndarray[[0, 50, 50, 2]], 255);
            // 75, 75
            assert_eq!(ndarray[[0, 75, 75, 0]], 64);
            assert_eq!(ndarray[[0, 75, 75, 1]], 64);
            assert_eq!(ndarray[[0, 75, 75, 2]], 64);
            // 16, 49
            assert_eq!(ndarray[[0, 49, 16, 0]], 0);
            assert_eq!(ndarray[[0, 49, 16, 1]], 0);
            assert_eq!(ndarray[[0, 49, 16, 2]], 255);
            // The second frame is the inverse of the first frame
            // 0, 0
            assert_eq!(ndarray[[1, 0, 0, 0]], 0);
            assert_eq!(ndarray[[1, 0, 0, 1]], 255);
            assert_eq!(ndarray[[1, 0, 0, 2]], 255);
            // 50, 50
            assert_eq!(ndarray[[1, 50, 50, 0]], 127);
            assert_eq!(ndarray[[1, 50, 50, 1]], 127);
            assert_eq!(ndarray[[1, 50, 50, 2]], 0);
            // 75, 75
            assert_eq!(ndarray[[1, 75, 75, 0]], 191);
            assert_eq!(ndarray[[1, 75, 75, 1]], 191);
            assert_eq!(ndarray[[1, 75, 75, 2]], 191);
            // 16, 49
            assert_eq!(ndarray[[1, 49, 16, 0]], 255);
            assert_eq!(ndarray[[1, 49, 16, 1]], 255);
            assert_eq!(ndarray[[1, 49, 16, 2]], 0);
        }

        #[cfg(feature = "ndarray")]
        #[test]
        fn test_native_decoding_pixel_data_rle_16bit_1frame() {
            use crate::{ConvertOptions, ModalityLutOption};

            let path = dicom_test_files::path("pydicom/SC_rgb_rle_16bit.dcm")
                .expect("test DICOM file should exist");
            let object = dicom_object::open_file(&path).unwrap();
            let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
            let ndarray = object
                .decode_pixel_data()
                .unwrap()
                .to_ndarray_with_options::<u16>(&options)
                .unwrap();
            assert_eq!(ndarray.shape(), &[1, 100, 100, 3]);
            assert_eq!(ndarray.len(), 30_000);
            // 0,0
            assert_eq!(ndarray[[0, 0, 0, 0]], 65535);
            assert_eq!(ndarray[[0, 0, 0, 1]], 0);
            assert_eq!(ndarray[[0, 0, 0, 2]], 0);
            // 50,50
            assert_eq!(ndarray[[0, 50, 50, 0]], 32896);
            assert_eq!(ndarray[[0, 50, 50, 1]], 32896);
            assert_eq!(ndarray[[0, 50, 50, 2]], 65535);
            // 75,75
            assert_eq!(ndarray[[0, 75, 75, 0]], 16448);
            assert_eq!(ndarray[[0, 75, 75, 1]], 16448);
            assert_eq!(ndarray[[0, 75, 75, 2]], 16448);
            // 16, 49
            assert_eq!(ndarray[[0, 49, 16, 0]], 0);
            assert_eq!(ndarray[[0, 49, 16, 1]], 0);
            assert_eq!(ndarray[[0, 49, 16, 2]], 65535);
        }

        #[cfg(feature = "ndarray")]
        #[test]
        fn test_native_decoding_pixel_data_rle_16bit_2frame() {
            let path = dicom_test_files::path("pydicom/SC_rgb_rle_16bit_2frame.dcm")
                .expect("test DICOM file should exist");
            let object = dicom_object::open_file(&path).unwrap();
            let ndarray = object
                .decode_pixel_data()
                .unwrap()
                .to_ndarray::<u16>()
                .unwrap();
            // Validated using Numpy
            // This doesn't reshape the array based on the PlanarConfiguration
            // So for this scan the pixel layout is [Rlsb..Rmsb, Glsb..Gmsb, Blsb..msb]
            assert_eq!(ndarray.shape(), &[2, 100, 100, 3]);
            assert_eq!(ndarray.len(), 60_000);
            // 0,0
            assert_eq!(ndarray[[0, 0, 0, 0]], 65535);
            assert_eq!(ndarray[[0, 0, 0, 1]], 0);
            assert_eq!(ndarray[[0, 0, 0, 2]], 0);
            // 50,50
            assert_eq!(ndarray[[0, 50, 50, 0]], 32896);
            assert_eq!(ndarray[[0, 50, 50, 1]], 32896);
            assert_eq!(ndarray[[0, 50, 50, 2]], 65535);
            // 75,75
            assert_eq!(ndarray[[0, 75, 75, 0]], 16448);
            assert_eq!(ndarray[[0, 75, 75, 1]], 16448);
            assert_eq!(ndarray[[0, 75, 75, 2]], 16448);
            // 16, 49
            assert_eq!(ndarray[[0, 49, 16, 0]], 0);
            assert_eq!(ndarray[[0, 49, 16, 1]], 0);
            assert_eq!(ndarray[[0, 49, 16, 2]], 65535);
            // The second frame is the inverse of the first frame
            // 0,0
            assert_eq!(ndarray[[1, 0, 0, 0]], 0);
            assert_eq!(ndarray[[1, 0, 0, 1]], 65535);
            assert_eq!(ndarray[[1, 0, 0, 2]], 65535);
            // 50,50
            assert_eq!(ndarray[[1, 50, 50, 0]], 32639);
            assert_eq!(ndarray[[1, 50, 50, 1]], 32639);
            assert_eq!(ndarray[[1, 50, 50, 2]], 0);
            // 75,75
            assert_eq!(ndarray[[1, 75, 75, 0]], 49087);
            assert_eq!(ndarray[[1, 75, 75, 1]], 49087);
            assert_eq!(ndarray[[1, 75, 75, 2]], 49087);
            // 16, 49
            assert_eq!(ndarray[[1, 49, 16, 0]], 65535);
            assert_eq!(ndarray[[1, 49, 16, 1]], 65535);
            assert_eq!(ndarray[[1, 49, 16, 2]], 0);
        }

        #[cfg(feature = "image")]
        const MAX_TEST_FRAMES: u32 = 16;

        #[cfg(feature = "image")]
        #[rstest]
        // jpeg2000 encoding
        #[cfg_attr(
            any(feature = "openjp2", feature = "openjpeg-sys"),
            case("pydicom/emri_small_jpeg_2k_lossless.dcm", 10)
        )]
        #[cfg_attr(
            any(feature = "openjp2", feature = "openjpeg-sys"),
            case("pydicom/693_J2KI.dcm", 1)
        )]
        #[cfg_attr(
            any(feature = "openjp2", feature = "openjpeg-sys"),
            case("pydicom/693_J2KR.dcm", 1)
        )]
        #[cfg_attr(
            any(feature = "openjp2", feature = "openjpeg-sys"),
            case("pydicom/JPEG2000.dcm", 1)
        )]
        //
        // jpeg-ls encoding
        #[cfg_attr(
            feature = "charls",
            case("pydicom/emri_small_jpeg_ls_lossless.dcm", 10)
        )]
        #[cfg_attr(feature = "charls", case("pydicom/MR_small_jpeg_ls_lossless.dcm", 1))]
        //
        // sample precision of 12 not supported yet
        #[should_panic(expected = "Unsupported(SamplePrecision(12))")]
        #[case("pydicom/JPEG-lossy.dcm", 1)]
        //
        // JPEG baseline (8bit)
        #[cfg_attr(feature = "jpeg", case("pydicom/color3d_jpeg_baseline.dcm", 120))]
        #[cfg_attr(feature = "jpeg", case("pydicom/SC_rgb_jpeg_lossy_gdcm.dcm", 1))]
        #[cfg_attr(feature = "jpeg", case("pydicom/SC_rgb_jpeg_gdcm.dcm", 1))]
        //
        // JPEG lossless
        #[cfg_attr(feature = "jpeg", case("pydicom/JPEG-LL.dcm", 1))]
        #[cfg_attr(feature = "jpeg", case("pydicom/JPGLosslessP14SV1_1s_1f_8b.dcm", 1))]

        fn test_parse_jpeg_encoded_dicom_pixel_data(#[case] value: &str, #[case] frames: u32) {
            use crate::PixelDecoder as _;
            use std::fs;
            use std::path::Path;

            let test_file = dicom_test_files::path(value).unwrap();
            println!("Parsing pixel data for {}", test_file.display());
            let obj = dicom_object::open_file(test_file).unwrap();
            let pixel_data = obj.decode_pixel_data().unwrap();
            assert_eq!(
                pixel_data.number_of_frames(),
                frames,
                "number of frames mismatch"
            );

            let output_dir = Path::new(
                "../target/dicom_test_files/_out/test_parse_jpeg_encoded_dicom_pixel_data",
            );
            fs::create_dir_all(output_dir).unwrap();

            for i in 0..pixel_data.number_of_frames().min(MAX_TEST_FRAMES) {
                let image = pixel_data
                    .to_dynamic_image(i)
                    .expect("failed to retrieve the frame requested");
                let image_path = output_dir.join(format!(
                    "{}-{}.png",
                    Path::new(value).file_stem().unwrap().to_str().unwrap(),
                    i,
                ));
                image.save(image_path).unwrap();
            }
        }

        #[cfg(feature = "image")]
        #[rstest]
        #[cfg_attr(feature = "jpeg", case("pydicom/color3d_jpeg_baseline.dcm", 0))]
        #[cfg_attr(feature = "jpeg", case("pydicom/color3d_jpeg_baseline.dcm", 1))]
        #[cfg_attr(feature = "jpeg", case("pydicom/color3d_jpeg_baseline.dcm", 78))]
        #[cfg_attr(feature = "jpeg", case("pydicom/color3d_jpeg_baseline.dcm", 119))]
        #[case("pydicom/SC_rgb_rle_2frame.dcm", 0)]
        #[case("pydicom/SC_rgb_rle_2frame.dcm", 1)]
        #[case("pydicom/JPEG2000_UNC.dcm", 0)]
        #[cfg_attr(feature = "charls", case("pydicom/emri_small_jpeg_ls_lossless.dcm", 5))]
        #[cfg_attr(feature = "charls", case("pydicom/MR_small_jpeg_ls_lossless.dcm", 0))]
        fn test_decode_pixel_data_individual_frames(#[case] value: &str, #[case] frame: u32) {
            use crate::PixelDecoder as _;
            use std::path::Path;

            let test_file = dicom_test_files::path(value).unwrap();
            println!("Parsing pixel data for {}", test_file.display());
            let obj = dicom_object::open_file(test_file).unwrap();
            let pixel_data = obj.decode_pixel_data_frame(frame).unwrap();
            let output_dir = Path::new(
                "../target/dicom_test_files/_out/test_decode_pixel_data_individual_frames",
            );
            std::fs::create_dir_all(output_dir).unwrap();

            assert_eq!(pixel_data.number_of_frames(), 1, "expected 1 frame only");

            let image = pixel_data.to_dynamic_image(0).unwrap();
            let image_path = output_dir.join(format!(
                "{}-{}.png",
                Path::new(value).file_stem().unwrap().to_str().unwrap(),
                frame,
            ));
            image.save(image_path).unwrap();
        }
    }

    /// Loading a MONOCHROME1 image with encapsulated pixel data
    /// should not change the photometric interpretation
    /// (this rule does not apply to decoding via GDCM)
    #[cfg(all(feature = "jpeg", not(feature = "gdcm")))]
    #[test]
    fn test_monochrome1_decode_retains_pmi() {
        let path = dicom_test_files::path("WG04/JPLL/RG1_JPLL").unwrap();
        let obj = dicom_object::open_file(&path).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        assert_eq!(
            pixel_data.photometric_interpretation(),
            &PhotometricInterpretation::Monochrome1
        );
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_interleave() {
        let planar: Vec<u8> = vec![
            1, 2, 3, 4, // R
            5, 6, 7, 8, // G
            9, 10, 11, 12, // B
        ];
        let interleaved: Vec<u8> = vec![1, 5, 9, 2, 6, 10, 3, 7, 11, 4, 8, 12];
        assert_eq!(interleave(&planar), interleaved);
    }
}
