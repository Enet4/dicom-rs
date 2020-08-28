//! This module includes a high level abstraction over a DICOM data element's value.

use crate::header::{HasLength, Length, Tag};
use smallvec::SmallVec;
use std::borrow::Cow;

pub mod deserialize;
mod primitive;
pub mod serialize;

pub use self::primitive::{InvalidValueReadError, CastValueError, PrimitiveValue, ValueType};
pub use self::deserialize::Error as DeserializeError;

/// re-exported from chrono
use chrono::{DateTime, NaiveDate, NaiveTime};

/// An aggregation of one or more elements in a value.
pub type C<T> = SmallVec<[T; 2]>;

/// A trait for a value that maps to a DICOM element data value.
pub trait DicomValueType: HasLength {
    /// Retrieve the specific type of this value.
    fn value_type(&self) -> ValueType;

    /// Retrieve the number of elements contained in the DICOM value.
    ///
    /// In a sequence value, this is the number of items in the sequence.
    /// In an encapsulated pixel data sequence, the output is always 1.
    /// Otherwise, the output is the number of elements effectively encoded
    /// in the value.
    fn cardinality(&self) -> usize;
}

/// Representation of a full DICOM value, which may be either primitive or
/// another DICOM object.
///
/// `I` is the complex type for nest data set items, which should usually
/// implement [`HasLength`].
/// `P` is the encapsulated pixel data provider, which should usually
/// implement `AsRef<[u8]>`.
///
/// [`HasLength`]: ../header/trait.HasLength.html
#[derive(Debug, Clone, PartialEq)]
pub enum Value<I, P> {
    /// Primitive value.
    Primitive(PrimitiveValue),
    /// A complex sequence of items.
    Sequence {
        /// Item collection.
        items: C<I>,
        /// The size in bytes.
        size: Length,
    },
    /// An encapsulated pixel data sequence.
    PixelSequence {
        /// The value contents of the offset table.
        offset_table: C<u8>,
        /// The sequence of compressed fragments.
        fragments: C<P>,
    },
}

impl<I, P> Value<I, P> {
    /// Obtain the number of individual values.
    /// In a sequence item, this is the number of items. In a pixel sequence,
    /// this is currently set to 1 regardless of the number of compressed
    /// fragments or frames.
    pub fn multiplicity(&self) -> u32 {
        match *self {
            Value::Primitive(ref v) => v.multiplicity(),
            Value::Sequence { ref items, .. } => items.len() as u32,
            Value::PixelSequence { .. } => 1,
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

    /// Gets a reference to the encapsulated pixel data's offset table.
    pub fn offset_table(&self) -> Option<&[u8]> {
        match self {
            Value::PixelSequence { offset_table, .. } => Some(&offset_table),
            _ => None,
        }
    }
}

impl<I, P> HasLength for Value<I, P> {
    fn length(&self) -> Length {
        match self {
            Value::Primitive(v) => v.length(),
            Value::Sequence { size, .. } => *size,
            Value::PixelSequence { .. } => Length::UNDEFINED,
        }
    }
}

impl<I, P> DicomValueType for Value<I, P> {
    fn value_type(&self) -> ValueType {
        match self {
            Value::Primitive(v) => v.value_type(),
            Value::Sequence { .. } => ValueType::Item,
            Value::PixelSequence { .. } => ValueType::PixelSequence,
        }
    }

    fn cardinality(&self) -> usize {
        match self {
            Value::Primitive(v) => v.cardinality(),
            Value::Sequence { items, .. } => items.len(),
            Value::PixelSequence { .. } => 1,
        }
    }
}

impl<I, P> Value<I, P>
where
    I: HasLength,
{
    /// Convert the full primitive value into a single string.
    ///
    /// If the value contains multiple strings, they are concatenated
    /// (separated by the standard DICOM value delimiter `'\\'`)
    /// into an owned string.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_str(&self) -> Result<Cow<str>, CastValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_str()),
            _ => Err(CastValueError {
                requested: "string",
                got: self.value_type(),
            }),
        }
    }

    /// Convert the full primitive value into raw bytes.
    ///
    /// String values already encoded with the `Str` and `Strs` variants
    /// are provided in UTF-8.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_bytes(&self) -> Result<Cow<[u8]>, CastValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_bytes()),
            _ => Err(CastValueError {
                requested: "bytes",
                got: self.value_type(),
            }),
        }
    }

    /// Retrieves the primitive value as a sequence of unsigned bytes
    /// without conversions.
    pub fn as_u8(&self) -> Result<&[u8], CastValueError> {
        match self {
            Value::Primitive(PrimitiveValue::U8(v)) => Ok(&v),
            _ => Err(CastValueError {
                requested: "u8",
                got: self.value_type(),
            }),
        }
    }

    /// Retrieves the primitive value as a sequence of signed 32-bit integers
    /// without conversions.
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

impl<I, P> From<PrimitiveValue> for Value<I, P> {
    fn from(v: PrimitiveValue) -> Self {
        Value::Primitive(v)
    }
}
