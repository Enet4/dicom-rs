//! This module includes a high level abstraction over a DICOM data element's value.

use error::InvalidValueReadError;
use data::Tag;
use std::result;
use std::fmt::Debug;
use chrono::NaiveDate;
use chrono::NaiveTime;
use chrono::DateTime;
use chrono::UTC;

/// Result type alias for this module.
pub type Result<T> = result::Result<T, InvalidValueReadError>;

type C<T> = Vec<T>;

/// An enum representing a primitive value from a DICOM element. The result of decoding
/// an element's data value may be one of the enumerated types depending on its content
/// and value representation.
#[derive(Debug, PartialEq, Clone)]
pub enum DicomValue {
    /// No data. Used for SQ (regardless of content) and any value of length 0.
    Empty,

    /// A sequence of strings.
    /// Used for AE, AS, PN, SH, CS, LO, UI and UC.
    /// Can also be used for IS, SS, DS, DA, DT and TM when decoding
    /// with format preservation.
    Strs(C<String>),

    /// A single string.
    /// Used for ST, LT, UT and UR, which are never multi-valued.
    Str(String),

    /// A sequence of attribute tags.
    /// Used specifically for AT.
    Tags(C<Tag>),

    /// The value is a sequence of unsigned 16-bit integers.
    /// Used for OB and UN.
    U8(C<u8>),

    /// The value is a sequence of signed 16-bit integers.
    /// Used for SS.
    I16(C<i16>),

    /// A sequence of unsigned 168-bit integers.
    /// Used for US and OW.
    U16(C<u16>),

    /// A sequence of signed 32-bit integers.
    /// Used for SL and IS.
    I32(C<i32>),

    /// A sequence of unsigned 32-bit integers.
    /// Used for UL and OL.
    U32(C<u32>),

    /// The value is a sequence of 32-bit floating point numbers.
    /// Used for OF and FL.
    F32(C<f32>),

    /// The value is a sequence of 64-bit floating point numbers.
    /// Used for OD and FD, DS.
    F64(C<f64>),

    /// A sequence of dates.
    /// Used for the DA representation.
    Date(C<NaiveDate>),

    /// A sequence of date-time values.
    /// Used for the DT representation.
    DateTime(C<DateTime<UTC>>),

    /// A sequence of time values.
    /// Used for the TM representation.
    Time(C<NaiveTime>),
}

/// An enum representing a programmatic abstraction of
/// a DICOM element's data value type. This should be
/// the equivalent of `DicomValue` without the content.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueType {
    /// No data. Used for SQ (regardless of content) and any value of length 0.
    Empty,

    /// A sequence of strings.
    /// Used for AE, AS, PN, SH, CS, LO, UI and UC.
    /// Can also be used for IS, SS, DS, DA, DT and TM when decoding
    /// with format preservation.
    Strs,

    /// A single string.
    /// Used for ST, LT, UT and UR, which are never multi-valued.
    Str,

    /// A sequence of attribute tags.
    /// Used specifically for AT.
    Tags,

    /// The value is a sequence of unsigned 16-bit integers.
    /// Used for OB and UN.
    U8,

    /// The value is a sequence of signed 16-bit integers.
    /// Used for SS.
    I16,

    /// A sequence of unsigned 168-bit integers.
    /// Used for US and OW.
    U16,

    /// A sequence of signed 32-bit integers.
    /// Used for SL and IS.
    I32,

    /// A sequence of unsigned 32-bit integers.
    /// Used for UL and OL.
    U32,

    /// The value is a sequence of 32-bit floating point numbers.
    /// Used for OF and FL.
    F32,

    /// The value is a sequence of 64-bit floating point numbers.
    /// Used for OD and FD, DS.
    F64,

    /// A sequence of dates.
    /// Used for the DA representation.
    Date,

    /// A sequence of date-time values.
    /// Used for the DT representation.
    DateTime,

    /// A sequence of time values.
    /// Used for the TM representation.
    Time,
}

/// A trait for a value that maps to a DICOM element data value.
pub trait DicomValueType: Clone + Debug + 'static {
    /// Retrieve the specific type of this value.
    fn get_type(&self) -> ValueType;

    /// Retrieve the number of values contained.
    fn size(&self) -> u32;

    /// Check whether the value is empty (0 length).
    fn is_empty(&self) -> bool {
        self.size() == 0
    }
}


impl DicomValueType for DicomValue {
    fn get_type(&self) -> ValueType {
        match *self {
            DicomValue::Empty => ValueType::Empty,
            DicomValue::Date(_) => ValueType::Date,
            DicomValue::DateTime(_) => ValueType::DateTime,
            DicomValue::F32(_) => ValueType::F32,
            DicomValue::F64(_) => ValueType::F64,
            DicomValue::I16(_) => ValueType::I16,
            DicomValue::I32(_) => ValueType::I32,
            DicomValue::Str(_) => ValueType::Str,
            DicomValue::Strs(_) => ValueType::Strs,
            DicomValue::Tags(_) => ValueType::Tags,
            DicomValue::Time(_) => ValueType::Time,
            DicomValue::U16(_) => ValueType::U16,
            DicomValue::U32(_) => ValueType::U32,
            DicomValue::U8(_) => ValueType::U8,
        }
    }

    fn size(&self) -> u32 {
        match *self {
            DicomValue::Empty => 0,
            DicomValue::Str(_) => 1,
            DicomValue::Date(ref b) => b.len() as u32,
            DicomValue::DateTime(ref b) => b.len() as u32,
            DicomValue::F32(ref b) => b.len() as u32,
            DicomValue::F64(ref b) => b.len() as u32,
            DicomValue::I16(ref b) => b.len() as u32,
            DicomValue::I32(ref b) => b.len() as u32,
            DicomValue::Strs(ref b) => b.len() as u32,
            DicomValue::Tags(ref b) => b.len() as u32,
            DicomValue::Time(ref b) => b.len() as u32,
            DicomValue::U16(ref b) => b.len() as u32,
            DicomValue::U32(ref b) => b.len() as u32,
            DicomValue::U8(ref b) => b.len() as u32,
        }
    }
}
