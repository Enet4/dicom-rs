//! This module aggregates errors that may emerge from the library.
use std::error::Error as BaseError;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};
use std::result;

//use data::dataset::DicomDataToken;
use value::ValueType;

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
        /// A failed attempt to cast a value to an inappropriate format.
        CastValue(err: CastValueError) {
            description("Failed value cast")
            from()
            cause(err)
            display(self_) -> ("{}: {}", self_.description(), err.description())
        }
    }
}

/// Type alias for a result from this library.
pub type Result<T> = result::Result<T, Error>;

quick_error! {
    /** Triggered when a value parsing attempt fails.
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
        ParseFloat(err: ParseFloatError) {
            description("Failed to parse text value as a floating point number")
            from()
            cause(err)
            display(self_) -> ("Value reading error: {}", self_.cause().unwrap().description())
        }
        /// The value cannot be parsed to an integer.
        ParseInteger(err: ParseIntError) {
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

/// An error type for an attempt of accessing a value
/// in an inappropriate format.
#[derive(Debug, Clone, PartialEq)]
pub struct CastValueError {
    /// The value format requested
    pub requested: &'static str,
    /// The value's actual representation
    pub got: ValueType,
}

impl fmt::Display for CastValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}: requested {} but value is {:?}",
            self.description(),
            self.requested,
            self.got
        )
    }
}

impl ::std::error::Error for CastValueError {
    fn description(&self) -> &str {
        "bad value cast"
    }
}
