//! Crate-level error types.
use dicom_core::error::Error as CoreError;
pub use dicom_core::error::{CastValueError, ConvertValueError, InvalidValueReadError};
use dicom_core::Tag;
use quick_error::quick_error;
use std::borrow::Cow;
use std::fmt;
use std::io;

/// Type alias for a result from this crate.
pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    /// The main data type for errors in the library.
    #[derive(Debug)]
    pub enum Error {
        /// Raised when the obtained data element tag was not the one expected.
        UnexpectedTag(tag: Tag) {
            display("Unexpected DICOM tag {}", tag)
        }
        /// Raised when the obtained length is inconsistent.
        UnexpectedDataValueLength {
            display("Inconsistent data value length in data element")
        }
        /// Error related to an invalid value read.
        ReadValue(err: InvalidValueReadError) {
            from()
            display("Invalid value read: {}", err)
        }
        /// Error related to a failed text encoding / decoding procedure.
        TextEncoding(err: TextEncodingError) {
            display("Failed text encoding/decoding: {}", err)
            from()
        }
        /// A failed attempt to cast a value to an inappropriate format.
        CastValue(err: CastValueError) {
            display("Failed value cast: {}", err)
            from()
        }
        /// A failed attempt to cast a value to an inappropriate format.
        ConvertValue(err: ConvertValueError) {
            display("Failed value conversion: {}", err)
            from()
        }
        /// Other I/O errors.
        Io(err: io::Error) {
            display("I/O error: {}", err)
            from()
        }
    }
}

impl From<CoreError> for Error {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::UnexpectedDataValueLength => Error::UnexpectedDataValueLength,
            CoreError::UnexpectedTag(tag) => Error::UnexpectedTag(tag),
            CoreError::ReadValue(e) => Error::ReadValue(e),
            CoreError::CastValue(e) => Error::CastValue(e),
            CoreError::ConvertValue(e) => Error::ConvertValue(e),
        }
    }
}

/// An error type for text encoding issues.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEncodingError(Cow<'static, str>);

impl TextEncodingError {
    /// Build an error from a cause text, as provided by the
    /// `encoding` crate.
    pub fn new<E: Into<Cow<'static, str>>>(cause: E) -> Self {
        TextEncodingError(cause.into())
    }
}

impl fmt::Display for TextEncodingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "encoding/decoding process failed: {}", self.0)
    }
}

impl ::std::error::Error for TextEncodingError {}
