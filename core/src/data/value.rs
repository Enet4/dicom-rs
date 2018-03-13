//! This module includes a high level abstraction over a DICOM data element's value.

use error::InvalidValueReadError;
use data::Tag;
use std::result;
use chrono::{NaiveDate, NaiveTime, DateTime, FixedOffset};

/// Result type alias for this module.
pub type Result<T> = result::Result<T, InvalidValueReadError>;

type C<T> = Vec<T>;

/// Representation of a full DICOM value, which may be either primitive or
/// another DICOM object.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<I> {
    /// Primitive value
    Primitive(PrimitiveValue),
    /// A complex item in a sequence
    Item(I),
}

impl<I> Value<I> {
    /// Gets a reference to the primitive value.
    pub fn primitive(&self) -> Option<&PrimitiveValue> {
        match *self {
            Value::Primitive(ref v) => Some(v),
            _ => None,
        }
    }

    /// Gets a reference to the item.
    pub fn item(&self) -> Option<&I> {
        match *self {
            Value::Item(ref item) => Some(item),
            _ => None,
        }
    }

    /// Retrieves the primitive value.
    pub fn into_primitive(self) -> Option<PrimitiveValue> {
        match self {
            Value::Primitive(v) => Some(v),
            _ => None,
        }
    }

    /// Retrieves the item.
    pub fn into_item(self) -> Option<I> {
        match self {
            Value::Item(item) => Some(item),
            _ => None,
        }
    }
}

impl<I> From<PrimitiveValue> for Value<I> {
    fn from(v: PrimitiveValue) -> Self {
        Value::Primitive(v)
    }
}

/// An enum representing a primitive value from a DICOM element. The result of decoding
/// an element's data value may be one of the enumerated types depending on its content
/// and value representation.
#[derive(Debug, PartialEq, Clone)]
pub enum PrimitiveValue {
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
    DateTime(C<DateTime<FixedOffset>>),

    /// A sequence of time values.
    /// Used for the TM representation.
    Time(C<NaiveTime>),
}

impl PrimitiveValue {
    /// Get a single string value. If it contains multiple strings,
    /// only the first one is returned.
    pub fn string(&self) -> Option<&str> {
        use self::PrimitiveValue::*;
        match self {
            &Strs(ref c) => c.get(0).map(String::as_str),
            &Str(ref s) => Some(s),
            _ => None,
        }
    }

    /// Get a sequence of string values.
    pub fn strings(&self) -> Option<Vec<&str>> {
        use self::PrimitiveValue::*;
        match self {
            &Strs(ref c) => Some(c.iter().map(String::as_str).collect()),
            &Str(ref s) => Some(vec![&s]),
            _ => None,
        }
    }

    /// Get a single DICOM tag.
    pub fn tag(&self) -> Option<Tag> {
        use self::PrimitiveValue::*;
        match self {
            &Tags(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }

    /// Get a sequence of DICOM tags.
    pub fn tags(&self) -> Option<&[Tag]> {
        use self::PrimitiveValue::*;
        match self {
            &Tags(ref c) => Some(&c),
            _ => None,
        }
    }

    /// Get a single 32-bit signed integer value.
    pub fn int32(&self) -> Option<i32> {
        use self::PrimitiveValue::*;
        match self {
            &I32(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }

    /// Get a single 32-bit unsigned integer value.
    pub fn uint32(&self) -> Option<u32> {
        use self::PrimitiveValue::*;
        match self {
            &U32(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }

    /// Get a single 16-bit signed integer value.
    pub fn int16(&self) -> Option<i16> {
        use self::PrimitiveValue::*;
        match self {
            &I16(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }

    /// Get a single 16-bit unsigned integer value.
    pub fn uint16(&self) -> Option<u16> {
        use self::PrimitiveValue::*;
        match self {
            &U16(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }

    /// Get a single 8-bit unsigned integer value.
    pub fn uint8(&self) -> Option<u8> {
        use self::PrimitiveValue::*;
        match self {
            &U8(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }

    /// Get a single 32-bit floating point number value.
    pub fn float32(&self) -> Option<f32> {
        use self::PrimitiveValue::*;
        match self {
            &F32(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }

    /// Get a single 64-bit floating point number value.
    pub fn float64(&self) -> Option<f64> {
        use self::PrimitiveValue::*;
        match self {
            &F64(ref c) => c.get(0).map(Clone::clone),
            _ => None,
        }
    }
}

/// An enum representing an abstraction of a DICOM element's data value type.
/// This should be the equivalent of `PrimitiveValue` without the content,
/// plus the `Item` entry.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueType {
    /// No data. Used for any value of length 0.
    Empty,

    /// An item. Used for elements in a SQ, regardless of content.
    Item,

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
    /// Used for OD, FD and DS.
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
pub trait DicomValueType {
    /// Retrieve the specific type of this value.
    fn get_type(&self) -> ValueType;

    /// Retrieve the number of values contained.
    fn size(&self) -> u32;

    /// Check whether the value is empty (0 length).
    fn is_empty(&self) -> bool {
        self.size() == 0
    }
}

impl DicomValueType for PrimitiveValue {
    fn get_type(&self) -> ValueType {
        use self::PrimitiveValue::*;
        match *self {
            Empty => ValueType::Empty,
            Date(_) => ValueType::Date,
            DateTime(_) => ValueType::DateTime,
            F32(_) => ValueType::F32,
            F64(_) => ValueType::F64,
            I16(_) => ValueType::I16,
            I32(_) => ValueType::I32,
            Str(_) => ValueType::Str,
            Strs(_) => ValueType::Strs,
            Tags(_) => ValueType::Tags,
            Time(_) => ValueType::Time,
            U16(_) => ValueType::U16,
            U32(_) => ValueType::U32,
            U8(_) => ValueType::U8,
        }
    }

    fn size(&self) -> u32 {
        use self::PrimitiveValue::*;
        match *self {
            Empty => 0,
            Str(_) => 1,
            Date(ref b) => b.len() as u32,
            DateTime(ref b) => b.len() as u32,
            F32(ref b) => b.len() as u32,
            F64(ref b) => b.len() as u32,
            I16(ref b) => b.len() as u32,
            I32(ref b) => b.len() as u32,
            Strs(ref b) => b.len() as u32,
            Tags(ref b) => b.len() as u32,
            Time(ref b) => b.len() as u32,
            U16(ref b) => b.len() as u32,
            U32(ref b) => b.len() as u32,
            U8(ref b) => b.len() as u32,
        }
    }
}

impl<I> DicomValueType for Value<I>
where
    I: DicomValueType,
{
    fn get_type(&self) -> ValueType {
        match *self {
            Value::Primitive(ref v) => v.get_type(),
            Value::Item(_) => ValueType::Item,
        }
    }

    fn size(&self) -> u32 {
        match *self {
            Value::Primitive(ref v) => v.size(),
            Value::Item(ref i) => i.size(),
        }
    }
}