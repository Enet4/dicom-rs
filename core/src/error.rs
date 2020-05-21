//! This module aggregates errors that may emerge from the library.
use crate::value::ValueType;
use crate::Tag;
use quick_error::quick_error;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};
use std::result;

quick_error! {
    /// The main data type for errors in the library.
    #[derive(Debug)]
    pub enum Error {
        /// Raised when the obtained data element was not the one expected.
        UnexpectedTag(tag: Tag) {
            display("Unexpected DICOM tag {}", tag)
        }
        /// Raised when the obtained length is inconsistent.
        UnexpectedDataValueLength {
            display("Inconsistent data value length in data element")
        }
        /// Error related to an invalid value read.
        ReadValue(err: InvalidValueReadError) {
            display("Invalid value read: {}", err)
            from()
        }
        /// A failed attempt to cast a value to an inappropriate format.
        CastValue(err: CastValueError) {
            display("Failed value cast: {}", err)
            from()
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
            display("attempted to retrieve complex value as primitive")
        }
        /// The value's effective length cannot be resolved.
        UnresolvedValueLength {
            display("value length could not be resolved")
        }
        /// The value does not have the expected format.
        InvalidToken(got: u8, expected: &'static str) {
            display("invalid token: expected {} but got {:?}", expected, got)
        }
        /// The value does not have the expected length.
        InvalidLength(got: usize, expected: &'static str) {
            display("invalid length: expected {} but got {}", expected, got)
        }
        /// Invalid date or time component.
        ParseDateTime(got: u32, expected: &'static str) {
            display("invalid date/time component: expected {} but got {}", expected, got)
        }
        /// Invalid or ambiguous combination of date with time.
        DateTimeZone {
            display("Invalid or ambiguous combination of date with time")
        }
        /// chrono error when parsing a date or time.
        Chrono(err: chrono::ParseError) {
            display("failed to parse date/time: {}", err)
            from()
        }
        /// The value cannot be parsed to a floating point number.
        ParseFloat(err: ParseFloatError) {
            display("Failed to parse text value as a floating point number")
            from()
        }
        /// The value cannot be parsed to an integer.
        ParseInteger(err: ParseIntError) {
            display("Failed to parse text value as an integer")
            from()
        }
        /// An attempt of reading more than the number of bytes in the length attribute was made.
        UnexpectedEndOfElement {
            display("Unexpected end of element")
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
            "bad value cast: requested {} but value is {:?}",
            self.requested, self.got
        )
    }
}

impl ::std::error::Error for CastValueError {}
