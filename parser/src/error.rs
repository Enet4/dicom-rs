use crate::dataset::DataToken;
use dicom_core::error::Error as CoreError;
pub use dicom_core::error::{CastValueError, InvalidValueReadError};
use dicom_core::Tag;
use dicom_encoding::error::{Error as EncodingError, TextEncodingError};
use quick_error::quick_error;
use std::fmt;
use std::io;

/// Type alias for a result from this crate.
pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    /// The main data type for errors in the library.
    #[derive(Debug)]
    pub enum Error {
        /// Not valid DICOM content, typically raised when checking the magic code.
        InvalidFormat {
            display("Content is not DICOM or is corrupted")
        }
        /// A required element in the meta group is missing
        MissingMetaElement(name: &'static str) {
            display("Missing required meta element `{}`", name)
        }
        /// Raised when the obtained data element was not the one expected.
        UnexpectedTag(tag: Tag) {
            display("Unexpected DICOM tag {}", tag)
        }
        InconsistentSequenceEnd(eos: u64, bytes_read: u64) {
            display("already read {} bytes, but end of sequence is @ {} bytes", bytes_read, eos)
        }
        /// Raised when the obtained length is inconsistent.
        UnexpectedDataValueLength {
            display("Inconsistent data value length in data element")
        }
        /// Raised when a read was illegally attempted.
        IllegalDataRead {
            display("Illegal data value read")
        }
        /// Raised when the demanded transfer syntax is not supported.
        UnsupportedTransferSyntax {
            display("Unsupported transfer syntax")
        }
        /// Raised when the required character set is not supported.
        UnsupportedCharacterSet {
            display("Unsupported character set")
        }
        /// Raised when attempting to fetch an element by an unknown attribute name.
        NoSuchAttributeName {
            display("No such attribute name")
        }
        /// Raised when attempting to fetch an unexistent element.
        NoSuchDataElement {
            display("No such data element")
        }
        /// Raised when attempting to read pixel data out of bounds.
        PixelDataOutOfBounds {
            display("Pixel data access index out of bounds")
        }
        /// Raised when a data set parser couldn't fetch a value after a primitive
        /// data element's header.
        MissingElementValue {
            display("Expected value after data element header, but was missing")
        }
        /// Raised while parsing a DICOM data set and found an unexpected
        /// element header or value.
        DataSetSyntax(err: DataSetSyntaxError) {
            from()
            display("Data set syntax error: {}", err)
        }
        /// Error related to an invalid value read.
        ReadValue(err: InvalidValueReadError) {
            from()
            display("Invalid value read: {}", err)
        }
        /// Error related to a failed text encoding / decoding procedure.
        TextEncoding(err: TextEncodingError) {
            from()
            display("Failed text encoding/decoding: {}", err)
        }
        /// A failed attempt to cast a value to an inappropriate format.
        CastValue(err: CastValueError) {
            from()
            display("Failed value cast: {}", err)
        }
        /// Other I/O errors.
        Io(err: io::Error) {
            from()
            display("I/O error: {}", err)
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
        }
    }
}

impl From<EncodingError> for Error {
    fn from(e: EncodingError) -> Self {
        match e {
            EncodingError::UnexpectedTag(tag) => Error::UnexpectedTag(tag),
            EncodingError::UnexpectedDataValueLength => Error::UnexpectedDataValueLength,
            EncodingError::ReadValue(e) => Error::ReadValue(e),
            EncodingError::TextEncoding(e) => Error::TextEncoding(e),
            EncodingError::CastValue(e) => Error::CastValue(e),
            EncodingError::Io(e) => Error::Io(e),
        }
    }
}

#[derive(Debug)]
pub enum DataSetSyntaxError {
    PrematureEnd,
    UnexpectedToken(DataToken),
}

impl fmt::Display for DataSetSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DataSetSyntaxError::PrematureEnd => write!(f, "{}", self),
            DataSetSyntaxError::UnexpectedToken(ref token) => write!(f, "{} {}", self, token),
        }
    }
}

impl ::std::error::Error for DataSetSyntaxError {
    fn description(&self) -> &str {
        match self {
            DataSetSyntaxError::PrematureEnd => "data set ended prematurely",
            DataSetSyntaxError::UnexpectedToken(_) => "unexpected data set token",
        }
    }
}
