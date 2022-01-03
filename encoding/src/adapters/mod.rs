use dicom_core::value::C;
use snafu::Snafu;

pub mod rle_lossless;

/// Error conditions when decoding pixel data.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum DecodeError {
    /// A custom error when decoding fails
    #[snafu(display("Error decoding pixel data: {}", message))]
    CustomDecodeError { message: &'static str },

    /// Input pixel data is not encapsulated
    NotEncapsulated,

    /// A required attribute is missing from the DICOM
    #[snafu(display("Missing required attribute: {}", name))]
    MissingAttribute { name: &'static str },
}

/// Error conditions when encoding pixel data.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum EncodeError {
    /// A custom error when encoding fails
    #[snafu(display("Error encoding pixel data {}", message))]
    CustomEncodeError {
        message: &'static str,
    },

    /// Input pixel data is not native
    NotNative,

    /// Encoding is not implemented
    NotImplemented,
}

pub type DecodeResult<T, E = DecodeError> = Result<T, E>;

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
/// as defined in [`dicom_object`](::dicom_object),
/// in order to retrieve important pieces of the object
/// for pixel data decoding into images or multi-dimensional arrays.
/// 
/// It is defined in this crate so that
/// transfer syntax implementers only have to depend on `dicom_encoding`.
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
    fn number_of_frames(&self) -> Option<u16>;

    /// Returns the number of fragments or None for native pixel data
    fn number_of_fragments(&self) -> Option<u32>;

    /// Return a specific encoded pixel fragment by index as Vec<u8>
    /// or None if no pixel data is found
    fn fragment(&self, fragment: usize) -> Option<Vec<u8>>;

    /// Should return either a byte slice/vector if native pixel data
    /// or byte fragments if encapsulated.
    /// Returns None if no pixel data is found
    fn raw_pixel_data(&self) -> Option<RawPixelData>;
}

/// Trait object responsible for decoding and encoding
/// pixel data based on the Transfersyntax.
/// 
/// Every transfer syntax with encapsulated pixel data
/// should implement these methods.
///
pub trait PixelRWAdapter {
    /// Decode the given DICOM object
    /// containing encapsulated pixel data
    /// into native pixel data as a byte stream.
    /// 
    /// It is a necessary precondition that the object's pixel data
    /// is encoded in accordance to the transfer syntax(es)
    /// supported by this adapter.
    /// A `NotEncapsulated` error is returned otherwise.
    /// 
    /// The output is a sequence of native pixel values
    /// which follow the image properties of the given object.
    /// 
    fn decode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> DecodeResult<()>;

    /// Encode a DICOM object's image into the format supported by this adapter,
    /// writing a byte stream of pixel data fragment values
    /// into the given destination.
    /// 
    /// It is a necessary precondition that the object's pixel data
    /// is in a _native encoding_.
    /// A `NotNative` error is returned otherwise.
    /// 
    /// It is possible that
    /// image encoding is not actually supported by this adapter,
    /// in which case a `NotImplemented` error is returned.
    /// Implementers leave the default method implementation
    /// for this behavior.
    #[allow(unused_variables)]
    fn encode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> EncodeResult<()> {
        Err(EncodeError::NotImplemented)
    }
}

/// Alias type for a dynamically dispatched data adapter.
pub type DynPixelRWAdapter = Box<dyn PixelRWAdapter + Send + Sync>;

/// An immaterial type representing an adapter which is never required.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NeverPixelAdapter {}

impl PixelRWAdapter for NeverPixelAdapter {
    fn decode(&self, _src: &dyn PixelDataObject, _dst: &mut Vec<u8>) -> DecodeResult<()> {
        unreachable!();
    }

    fn encode(&self, _src: &dyn PixelDataObject, _dst: &mut Vec<u8>) -> EncodeResult<()> {
        unreachable!();
    }
}
