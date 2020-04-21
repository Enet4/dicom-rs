//! This module includes a high level abstraction over a DICOM data element's value.

use crate::error::CastValueError;
use crate::header::{HasLength, Length, Tag};
use chrono::{Datelike, FixedOffset, Timelike};
use itertools::Itertools;
use smallvec::SmallVec;
use std::borrow::Cow;

/// re-exported from chrono
pub use chrono::{DateTime, NaiveDate, NaiveTime};

/// An aggregation of one or more elements in a value.
pub type C<T> = SmallVec<[T; 2]>;

/// Representation of a full DICOM value, which may be either primitive or
/// another DICOM object.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<I> {
    /// Primitive value
    Primitive(PrimitiveValue),
    /// A complex sequence of items
    Sequence {
        /// Item collection.
        items: C<I>,
        /// The size in bytes.
        size: Length,
    },
}

impl<I> Value<I> {
    /// Obtain the number of individual values.
    /// In a sequence item, this is the number of items.
    pub fn multiplicity(&self) -> u32 {
        match *self {
            Value::Primitive(ref v) => v.multiplicity(),
            Value::Sequence { ref items, .. } => items.len() as u32,
        }
    }

    /// Gets a reference to the primitive value.
    pub fn primitive(&self) -> Option<&PrimitiveValue> {
        match *self {
            Value::Primitive(ref v) => Some(v),
            _ => None,
        }
    }

    /// Gets a reference to the items.
    pub fn items(&self) -> Option<&[I]> {
        match *self {
            Value::Sequence { ref items, .. } => Some(items),
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

    /// Retrieves the items.
    pub fn into_items(self) -> Option<C<I>> {
        match self {
            Value::Sequence { items, .. } => Some(items),
            _ => None,
        }
    }
}

impl<I> Value<I>
where
    I: HasLength,
{
    /// Retrieves the primitive value as a single string.
    ///
    /// If the value contains multiple strings, they are concatenated
    /// (separated by `'\\'`) into an owned string.
    pub fn to_str(&self) -> Result<Cow<str>, CastValueError> {
        match self {
            Value::Primitive(PrimitiveValue::Str(v)) => Ok(Cow::from(v.as_str())),
            Value::Primitive(PrimitiveValue::Strs(v)) => Ok(Cow::from(v.into_iter().join("\\"))),
            _ => Err(CastValueError {
                requested: "string",
                got: self.value_type(),
            }),
        }
    }

    /// Retrieves the primitive value as a sequence of unsigned bytes.
    pub fn as_u8(&self) -> Result<&[u8], CastValueError> {
        match self {
            Value::Primitive(PrimitiveValue::U8(v)) => Ok(&v),
            _ => Err(CastValueError {
                requested: "u8",
                got: self.value_type(),
            }),
        }
    }

    /// Retrieves the primitive value as a sequence of signed 32-bit integers.
    pub fn as_i32(&self) -> Result<&[i32], CastValueError> {
        match self {
            Value::Primitive(PrimitiveValue::I32(v)) => Ok(&v),
            _ => Err(CastValueError {
                requested: "i32",
                got: self.value_type(),
            }),
        }
    }

    /// Retrieves the primitive value as a DICOM tag.
    pub fn to_tag(&self) -> Result<Tag, CastValueError> {
        match self {
            Value::Primitive(PrimitiveValue::Tags(v)) => Ok(v[0]),
            _ => Err(CastValueError {
                requested: "tag",
                got: self.value_type(),
            }),
        }
    }

    /// Retrieves the primitive value as a sequence of DICOM tags.
    pub fn as_tags(&self) -> Result<&[Tag], CastValueError> {
        match self {
            Value::Primitive(PrimitiveValue::Tags(v)) => Ok(&v),
            _ => Err(CastValueError {
                requested: "tag",
                got: self.value_type(),
            }),
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

    /// A sequence of signed 64-bit integers.
    /// Used for SV.
    I64(C<i64>),

    /// A sequence of unsigned 64-bit integers.
    /// Used for UV and OV.
    U64(C<u64>),

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

/// A utility macro for implementing the conversion from a core type into a
/// DICOM primitive value with a single element.
macro_rules! impl_from_for_primitive {
    ($typ: ty, $variant: ident) => {
        impl From<$typ> for PrimitiveValue {
            fn from(value: $typ) -> Self {
                PrimitiveValue::$variant(C::from_elem(value, 1))
            }
        }
    };
}

impl_from_for_primitive!(u8, U8);
impl_from_for_primitive!(u16, U16);
impl_from_for_primitive!(i16, I16);
impl_from_for_primitive!(u32, U32);
impl_from_for_primitive!(i32, I32);
impl_from_for_primitive!(u64, U64);
impl_from_for_primitive!(i64, I64);
impl_from_for_primitive!(f32, F32);
impl_from_for_primitive!(f64, F64);

impl_from_for_primitive!(Tag, Tags);
impl_from_for_primitive!(NaiveDate, Date);
impl_from_for_primitive!(NaiveTime, Time);
impl_from_for_primitive!(DateTime<FixedOffset>, DateTime);

/// Construct a DICOM value.
#[macro_export]
macro_rules! dicom_value {
    ($typ: ident, [ $($elem: expr),* ]) => {
        {
            use smallvec::smallvec; // import smallvec macro
            dicom_core::value::PrimitiveValue :: $typ (smallvec![$($elem,)*])
        }
    };
    ($typ: ident, $elem: expr) => {
        dicom_core::value::PrimitiveValue :: $typ (dicom_core::value::C::from_elem($elem, 1))
    };
}

impl From<String> for PrimitiveValue {
    fn from(value: String) -> Self {
        PrimitiveValue::Str(value)
    }
}

impl From<&str> for PrimitiveValue {
    fn from(value: &str) -> Self {
        PrimitiveValue::Str(value.to_owned())
    }
}

impl PrimitiveValue {
    /// Create a single unsigned 16-bit value.
    pub fn new_u16(value: u16) -> Self {
        PrimitiveValue::U16(C::from_elem(value, 1))
    }

    /// Create a single unsinged 32 value.
    pub fn new_u32(value: u32) -> Self {
        PrimitiveValue::U32(C::from_elem(value, 1))
    }

    /// Create a single I32 value.
    pub fn new_i32(value: u32) -> Self {
        PrimitiveValue::U32(C::from_elem(value, 1))
    }

    /// Obtain the number of individual elements. This number may not
    /// match the DICOM value multiplicity in some value representations.
    pub fn multiplicity(&self) -> u32 {
        use self::PrimitiveValue::*;
        match self {
            Empty => 0,
            Str(_) => 1,
            Strs(c) => c.len() as u32,
            Tags(c) => c.len() as u32,
            U8(c) => c.len() as u32,
            I16(c) => c.len() as u32,
            U16(c) => c.len() as u32,
            I32(c) => c.len() as u32,
            U32(c) => c.len() as u32,
            I64(c) => c.len() as u32,
            U64(c) => c.len() as u32,
            F32(c) => c.len() as u32,
            F64(c) => c.len() as u32,
            Date(c) => c.len() as u32,
            DateTime(c) => c.len() as u32,
            Time(c) => c.len() as u32,
        }
    }

    /// Get a single string value. If it contains multiple strings,
    /// only the first one is returned.
    pub fn string(&self) -> Option<&str> {
        use self::PrimitiveValue::*;
        match self {
            Strs(c) => c.first().map(String::as_str),
            Str(s) => Some(s),
            _ => None,
        }
    }

    /// Get a sequence of string values.
    pub fn strings(&self) -> Option<Vec<&str>> {
        use self::PrimitiveValue::*;
        match self {
            Strs(c) => Some(c.iter().map(String::as_str).collect()),
            Str(s) => Some(vec![&s]),
            _ => None,
        }
    }

    /// Get a single DICOM tag.
    pub fn tag(&self) -> Option<Tag> {
        use self::PrimitiveValue::*;
        match self {
            Tags(c) => c.first().map(Clone::clone),
            _ => None,
        }
    }

    /// Get a sequence of DICOM tags.
    pub fn tags(&self) -> Option<&[Tag]> {
        use self::PrimitiveValue::*;
        match self {
            Tags(c) => Some(&c),
            _ => None,
        }
    }

    /// Get a single 64-bit signed integer value.
    pub fn int64(&self) -> Option<i64> {
        use self::PrimitiveValue::*;
        match self {
            I64(c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 64-bit unsigned integer value.
    pub fn uint64(&self) -> Option<u64> {
        use self::PrimitiveValue::*;
        match self {
            U64(c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 32-bit signed integer value.
    pub fn int32(&self) -> Option<i32> {
        use self::PrimitiveValue::*;
        match self {
            I32(c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 32-bit unsigned integer value.
    pub fn uint32(&self) -> Option<u32> {
        use self::PrimitiveValue::*;
        match self {
            U32(ref c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 16-bit signed integer value.
    pub fn int16(&self) -> Option<i16> {
        use self::PrimitiveValue::*;
        match self {
            I16(ref c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 16-bit unsigned integer value.
    pub fn uint16(&self) -> Option<u16> {
        use self::PrimitiveValue::*;
        match self {
            U16(c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 8-bit unsigned integer value.
    pub fn uint8(&self) -> Option<u8> {
        use self::PrimitiveValue::*;
        match self {
            U8(c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 32-bit floating point number value.
    pub fn float32(&self) -> Option<f32> {
        use self::PrimitiveValue::*;
        match self {
            F32(c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Get a single 64-bit floating point number value.
    pub fn float64(&self) -> Option<f64> {
        use self::PrimitiveValue::*;
        match self {
            F64(c) => c.first().cloned(),
            _ => None,
        }
    }

    /// Determine the minimum number of bytes that this value would need to
    /// occupy in a DICOM file, without compression and without the header.
    /// As mandated by the standard, it is always even.
    /// The calculated number does not need to match the size of the original
    /// byte stream.
    pub fn calculate_byte_len(&self) -> usize {
        use self::PrimitiveValue::*;
        match self {
            Empty => 0,
            U8(c) => c.len(),
            I16(c) => c.len() * 2,
            U16(c) => c.len() * 2,
            U32(c) => c.len() * 4,
            I32(c) => c.len() * 4,
            U64(c) => c.len() * 8,
            I64(c) => c.len() * 8,
            F32(c) => c.len() * 4,
            F64(c) => c.len() * 8,
            Tags(c) => c.len() * 4,
            Date(c) => c.len() * 8,
            Str(s) => s.as_bytes().len(),
            Strs(c) if c.is_empty() => 0,
            Strs(c) => {
                c.iter()
                    .map(|s| ((s.as_bytes().len() + 1) & !1) + 1)
                    .sum::<usize>()
                    - 1
            }
            Time(c) if c.is_empty() => 0,
            Time(c) => {
                c.iter()
                    .map(|t| ((PrimitiveValue::tm_byte_len(*t) + 1) & !1) + 1)
                    .sum::<usize>()
                    - 1
            }
            DateTime(c) if c.is_empty() => 0,
            DateTime(c) => {
                c.iter()
                    .map(|dt| ((PrimitiveValue::dt_byte_len(*dt) + 1) & !1) + 1)
                    .sum::<usize>()
                    - 1
            }
        }
    }

    fn tm_byte_len(time: NaiveTime) -> usize {
        match (time.hour(), time.minute(), time.second(), time.nanosecond()) {
            (_, 0, 0, 0) => 2,
            (_, _, 0, 0) => 4,
            (_, _, _, 0) => 6,
            (_, _, _, nano) => {
                let mut frac = nano / 1000; // nano to microseconds
                let mut trailing_zeros = 0;
                while frac % 10 == 0 {
                    frac /= 10;
                    trailing_zeros += 1;
                }
                7 + 6 - trailing_zeros
            }
        }
    }

    fn dt_byte_len(datetime: DateTime<FixedOffset>) -> usize {
        // !!! the current local definition of datetime is inaccurate, because
        // it cannot distinguish unspecified components from their defaults
        // (e.g. 201812 should be different from 20181201). This will have to
        // be changed at some point.
        (match (datetime.month(), datetime.day()) {
            (1, 1) => 0,
            (_, 1) => 2,
            _ => 4,
        }) + 8
            + PrimitiveValue::tm_byte_len(datetime.time())
            + if datetime.offset() == &FixedOffset::east(0) {
                0
            } else {
                5
            }
    }
}

impl HasLength for PrimitiveValue {
    fn length(&self) -> Length {
        Length::defined(self.calculate_byte_len() as u32)
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

    /// A sequence of signed 64-bit integers.
    /// Used for SV.
    I64,

    /// A sequence of unsigned 64-bit integers.
    /// Used for UV and OV.
    U64,

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
pub trait DicomValueType: HasLength {
    /// Retrieve the specific type of this value.
    fn value_type(&self) -> ValueType;

    /// Retrieve the number of values contained.
    fn cardinality(&self) -> usize;

    /// Check whether the value is empty (0 length).
    fn is_empty(&self) -> bool {
        self.length() == Length(0)
    }
}

impl DicomValueType for PrimitiveValue {
    fn value_type(&self) -> ValueType {
        use self::PrimitiveValue::*;
        match *self {
            Empty => ValueType::Empty,
            Date(_) => ValueType::Date,
            DateTime(_) => ValueType::DateTime,
            F32(_) => ValueType::F32,
            F64(_) => ValueType::F64,
            I16(_) => ValueType::I16,
            I32(_) => ValueType::I32,
            I64(_) => ValueType::I64,
            Str(_) => ValueType::Str,
            Strs(_) => ValueType::Strs,
            Tags(_) => ValueType::Tags,
            Time(_) => ValueType::Time,
            U16(_) => ValueType::U16,
            U32(_) => ValueType::U32,
            U64(_) => ValueType::U64,
            U8(_) => ValueType::U8,
        }
    }

    fn cardinality(&self) -> usize {
        use self::PrimitiveValue::*;
        match self {
            Empty => 0,
            Str(_) => 1,
            Date(b) => b.len(),
            DateTime(b) => b.len(),
            F32(b) => b.len(),
            F64(b) => b.len(),
            I16(b) => b.len(),
            I32(b) => b.len(),
            I64(b) => b.len(),
            Strs(b) => b.len(),
            Tags(b) => b.len(),
            Time(b) => b.len(),
            U16(b) => b.len(),
            U32(b) => b.len(),
            U64(b) => b.len(),
            U8(b) => b.len(),
        }
    }
}

impl<I> HasLength for Value<I> {
    fn length(&self) -> Length {
        match self {
            Value::Primitive(v) => v.length(),
            Value::Sequence { size, .. } => *size,
        }
    }
}

impl<I> DicomValueType for Value<I> {
    fn value_type(&self) -> ValueType {
        match self {
            Value::Primitive(v) => v.value_type(),
            Value::Sequence { .. } => ValueType::Item,
        }
    }

    fn cardinality(&self) -> usize {
        match self {
            Value::Primitive(v) => v.cardinality(),
            Value::Sequence { items, .. } => items.len(),
        }
    }
}
