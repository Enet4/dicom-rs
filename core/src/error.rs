//! This module aggregates errors that may emerge from the library.
use std::borrow::Cow;
use std::error::Error as BaseError;
use std::result;
use std::io;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};

quick_error! {
    /// The main data type for errors in the library.
    #[derive(Debug)]
    pub enum Error {
        /// Not valid DICOM content, typically raised when checking the magic code.
        InvalidFormat {
            description("Content is not DICOM or is corrupted")
        }
        /// Raised when the obtained data element was not the one expected.
        UnexpectedElement {
            description("Unexpected DICOM element in current reading position")
        }
        /// Raised when the obtained length is inconsistent.
        UnexpectedDataValueLength {
            description("Inconsistent data value length in data element")
        }
        /// Raised when a read was illegally attempted.
        IllegalDataRead {
            description("Illegal data value read")
        }
        /// Raised when the demanded transfer syntax is not supported.
        UnsupportedTransferSyntax {
            description("Unsupported transfer syntax")
        }
        /// Raised when the required character set is not supported.
        UnsupportedCharacterSet {
            description("Unsupported character set")
        }
        /// Raised when attempting to fetch an element by an unknown attribute name.
        NoSuchAttributeName {
            description("No such attribute name")
        }
        /// Raised when attempting to fetch an unexistent element.
        NoSuchDataElement {
            description("No such data element")
        }
        /// Raised when attempting to read pixel data out of bounds.
        PixelDataOutOfBounds {
            description("Pixel data access index out of bounds")
        }
        /// Error related to an invalid value read.
        ValueRead(err: InvalidValueReadError) {
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
        /// Other I/O errors.
        Io(err: io::Error) {
            description("I/O error")
            from()
            cause(err)
            display(self_) -> ("{}: {}", self_.description(), err.description())
        }
    }
}

/// Type alias for a result from this library.
pub type Result<T> = result::Result<T, Error>;

quick_error! {
    /** Triggered when an invalid value read is attempted.
    */
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum InvalidValueReadError {
        /// The value cannot be read as a primitive value.
        NonPrimitiveType {
            description("Attempted to retrieve complex value as primitive")
            display(self_) -> ("Value reading error: {}", self_.description())
        }
        /// The value's effective length cannot be resolved.
        UnresolvedValueLength {
            description("Value length could not be resolved")
            display(self_) -> ("Value reading error: {}", self_.description())
        }
        /// The value does not have the expected format.
        InvalidFormat {
            description("Invalid format for the expected value representation")
            display(self_) -> ("Value reading error: {}", self_.description())
        }
        /// The value cannot be parsed to a floating point number.
        FloatParse(err: ParseFloatError) {
            description("Failed to parse text value as a floating point number")
            from()
            cause(err)
            display(self_) -> ("Value reading error: {}", self_.cause().unwrap().description())
        }
        /// The value cannot be parsed to an integer.
        IntegerParse(err: ParseIntError) {
            description("Failed to parse text value as an integer")
            from()
            cause(err)
            display(self_) -> ("Value reading error: {}", err.description())
        }
        /// An attempt of reading more than the number of bytes in the length attribute was made.
        UnexpectedEndOfElement {
            description("Unexpected end of element")
            display(self_) -> ("Value reading error: {}", self_.description())
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
