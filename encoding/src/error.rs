use dicom_core::error::Error as CoreError;
pub use dicom_core::error::{CastValueError, InvalidValueReadError};
use quick_error::quick_error;
use std::borrow::Cow;
use std::error::Error as BaseError;
use std::fmt;
use std::io;

/// Type alias for a result from this crate.
pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    /// The main data type for errors in the library.
    #[derive(Debug)]
    pub enum Error {
        /// Raised when the obtained data element was not the one expected.
        UnexpectedElement {
            description("Unexpected DICOM element in current reading position")
        }
        /// Raised when the obtained length is inconsistent.
        UnexpectedDataValueLength {
            description("Inconsistent data value length in data element")
        }
        /// Error related to an invalid value read.
        ReadValue(err: InvalidValueReadError) {
            description("Invalid value read")
            from()
            cause(err)
            display(self_) -> ("{}: {}", self_.description(), err.description())
        }
        /// Error related to a failed text encoding / decoding procedure.
        TextEncoding(err: TextEncodingError) {
            description("Failed text encoding/decoding")
            from()
            cause(err)
            display(self_) -> ("{}: {}", self_.description(), err.description())
        }
        /// A failed attempt to cast a value to an inappropriate format.
        CastValue(err: CastValueError) {
            description("Failed value cast")
            from()
            cause(err)
            display(self_) -> ("{}: {}", self_.description(), err.description())
        }
        /// Other I/O errors.
        Io(err: io::Error) {
            description("I/O error")
            from()
            cause(err)
            display(self_) -> ("{}: {}", self_.description(), err.description())
        }
    }
}

impl From<CoreError> for Error {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::UnexpectedDataValueLength => Error::UnexpectedDataValueLength,
            CoreError::UnexpectedElement => Error::UnexpectedElement,
            CoreError::ReadValue(e) => Error::ReadValue(e),
            CoreError::CastValue(e) => Error::CastValue(e),
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
        write!(f, "{}: {}", self.description(), self.0)
    }
}

impl ::std::error::Error for TextEncodingError {
    fn description(&self) -> &str {
        "encoding/decoding process failed"
    }
}
