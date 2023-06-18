//! Core module for building pixel data adapters.
//!
//! This module contains the core types and traits
//! for consumers and implementers of
//! transfer syntaxes with encapsulated pixel data.
//!
//! Complete DICOM object types
//! (such as `FileDicomObject<InMemDicomObject>`)
//! implement the [`PixelDataObject`] trait.
//! Transfer syntaxes which define an encapsulated pixel data encoding
//! need to provide suitable implementations of
//! [`PixelDataReader`] and [`PixelDataWriter`]
//! to be able to decode and encode imaging data, respectively.

use dicom_core::{ops::AttributeOp, value::C};
use snafu::Snafu;
use std::borrow::Cow;

/// The possible error conditions when decoding (reading) pixel data.
///
/// Users of this type are free to handle errors based on their variant,
/// but should not make decisions based on the display message,
/// since that is not considered part of the API
/// and may change on any new release.
///
/// Implementers of transfer syntaxes
/// are recommended to choose the most fitting error variant
/// for the tested condition.
/// When no suitable variant is available,
/// the [`Custom`](DecodeError::Custom) variant may be used.
/// See also [`snafu`] for guidance on using context selectors.
#[derive(Debug, Snafu)]
#[non_exhaustive]
#[snafu(visibility(pub), module)]
pub enum DecodeError {
    /// A custom error occurred when decoding,
    /// reported as a dynamic error value with a message.
    ///
    /// The [`whatever!`](snafu::whatever) macro can be used
    /// to easily create an error of this kind.
    #[snafu(whatever, display("{}", message))]
    Custom {
        /// The error message.
        message: String,
        /// The underlying error cause, if any.
        #[snafu(source(from(Box<dyn std::error::Error + Send + Sync + 'static>, Some)))]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },

    /// The input pixel data is not encapsulated.
    /// 
    /// Either the image needs no decoding
    /// or the compressed imaging data was in a flat pixel data element by mistake.
    NotEncapsulated,

    /// The requested frame range is outside the given object's frame range.
    FrameRangeOutOfBounds,

    /// A required attribute is missing
    /// from the DICOM object representing the image.
    #[snafu(display("Missing required attribute `{}`", name))]
    MissingAttribute { name: &'static str },
}

/// The possible error conditions when encoding (writing) pixel data.
///
/// Users of this type are free to handle errors based on their variant,
/// but should not make decisions based on the display message,
/// since that is not considered part of the API
/// and may change on any new release.
///
/// Implementers of transfer syntaxes
/// are recommended to choose the most fitting error variant
/// for the tested condition.
/// When no suitable variant is available,
/// the [`Custom`](EncodeError::Custom) variant may be used.
/// See also [`snafu`] for guidance on using context selectors.
#[derive(Debug, Snafu)]
#[non_exhaustive]
#[snafu(visibility(pub), module)]
pub enum EncodeError {
    /// A custom error when encoding fails.
    /// Read the `message` and the underlying `source`
    /// for more details. 
    #[snafu(whatever, display("{}", message))]
    Custom {
        /// The error message.
        message: String,
        /// The underlying error cause, if any.
        #[snafu(source(from(Box<dyn std::error::Error + Send + Sync + 'static>, Some)))]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },

    /// Input pixel data is not native, should be decoded first.
    NotNative,

    /// The requested frame range is outside the given object's frame range.
    FrameRangeOutOfBounds,

    /// A required attribute is missing
    /// from the DICOM object representing the image.
    #[snafu(display("Missing required attribute `{}`", name))]
    MissingAttribute { name: &'static str },
}

/// The result of decoding (reading) pixel data
pub type DecodeResult<T, E = DecodeError> = Result<T, E>;

/// The result of encoding (writing) pixel data
pub type EncodeResult<T, E = EncodeError> = Result<T, E>;

#[derive(Debug)]
pub struct RawPixelData {
    /// Either a byte slice/vector if native pixel data
    /// or byte fragments if encapsulated
    pub fragments: C<Vec<u8>>,

    /// The offset table for the fragments,
    /// or empty if there is none
    pub offset_table: C<u32>,
}

/// A DICOM object trait to be interpreted as pixel data.
///
/// This trait extends the concept of DICOM object
/// as defined in [`dicom_object`],
/// in order to retrieve important pieces of the object
/// for pixel data decoding into images or multi-dimensional arrays.
///
/// It is defined in this crate so that
/// transfer syntax implementers only have to depend on `dicom_encoding`.
///
/// [`dicom_object`]: https://docs.rs/dicom_object
pub trait PixelDataObject {
    /// Return the Rows attribute or None if it is not found
    fn rows(&self) -> Option<u16>;

    /// Return the Columns attribute or None if it is not found
    fn cols(&self) -> Option<u16>;

    /// Return the SamplesPerPixel attribute or None if it is not found
    fn samples_per_pixel(&self) -> Option<u16>;

    /// Return the BitsAllocated attribute or None if it is not set
    fn bits_allocated(&self) -> Option<u16>;

    /// Return the NumberOfFrames attribute or None if it is not set
    fn number_of_frames(&self) -> Option<u32>;

    /// Returns the number of fragments or None for native pixel data
    fn number_of_fragments(&self) -> Option<u32>;

    /// Return a specific encoded pixel fragment by index
    /// (where 0 is the first fragment after the basic offset table)
    /// as a [`Cow<[u8]>`][1],
    /// or `None` if no such fragment is available.
    ///
    /// [1]: std::borrow::Cow
    fn fragment(&self, fragment: usize) -> Option<Cow<[u8]>>;

    /// Should return either a byte slice/vector if native pixel data
    /// or byte fragments if encapsulated.
    /// Returns None if no pixel data is found
    fn raw_pixel_data(&self) -> Option<RawPixelData>;
}

/// Custom options when encoding pixel data into an encapsulated form.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct EncodeOptions {
    /// The quality of the output image as a number between 0 and 100,
    /// where 100 is the best quality that the encapsulated form can achieve
    /// and smaller values represent smaller data size
    /// with an increasingly higher error.
    /// It is ignored if the transfer syntax only supports lossless compression.
    /// If it does support lossless compression,
    /// it is expected that a quality of 100 results in a lossless encoding.
    ///
    /// If this option is not specified,
    /// the output quality is decided automatically by the underlying adapter.
    pub quality: Option<u8>,

    /// The amount of effort that the encoder may take to encode the pixel data,
    /// as a number between 0 and 100.
    /// If supported, higher values result in better compression,
    /// at the expense of more processing time.
    /// Encoders are not required to support this option.
    /// If this option is not specified,
    /// the actual effort is decided by the underlying adapter.
    pub effort: Option<u8>,
}

impl EncodeOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Trait object responsible for decoding
/// pixel data based on the transfer syntax.
///
/// A transfer syntax with support for decoding encapsulated pixel data
/// would implement these methods.
pub trait PixelDataReader {
    /// Decode the given DICOM object
    /// containing encapsulated pixel data
    /// into native pixel data as a byte stream in little endian,
    /// appending these bytes to the given vector `dst`.
    ///
    /// It is a necessary precondition that the object's pixel data
    /// is encoded in accordance to the transfer syntax(es)
    /// supported by this adapter.
    /// A `NotEncapsulated` error is returned otherwise.
    ///
    /// The output is a sequence of native pixel values
    /// which follow the image properties of the given object
    /// _save for the photometric interpretation and planar configuration_.
    /// The output of an image with 1 sample per pixel
    /// is expected to be interpreted as `MONOCHROME2`,
    /// and for 3-channel images,
    /// the output must be in RGB with each pixel contiguous in memory
    /// (planar configuration of 0).
    fn decode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> DecodeResult<()>;

    /// Decode the given DICOM object
    /// containing encapsulated pixel data
    /// into native pixel data of a single frame
    /// as a byte stream in little endian,
    /// appending these bytes to the given vector `dst`.
    ///
    /// It is a necessary precondition that the object's pixel data
    /// is encoded in accordance to the transfer syntax(es)
    /// supported by this adapter.
    /// A `NotEncapsulated` error is returned otherwise.
    ///
    /// The output is a sequence of native pixel values of a frame
    /// which follow the image properties of the given object
    /// _save for the photometric interpretation and planar configuration_.
    /// The output of an image with 1 sample per pixel
    /// is expected to be interpreted as `MONOCHROME2`,
    /// and for 3-channel images,
    /// the output must be in RGB with each pixel contiguous in memory
    /// (planar configuration of 0).
    fn decode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        dst: &mut Vec<u8>,
    ) -> DecodeResult<()>;
}

/// Trait object responsible for decoding
/// pixel data based on the transfer syntax.
///
/// A transfer syntax with support for decoding encapsulated pixel data
/// would implement these methods.
pub trait PixelDataWriter {
    /// Encode a DICOM object's image into the format supported by this adapter,
    /// writing a byte stream of pixel data fragment values
    /// to the given vector `dst`.
    ///
    /// It is a necessary precondition that the object's pixel data
    /// is in a _native encoding_.
    /// A `NotNative` error is returned otherwise.
    ///
    /// When the operation is successful,
    /// a listing of attribute changes is returned,
    /// comprising the sequence of operations that the DICOM object
    /// should consider upon assuming the new encoding.
    fn encode(
        &self,
        src: &dyn PixelDataObject,
        options: EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>>;

    /// Encode a single frame of a DICOM object's image
    /// into the format supported by this adapter,
    /// writing a byte stream of pixel data fragment values
    /// into the given destination.
    ///
    /// It is a necessary precondition that the object's pixel data
    /// is in a _native encoding_.
    /// A `NotNative` error is returned otherwise.
    ///
    /// When the operation is successful,
    /// a listing of attribute changes is returned,
    /// comprising the sequence of operations that the DICOM object
    /// should consider upon assuming the new encoding.
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        options: EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>>;
}

/// Alias type for a dynamically dispatched pixel data reader.
pub type DynPixelDataReader = Box<dyn PixelDataReader + Send + Sync + 'static>;

/// Alias type for a dynamically dispatched pixel data writer.
pub type DynPixelDataWriter = Box<dyn PixelDataWriter + Send + Sync + 'static>;

/// An immaterial type representing an adapter which is never provided.
/// 
/// This type may be used as the type parameters `R` and `W`
/// of [`TransferSyntax`](crate::transfer_syntax::TransferSyntax)
/// when representing a transfer syntax which
/// either does not support reading and writing imaging data,
/// or when such support is not needed in the first place.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NeverPixelAdapter {}

impl PixelDataReader for NeverPixelAdapter {
    fn decode(&self, _src: &dyn PixelDataObject, _dst: &mut Vec<u8>) -> DecodeResult<()> {
        unreachable!()
    }

    fn decode_frame(
        &self,
        _src: &dyn PixelDataObject,
        _frame: u32,
        _dst: &mut Vec<u8>,
    ) -> DecodeResult<()> {
        unreachable!()
    }
}

impl PixelDataWriter for NeverPixelAdapter {
    fn encode(
        &self,
        _src: &dyn PixelDataObject,
        _options: EncodeOptions,
        _dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        unreachable!()
    }

    fn encode_frame(
        &self,
        _src: &dyn PixelDataObject,
        _frame: u32,
        _options: EncodeOptions,
        _dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        unreachable!()
    }
}

impl PixelDataReader for crate::transfer_syntax::NeverAdapter {
    fn decode(&self, _src: &dyn PixelDataObject, _dst: &mut Vec<u8>) -> DecodeResult<()> {
        unreachable!()
    }

    fn decode_frame(
        &self,
        _src: &dyn PixelDataObject,
        _frame: u32,
        _dst: &mut Vec<u8>,
    ) -> DecodeResult<()> {
        unreachable!()
    }
}

impl PixelDataWriter for crate::transfer_syntax::NeverAdapter {
    fn encode(
        &self,
        _src: &dyn PixelDataObject,
        _options: EncodeOptions,
        _dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        unreachable!()
    }

    fn encode_frame(
        &self,
        _src: &dyn PixelDataObject,
        _frame: u32,
        _options: EncodeOptions,
        _dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        unreachable!()
    }
}
