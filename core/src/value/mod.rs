//! This module includes a high level abstraction over a DICOM data element's value.

use crate::header::{EmptyObject, HasLength, Length, Tag};
use num_traits::NumCast;
use smallvec::SmallVec;
use std::{borrow::Cow, str::FromStr};

pub mod deserialize;
pub mod fragments;
pub mod partial;
pub mod person_name;
mod primitive;
pub mod range;
pub mod serialize;

pub use self::deserialize::Error as DeserializeError;
pub use self::partial::{DicomDate, DicomDateTime, DicomTime, PreciseDateTime};
pub use self::person_name::PersonName;
pub use self::range::{AsRange, DateRange, DateTimeRange, TimeRange};

pub use self::primitive::{
    CastValueError, ConvertValueError, InvalidValueReadError, ModifyValueError, PrimitiveValue,
    ValueType,
};

pub use either::Either;

/// An aggregation of one or more elements in a value.
pub type C<T> = SmallVec<[T; 2]>;

/// Type alias for the in-memory pixel data fragment data.
pub type InMemFragment = Vec<u8>;

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

impl<L, R> HasLength for Either<L, R>
where
    L: HasLength,
    R: HasLength,
{
    fn length(&self) -> Length {
        match self {
            Either::Left(l) => l.length(),
            Either::Right(r) => r.length(),
        }
    }
}

impl<L, R> DicomValueType for Either<L, R>
where
    L: DicomValueType,
    R: DicomValueType,
{
    fn value_type(&self) -> ValueType {
        match self {
            Either::Left(l) => l.value_type(),
            Either::Right(r) => r.value_type(),
        }
    }

    fn cardinality(&self) -> usize {
        match self {
            Either::Left(l) => l.cardinality(),
            Either::Right(r) => r.cardinality(),
        }
    }
}

/// Representation of a full DICOM value, which may be either primitive or
/// another DICOM object.
///
/// `I` is the complex type for nest data set items, which should usually
/// implement [`HasLength`].
/// `P` is the encapsulated pixel data provider,
/// which should usually implement `AsRef<[u8]>`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<I = EmptyObject, P = InMemFragment> {
    /// Primitive value.
    Primitive(PrimitiveValue),
    /// A complex sequence of items.
    Sequence(DataSetSequence<I>),
    /// A sequence of encapsulated pixel data fragments.
    PixelSequence(PixelFragmentSequence<P>),
}

impl<P> Value<EmptyObject, P> {
    /// Construct an isolated DICOM pixel sequence sequence value
    /// from a basic offset table and a list of fragments.
    ///
    /// This function will define the data set sequence item type `I`
    /// to an empty object ([`EmptyObject`]),
    /// so that it can be used more easily in isolation.
    /// As a consequence, it cannot be directly combined with
    /// DICOM objects that may contain sequence values.
    /// To let the type parameter `I` be inferred from its context,
    /// create a [`PixelFragmentSequence`] and use `Value::from` instead.
    ///
    /// **Note:** This function does not validate the offset table
    /// against the fragments.
    pub fn new_pixel_sequence<T>(offset_table: C<u32>, fragments: T) -> Self
    where
        T: Into<C<P>>,
    {
        Value::from(PixelFragmentSequence::new(offset_table, fragments))
    }
}

impl<I> Value<I> {
    /// Construct an isolated DICOM data set sequence value
    /// from a list of items and length.
    ///
    /// This function will define the pixel data fragment type parameter `P`
    /// to the `Value` type's default ([`InMemFragment`]),
    /// so that it can be used more easily.
    /// If necessary,
    /// it is possible to let this type parameter be inferred from its context
    /// by creating a [`DataSetSequence`] and using `Value::from` instead.
    #[inline]
    pub fn new_sequence<T>(items: T, length: Length) -> Self
    where
        T: Into<C<I>>,
    {
        Self::from(DataSetSequence::new(items, length))
    }
}

impl Value {
    /// Construct a DICOM value from a primitive value.
    ///
    /// This is equivalent to `Value::from` in behavior,
    /// except that suitable type parameters are specified
    /// instead of inferred.
    ///
    /// This function will automatically define
    /// the sequence item parameter `I`
    /// to [`EmptyObject`]
    /// and the pixel data fragment type parameter `P`
    /// to the default fragment data type ([`InMemFragment`]),
    /// so that it can be used more easily in isolation.
    /// As a consequence, it cannot be directly combined with
    /// DICOM objects that may contain
    /// nested data sets or encapsulated pixel data.
    /// To let the type parameters `I` and `P` be inferred from their context,
    /// create a value of one of the types and use `Value::from` instead.
    ///
    /// - [`PrimitiveValue`]
    /// - [`PixelFragmentSequence`]
    /// - [`DataSetSequence`]
    #[inline]
    pub fn new(value: PrimitiveValue) -> Self {
        Self::from(value)
    }
}

impl<I, P> Value<I, P> {
    /// Obtain the number of individual values.
    /// In a primitive, this is the number of individual elements in the value.
    /// In a sequence item, this is the number of items.
    /// In a pixel sequence, this is currently set to 1
    /// regardless of the number of compressed fragments or frames.
    pub fn multiplicity(&self) -> u32 {
        match self {
            Value::Primitive(v) => v.multiplicity(),
            Value::Sequence(v) => v.multiplicity(),
            Value::PixelSequence(..) => 1,
        }
    }

    /// Gets a reference to the primitive value.
    pub fn primitive(&self) -> Option<&PrimitiveValue> {
        match self {
            Value::Primitive(v) => Some(v),
            _ => None,
        }
    }

    /// Produce a shallow clone of the value,
    /// leaving the items and pixel data fragments as references.
    /// 
    /// If the value is primitive,
    /// the entire value will be copied.
    /// Otherwise, the item or fragment sequences
    /// will hold references to the original data.
    pub fn shallow_clone<'a>(&'a self) -> Value<&'a I, &'a P> {
        match self {
            Value::Primitive(v) => Value::Primitive(v.clone()),
            Value::Sequence(v) => Value::Sequence(DataSetSequence {
                items: v.items.iter().collect(),
                length: v.length,
            }),
            Value::PixelSequence(v) => Value::PixelSequence(PixelFragmentSequence {
                offset_table: v.offset_table.iter().copied().collect(),
                fragments: v.fragments.iter().collect(),
            }),
        }
    }

    /// Gets a mutable reference to the primitive value.
    pub fn primitive_mut(&mut self) -> Option<&mut PrimitiveValue> {
        match self {
            Value::Primitive(v) => Some(v),
            _ => None,
        }
    }

    /// Gets a reference to the items of a sequence.
    ///
    /// Returns `None` if the value is not a data set sequence.
    pub fn items(&self) -> Option<&[I]> {
        match self {
            Value::Sequence(v) => Some(v.items()),
            _ => None,
        }
    }

    /// Gets a mutable reference to the items of a sequence.
    ///
    /// Returns `None` if the value is not a data set sequence.
    pub fn items_mut(&mut self) -> Option<&mut C<I>> {
        match self {
            Value::Sequence(v) => Some(v.items_mut()),
            _ => None,
        }
    }

    /// Gets a reference to the fragments of a pixel data sequence.
    ///
    /// Returns `None` if the value is not a pixel data sequence.
    pub fn fragments(&self) -> Option<&[P]> {
        match self {
            Value::PixelSequence(v) => Some(v.fragments()),
            _ => None,
        }
    }

    /// Gets a mutable reference to the fragments of a pixel data sequence.
    ///
    /// Returns `None` if the value is not a pixel data sequence.
    pub fn fragments_mut(&mut self) -> Option<&mut C<P>> {
        match self {
            Value::PixelSequence(v) => Some(v.fragments_mut()),
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

    /// Retrieves the data set items,
    /// discarding the recorded length information.
    ///
    /// Returns `None` if the value is not a data set sequence.
    pub fn into_items(self) -> Option<C<I>> {
        match self {
            Value::Sequence(v) => Some(v.into_items()),
            _ => None,
        }
    }

    /// Retrieves the pixel data fragments,
    /// discarding the rest of the information.
    pub fn into_fragments(self) -> Option<C<P>> {
        match self {
            Value::PixelSequence(v) => Some(v.into_fragments()),
            _ => None,
        }
    }

    /// Gets a reference to the encapsulated pixel data's offset table.
    ///
    /// Returns `None` if the value is not a pixel data sequence.
    pub fn offset_table(&self) -> Option<&[u32]> {
        match self {
            Value::PixelSequence(v) => Some(v.offset_table()),
            _ => None,
        }
    }

    /// Gets a mutable reference to the encapsulated pixel data's offset table.
    ///
    /// Returns `None` if the value is not a pixel data sequence.
    pub fn offset_table_mut(&mut self) -> Option<&mut C<u32>> {
        match self {
            Value::PixelSequence(v) => Some(v.offset_table_mut()),
            _ => None,
        }
    }

    /// Shorten this value by removing trailing elements
    /// to fit the given limit.
    ///
    /// On primitive values,
    /// elements are counted by the number of individual value items
    /// (note that bytes in a [`PrimitiveValue::U8`]
    /// are treated as individual items).
    /// On data set sequences and pixel data fragment sequences,
    /// this operation is applied to
    /// the data set items (or fragments) in the sequence.
    ///
    /// Nothing is done if the value's cardinality
    /// is already lower than or equal to the limit.
    pub fn truncate(&mut self, limit: usize) {
        match self {
            Value::Primitive(v) => v.truncate(limit),
            Value::Sequence(v) => v.truncate(limit),
            Value::PixelSequence(v) => v.truncate(limit),
        }
    }
}

impl<I, P> From<&str> for Value<I, P> {
    /// Converts a string into a primitive textual value.
    fn from(value: &str) -> Self {
        Value::Primitive(PrimitiveValue::from(value))
    }
}

impl<I, P> From<String> for Value<I, P> {
    /// Converts a string into a primitive textual value.
    fn from(value: String) -> Self {
        Value::Primitive(PrimitiveValue::from(value))
    }
}

impl<I, P> From<DicomDate> for Value<I, P> {
    /// Converts the DICOM date into a primitive value.
    fn from(value: DicomDate) -> Self {
        Value::Primitive(PrimitiveValue::from(value))
    }
}

impl<I, P> From<DicomTime> for Value<I, P> {
    /// Converts the DICOM time into a primitive value.
    fn from(value: DicomTime) -> Self {
        Value::Primitive(PrimitiveValue::from(value))
    }
}

impl<I, P> From<DicomDateTime> for Value<I, P> {
    /// Converts the DICOM date-time into a primitive value.
    fn from(value: DicomDateTime) -> Self {
        Value::Primitive(PrimitiveValue::from(value))
    }
}

impl<I, P> HasLength for Value<I, P> {
    fn length(&self) -> Length {
        match self {
            Value::Primitive(v) => v.length(),
            Value::Sequence(v) => v.length(),
            Value::PixelSequence(v) => v.length(),
        }
    }
}

impl<I, P> HasLength for &Value<I, P> {
    fn length(&self) -> Length {
        HasLength::length(*self)
    }
}

impl<I, P> DicomValueType for Value<I, P> {
    fn value_type(&self) -> ValueType {
        match self {
            Value::Primitive(v) => v.value_type(),
            Value::Sequence(..) => ValueType::DataSetSequence,
            Value::PixelSequence(..) => ValueType::PixelSequence,
        }
    }

    fn cardinality(&self) -> usize {
        match self {
            Value::Primitive(v) => v.cardinality(),
            Value::Sequence(DataSetSequence { items, .. }) => items.len(),
            Value::PixelSequence { .. } => 1,
        }
    }
}

impl<I, P> DicomValueType for &Value<I, P> {
    fn value_type(&self) -> ValueType {
        DicomValueType::value_type(*self)
    }

    fn cardinality(&self) -> usize {
        DicomValueType::cardinality(*self)
    }
}

impl<I, P> Value<I, P>
where
    I: HasLength,
{
    /// Convert the full primitive value into a clean string.
    ///
    /// The value is converted into a strings
    /// as described in [`PrimitiveValue::to_str`].
    /// If the value contains multiple strings,
    /// they are trimmed at the end and concatenated
    /// (separated by the standard DICOM value delimiter `'\\'`)
    /// into an owned string.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_str(&self) -> Result<Cow<'_, str>, ConvertValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_str()),
            _ => Err(ConvertValueError {
                requested: "string",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Convert the full primitive value into a single raw string,
    /// with trailing whitespace kept.
    ///
    /// If the value contains multiple strings, they are concatenated
    /// (separated by the standard DICOM value delimiter `'\\'`)
    /// into an owned string.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_raw_str(&self) -> Result<Cow<'_, str>, ConvertValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_raw_str()),
            _ => Err(ConvertValueError {
                requested: "string",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Convert the full primitive value into a sequence of strings.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of strings as described in [`PrimitiveValue::to_multi_str`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_multi_str`]: ../enum.PrimitiveValue.html#to_multi_str
    pub fn to_multi_str(&self) -> Result<Cow<'_, [String]>, CastValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_multi_str()),
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
    pub fn to_bytes(&self) -> Result<Cow<'_, [u8]>, ConvertValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_bytes()),
            _ => Err(ConvertValueError {
                requested: "bytes",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into an integer.
    ///
    /// If the value is a primitive, it will be converted into
    /// an integer as described in [`PrimitiveValue::to_int`].
    ///
    /// [`PrimitiveValue::to_int`]: ../enum.PrimitiveValue.html#to_int
    pub fn to_int<T>(&self) -> Result<T, ConvertValueError>
    where
        T: Clone,
        T: NumCast,
        T: FromStr<Err = std::num::ParseIntError>,
    {
        match self {
            Value::Primitive(v) => v.to_int::<T>(),
            _ => Err(ConvertValueError {
                requested: "integer",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a sequence of integers.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of integers as described in [PrimitiveValue::to_multi_int].
    ///
    /// [PrimitiveValue::to_multi_int]: ../enum.PrimitiveValue.html#to_multi_int
    pub fn to_multi_int<T>(&self) -> Result<Vec<T>, ConvertValueError>
    where
        T: Clone,
        T: NumCast,
        T: FromStr<Err = std::num::ParseIntError>,
    {
        match self {
            Value::Primitive(v) => v.to_multi_int::<T>(),
            _ => Err(ConvertValueError {
                requested: "integer",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value
    /// into a single-precision floating point number.
    ///
    /// If the value is a primitive, it will be converted into
    /// a number as described in [`PrimitiveValue::to_float32`].
    ///
    /// [`PrimitiveValue::to_float32`]: ../enum.PrimitiveValue.html#to_float32
    pub fn to_float32(&self) -> Result<f32, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_float32(),
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value
    /// into a sequence of single-precision floating point numbers.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of numbers as described in [`PrimitiveValue::to_multi_float32`].
    ///
    /// [`PrimitiveValue::to_multi_float32`]: ../enum.PrimitiveValue.html#to_multi_float32
    pub fn to_multi_float32(&self) -> Result<Vec<f32>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_float32(),
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value
    /// into a double-precision floating point number.
    ///
    /// If the value is a primitive, it will be converted into
    /// a number as described in [`PrimitiveValue::to_float64`].
    ///
    /// [`PrimitiveValue::to_float64`]: ../enum.PrimitiveValue.html#to_float64
    pub fn to_float64(&self) -> Result<f64, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_float64(),
            _ => Err(ConvertValueError {
                requested: "float64",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value
    /// into a sequence of double-precision floating point numbers.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of numbers as described in [`PrimitiveValue::to_multi_float64`].
    ///
    /// [`PrimitiveValue::to_multi_float64`]: ../enum.PrimitiveValue.html#to_multi_float64
    pub fn to_multi_float64(&self) -> Result<Vec<f64>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_float64(),
            _ => Err(ConvertValueError {
                requested: "float64",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a `DicomDate`.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DicomDate` as described in [`PrimitiveValue::to_date`].
    ///
    pub fn to_date(&self) -> Result<DicomDate, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_date(),
            _ => Err(ConvertValueError {
                requested: "DicomDate",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a sequence of `DicomDate`s.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `DicomDate` as described in [`PrimitiveValue::to_multi_date`].
    ///
    pub fn to_multi_date(&self) -> Result<Vec<DicomDate>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_date(),
            _ => Err(ConvertValueError {
                requested: "DicomDate",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a `DicomTime`.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DicomTime` as described in [`PrimitiveValue::to_time`].
    ///
    pub fn to_time(&self) -> Result<DicomTime, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_time(),
            _ => Err(ConvertValueError {
                requested: "DicomTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a sequence of `DicomTime`s.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `DicomTime` as described in [`PrimitiveValue::to_multi_time`].
    ///
    pub fn to_multi_time(&self) -> Result<Vec<DicomTime>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_time(),
            _ => Err(ConvertValueError {
                requested: "DicomTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a `DicomDateTime`.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DateTime` as described in [`PrimitiveValue::to_datetime`].
    ///
    pub fn to_datetime(&self) -> Result<DicomDateTime, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_datetime(),
            _ => Err(ConvertValueError {
                requested: "DicomDateTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a sequence of `DicomDateTime`s.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `DicomDateTime` as described in [`PrimitiveValue::to_multi_datetime`].
    ///
    pub fn to_multi_datetime(&self) -> Result<Vec<DicomDateTime>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_datetime(),
            _ => Err(ConvertValueError {
                requested: "DicomDateTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a `DateRange`.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DateRange` as described in [`PrimitiveValue::to_date_range`].
    ///
    pub fn to_date_range(&self) -> Result<DateRange, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_date_range(),
            _ => Err(ConvertValueError {
                requested: "DateRange",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a `TimeRange`.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `TimeRange` as described in [`PrimitiveValue::to_time_range`].
    ///
    pub fn to_time_range(&self) -> Result<TimeRange, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_time_range(),
            _ => Err(ConvertValueError {
                requested: "TimeRange",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a `DateTimeRange`.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DateTimeRange` as described in [`PrimitiveValue::to_datetime_range`].
    ///
    pub fn to_datetime_range(&self) -> Result<DateTimeRange, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_datetime_range(),
            _ => Err(ConvertValueError {
                requested: "DateTimeRange",
                original: self.value_type(),
                cause: None,
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

    /// Retrieves the primitive value as a [`PersonName`].
    pub fn to_person_name(&self) -> Result<PersonName<'_>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_person_name(),
            _ => Err(ConvertValueError {
                requested: "PersonName",
                original: self.value_type(),
                cause: None,
            }),
        }
    }
}

/// Macro for implementing getters to single and multi-values,
/// by delegating to `PrimitiveValue`.
///
/// Should be placed inside `Value`'s impl block.
macro_rules! impl_primitive_getters {
    ($name_single: ident, $name_multi: ident, $variant: ident, $ret: ty) => {
        /// Get a single value of the requested type.
        ///
        /// If it contains multiple values,
        /// only the first one is returned.
        /// An error is returned if the variant is not compatible.
        pub fn $name_single(&self) -> Result<$ret, CastValueError> {
            match self {
                Value::Primitive(v) => v.$name_single(),
                value => Err(CastValueError {
                    requested: stringify!($name_single),
                    got: value.value_type(),
                }),
            }
        }

        /// Get a sequence of values of the requested type without copying.
        ///
        /// An error is returned if the variant is not compatible.
        pub fn $name_multi(&self) -> Result<&[$ret], CastValueError> {
            match self {
                Value::Primitive(v) => v.$name_multi(),
                value => Err(CastValueError {
                    requested: stringify!($name_multi),
                    got: value.value_type(),
                }),
            }
        }
    };
}

impl<I, P> Value<I, P> {
    /// Get a single string value.
    ///
    /// If it contains multiple strings,
    /// only the first one is returned.
    ///
    /// An error is returned if the variant is not compatible.
    ///
    /// To enable conversions of other variants to a textual representation,
    /// see [`to_str()`] instead.
    ///
    /// [`to_str()`]: #method.to_str
    pub fn string(&self) -> Result<&str, CastValueError> {
        match self {
            Value::Primitive(v) => v.string(),
            _ => Err(CastValueError {
                requested: "string",
                got: self.value_type(),
            }),
        }
    }

    /// Get the inner sequence of string values
    /// if the variant is either `Str` or `Strs`.
    ///
    /// An error is returned if the variant is not compatible.
    ///
    /// To enable conversions of other variants to a textual representation,
    /// see [`to_str()`] instead.
    ///
    /// [`to_str()`]: #method.to_str
    pub fn strings(&self) -> Result<&[String], CastValueError> {
        match self {
            Value::Primitive(v) => v.strings(),
            _ => Err(CastValueError {
                requested: "strings",
                got: self.value_type(),
            }),
        }
    }

    impl_primitive_getters!(tag, tags, Tags, Tag);
    impl_primitive_getters!(date, dates, Date, DicomDate);
    impl_primitive_getters!(time, times, Time, DicomTime);
    impl_primitive_getters!(datetime, datetimes, DateTime, DicomDateTime);
    impl_primitive_getters!(uint8, uint8_slice, U8, u8);
    impl_primitive_getters!(uint16, uint16_slice, U16, u16);
    impl_primitive_getters!(int16, int16_slice, I16, i16);
    impl_primitive_getters!(uint32, uint32_slice, U32, u32);
    impl_primitive_getters!(int32, int32_slice, I32, i32);
    impl_primitive_getters!(int64, int64_slice, I64, i64);
    impl_primitive_getters!(uint64, uint64_slice, U64, u64);
    impl_primitive_getters!(float32, float32_slice, F32, f32);
    impl_primitive_getters!(float64, float64_slice, F64, f64);
}

impl<I, P> From<PrimitiveValue> for Value<I, P> {
    fn from(v: PrimitiveValue) -> Self {
        Value::Primitive(v)
    }
}

/// A sequence of complex data set items of type `I`.
#[derive(Debug, Clone)]
pub struct DataSetSequence<I> {
    /// The item sequence.
    items: C<I>,
    /// The sequence length in bytes.
    ///
    /// The value may be [`UNDEFINED`](Length::UNDEFINED)
    /// if the length is implicitly defined,
    /// otherwise it should match the full byte length of all items.
    length: Length,
}

impl<I> DataSetSequence<I> {
    /// Construct a DICOM data sequence
    /// using a sequence of items and a length.
    ///
    /// **Note:** This function does not validate the `length`
    /// against the items.
    /// When not sure,
    /// `length` can be set to [`UNDEFINED`](Length::UNDEFINED)
    /// to leave it as implicitly defined.
    #[inline]
    pub fn new(items: impl Into<C<I>>, length: Length) -> Self {
        DataSetSequence {
            items: items.into(),
            length,
        }
    }

    /// Construct an empty DICOM data sequence,
    /// with the length explicitly defined to zero.
    #[inline]
    pub fn empty() -> Self {
        DataSetSequence {
            items: Default::default(),
            length: Length(0),
        }
    }

    /// Gets a reference to the items of a sequence.
    #[inline]
    pub fn items(&self) -> &[I] {
        &self.items
    }

    /// Gets a mutable reference to the items of a sequence.
    #[inline]
    pub fn items_mut(&mut self) -> &mut C<I> {
        &mut self.items
    }

    /// Obtain the number of items in the sequence.
    #[inline]
    pub fn multiplicity(&self) -> u32 {
        self.items.len() as u32
    }

    /// Retrieve the sequence of items,
    /// discarding the recorded length information.
    #[inline]
    pub fn into_items(self) -> C<I> {
        self.items
    }

    /// Get the value data's length
    /// as specified by the sequence's data element,
    /// in bytes.
    ///
    /// This is equivalent to [`HasLength::length`].
    #[inline]
    pub fn length(&self) -> Length {
        HasLength::length(self)
    }

    /// Shorten this sequence by removing trailing data set items
    /// to fit the given limit.
    #[inline]
    pub fn truncate(&mut self, limit: usize) {
        self.items.truncate(limit);
    }
}

impl<I> HasLength for DataSetSequence<I> {
    #[inline]
    fn length(&self) -> Length {
        self.length
    }
}

impl<I> DicomValueType for DataSetSequence<I> {
    #[inline]
    fn value_type(&self) -> ValueType {
        ValueType::DataSetSequence
    }

    #[inline]
    fn cardinality(&self) -> usize {
        self.items.len()
    }
}

impl<I> From<Vec<I>> for DataSetSequence<I> {
    /// Converts a vector of items
    /// into a data set sequence with an undefined length.
    #[inline]
    fn from(items: Vec<I>) -> Self {
        DataSetSequence {
            items: items.into(),
            length: Length::UNDEFINED,
        }
    }
}

impl<A, I> From<SmallVec<A>> for DataSetSequence<I>
where
    A: smallvec::Array<Item = I>,
    C<I>: From<SmallVec<A>>,
{
    /// Converts a smallvec of items
    /// into a data set sequence with an undefined length.
    #[inline]
    fn from(items: SmallVec<A>) -> Self {
        DataSetSequence {
            items: items.into(),
            length: Length::UNDEFINED,
        }
    }
}

impl<I> From<[I; 1]> for DataSetSequence<I> {
    /// Constructs a data set sequence with a single item
    /// and an undefined length.
    #[inline]
    fn from([item]: [I; 1]) -> Self {
        DataSetSequence {
            items: smallvec::smallvec![item],
            length: Length::UNDEFINED,
        }
    }
}

impl<I, P> From<DataSetSequence<I>> for Value<I, P> {
    #[inline]
    fn from(value: DataSetSequence<I>) -> Self {
        Value::Sequence(value)
    }
}

impl<I> PartialEq<DataSetSequence<I>> for DataSetSequence<I>
where
    I: PartialEq,
{
    /// This method tests for `self` and `other` values to be equal,
    /// and is used by `==`.
    ///
    /// This implementation only checks for item equality,
    /// disregarding the byte length.
    #[inline]
    fn eq(&self, other: &DataSetSequence<I>) -> bool {
        self.items() == other.items()
    }
}

/// A sequence of pixel data fragments.
///
/// Each fragment (of data type `P`) is
/// an even-lengthed sequence of bytes
/// representing the encoded pixel data.
/// The first item of the sequence is interpreted as a basic offset table,
/// which is defined separately.
#[derive(Debug, Clone, PartialEq)]
pub struct PixelFragmentSequence<P> {
    /// The value contents of the basic offset table.
    offset_table: C<u32>,
    /// The sequence of pixel data fragments.
    fragments: C<P>,
}

impl<P> PixelFragmentSequence<P> {
    /// Construct a DICOM pixel sequence sequence value
    /// from a basic offset table and a list of fragments.
    ///
    /// **Note:** This function does not validate the offset table
    /// against the given fragments.
    #[inline]
    pub fn new(offset_table: impl Into<C<u32>>, fragments: impl Into<C<P>>) -> Self {
        PixelFragmentSequence {
            offset_table: offset_table.into(),
            fragments: fragments.into(),
        }
    }

    /// Construct a DICOM pixel sequence sequence value
    /// from a list of fragments,
    /// with an empty basic offset table.
    #[inline]
    pub fn new_fragments(fragments: impl Into<C<P>>) -> Self {
        PixelFragmentSequence {
            offset_table: Default::default(),
            fragments: fragments.into(),
        }
    }

    /// Gets a reference to the pixel data fragments.
    ///
    /// This sequence does not include the offset table.
    #[inline]
    pub fn fragments(&self) -> &[P] {
        &self.fragments
    }

    /// Gets a mutable reference to the pixel data fragments.
    ///
    /// This sequence does not include the offset table.
    #[inline]
    pub fn fragments_mut(&mut self) -> &mut C<P> {
        &mut self.fragments
    }

    /// Retrieve the pixel data fragments,
    /// discarding the rest of the information.
    ///
    /// This sequence does not include the offset table.
    #[inline]
    pub fn into_fragments(self) -> C<P> {
        self.fragments
    }

    /// Decompose the sequence into its constituent parts:
    /// the basic offset table and the pixel data fragments.
    pub fn into_parts(self) -> (C<u32>, C<P>) {
        (self.offset_table, self.fragments)
    }

    /// Gets a reference to the encapsulated pixel data's offset table.
    pub fn offset_table(&self) -> &[u32] {
        &self.offset_table
    }

    /// Gets a mutable reference to the encapsulated pixel data's offset table.
    pub fn offset_table_mut(&mut self) -> &mut C<u32> {
        &mut self.offset_table
    }

    /// Get the value data's length
    /// as specified by the sequence's data element,
    /// in bytes.
    ///
    /// This is equivalent to [`HasLength::length`].
    #[inline]
    pub fn length(&self) -> Length {
        HasLength::length(self)
    }

    /// Shorten this sequence by removing trailing fragments
    /// to fit the given limit.
    ///
    /// Note that this operations does not affect the basic offset table.
    #[inline]
    pub fn truncate(&mut self, limit: usize) {
        self.fragments.truncate(limit);
    }
}

impl<T, F, P> From<(T, F)> for PixelFragmentSequence<P>
where
    T: Into<C<u32>>,
    F: Into<C<P>>,
{
    /// Construct a pixel data fragment sequence,
    /// interpreting the first tuple element as a basic offset table
    /// and the second element as the vector of fragments.
    ///
    /// **Note:** This function does not validate the offset table
    /// against the given fragments.
    fn from((offset_table, fragments): (T, F)) -> Self {
        PixelFragmentSequence::new(offset_table, fragments)
    }
}

impl<I, P> From<PixelFragmentSequence<P>> for Value<I, P> {
    #[inline]
    fn from(value: PixelFragmentSequence<P>) -> Self {
        Value::PixelSequence(value)
    }
}

impl<P> HasLength for PixelFragmentSequence<P> {
    /// In standard DICOM,
    /// encapsulated pixel data is always defined by
    /// a pixel data element with an undefined length.
    #[inline]
    fn length(&self) -> Length {
        Length::UNDEFINED
    }
}

impl<P> DicomValueType for PixelFragmentSequence<P> {
    #[inline]
    fn value_type(&self) -> ValueType {
        ValueType::PixelSequence
    }

    #[inline]
    fn cardinality(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dicom_value;
    use smallvec::smallvec;

    #[test]
    fn to_int() {
        let value = Value::new(dicom_value!(I32, [1, 2, 5]));
        assert_eq!(value.to_int::<u32>().unwrap(), 1);
        assert_eq!(value.to_int::<i32>().unwrap(), 1);
        assert_eq!(value.to_int::<u16>().unwrap(), 1);
        assert_eq!(value.to_int::<i16>().unwrap(), 1);
        assert_eq!(value.to_int::<u64>().unwrap(), 1);
        assert_eq!(value.to_int::<i64>().unwrap(), 1);

        assert_eq!(value.to_multi_int::<i32>().unwrap(), vec![1, 2, 5]);
        assert_eq!(value.to_multi_int::<u32>().unwrap(), vec![1, 2, 5]);

        // sequence values can't be turned to an int
        let value = Value::<EmptyObject, _>::new_sequence(smallvec![], Length::UNDEFINED);

        assert!(matches!(
            value.to_int::<u32>(),
            Err(ConvertValueError {
                requested: "integer",
                original: ValueType::DataSetSequence,
                ..
            })
        ));
    }

    #[test]
    fn to_float() {
        let value = Value::new(dicom_value!(F64, [1., 2., 5.]));
        assert_eq!(value.to_float32().unwrap(), 1.);
        assert_eq!(value.to_float64().unwrap(), 1.);

        assert_eq!(value.to_multi_float32().unwrap(), vec![1., 2., 5.]);
        assert_eq!(value.to_multi_float64().unwrap(), vec![1., 2., 5.]);

        // sequence values can't be turned to a number
        let value = Value::<EmptyObject, _>::new_sequence(smallvec![], Length::UNDEFINED);

        assert!(matches!(
            value.to_float32(),
            Err(ConvertValueError {
                requested: "float32",
                original: ValueType::DataSetSequence,
                ..
            })
        ));
    }

    #[test]
    fn to_date() {
        let expected_dates = [
            DicomDate::from_ymd(2021, 2, 3).unwrap(),
            DicomDate::from_ymd(2022, 3, 4).unwrap(),
            DicomDate::from_ymd(2023, 4, 5).unwrap(),
        ];

        let value = Value::new(dicom_value!(Strs, ["20210203", "20220304", "20230405"]));
        assert_eq!(value.to_date().unwrap(), expected_dates[0],);
        assert_eq!(value.to_multi_date().unwrap(), &expected_dates[..]);

        let value_pair = Value::new(dicom_value!(
            Date,
            [
                DicomDate::from_ymd(2021, 2, 3).unwrap(),
                DicomDate::from_ymd(2022, 3, 4).unwrap(),
            ]
        ));

        assert_eq!(value_pair.to_date().unwrap(), expected_dates[0]);
        assert_eq!(value_pair.to_multi_date().unwrap(), &expected_dates[0..2]);

        // cannot turn to integers
        assert!(matches!(
            value_pair.to_multi_int::<i64>(),
            Err(ConvertValueError {
                requested: "integer",
                original: ValueType::Date,
                ..
            })
        ));

        let range_value = Value::new(dicom_value!(Str, "20210203-20220304"));

        // can turn to range
        assert_eq!(
            range_value.to_date_range().unwrap(),
            DateRange::from_start_to_end(
                expected_dates[0].to_naive_date().unwrap(),
                expected_dates[1].to_naive_date().unwrap()
            )
            .unwrap()
        );
    }

    #[test]
    fn getters() {
        assert_eq!(
            Value::new(dicom_value!(Strs, ["Smith^John"]))
                .string()
                .unwrap(),
            "Smith^John"
        );

        assert_eq!(
            Value::new(dicom_value!(Strs, ["Smith^John"]))
                .strings()
                .unwrap(),
            &["Smith^John"]
        );

        assert_eq!(Value::new(dicom_value!(I32, [1, 2, 5])).int32().unwrap(), 1,);

        assert_eq!(
            Value::new(dicom_value!(I32, [1, 2, 5]))
                .int32_slice()
                .unwrap(),
            &[1, 2, 5],
        );

        assert!(matches!(
            Value::new(dicom_value!(I32, [1, 2, 5])).uint32(),
            Err(CastValueError {
                requested: "uint32",
                got: ValueType::I32,
                ..
            })
        ));

        assert!(matches!(
            Value::new(dicom_value!(I32, [1, 2, 5])).strings(),
            Err(CastValueError {
                requested: "strings",
                got: ValueType::I32,
                ..
            })
        ));

        assert_eq!(
            Value::new(PrimitiveValue::Date(smallvec![DicomDate::from_ymd(
                2014, 10, 12
            )
            .unwrap()]))
            .date()
            .ok(),
            Some(DicomDate::from_ymd(2014, 10, 12).unwrap()),
        );

        assert_eq!(
            Value::new(PrimitiveValue::Date(
                smallvec![DicomDate::from_ymd(2014, 10, 12).unwrap(); 5]
            ))
            .dates()
            .unwrap(),
            &[DicomDate::from_ymd(2014, 10, 12).unwrap(); 5]
        );

        assert!(matches!(
            Value::new(PrimitiveValue::Date(smallvec![DicomDate::from_ymd(
                2014, 10, 12
            )
            .unwrap()]))
            .time(),
            Err(CastValueError {
                requested: "time",
                got: ValueType::Date,
                ..
            })
        ));
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct DummyItem(u32);

    impl HasLength for DummyItem {
        fn length(&self) -> Length {
            Length::defined(8)
        }
    }

    #[test]
    fn value_eq() {
        // the following declarations are equivalent
        let v1 = Value::<_, _>::from(PixelFragmentSequence::new(
            smallvec![],
            smallvec![vec![1, 2, 5]],
        ));
        let v2 = Value::new_pixel_sequence(smallvec![], smallvec![vec![1, 2, 5]]);
        assert_eq!(v1, v2);
        assert_eq!(v2, v1);

        // redeclare with different type parameters
        let v1 = Value::<DummyItem, _>::from(PixelFragmentSequence::new(
            smallvec![],
            smallvec![vec![1, 2, 5]],
        ));

        // declarations are equivalent
        let v3 = Value::from(PrimitiveValue::from("Something"));
        let v4 = Value::new(dicom_value!(Str, "Something"));
        let v3_2: Value = "Something".into();
        assert_eq!(v3, v4);
        assert_eq!(v3, v3_2);

        // redeclare with different type parameters
        let v3: Value<DummyItem, _> = PrimitiveValue::from("Something").into();

        let v5 = Value::from(DataSetSequence::new(
            vec![DummyItem(0), DummyItem(1), DummyItem(2)],
            Length::defined(1000),
        ));
        let v6 = Value::from(DataSetSequence::new(
            vec![DummyItem(0), DummyItem(1), DummyItem(2)],
            Length::UNDEFINED,
        ));
        assert_eq!(v5, v6);

        assert_ne!(v1, v3);
        assert_ne!(v3, v1);
        assert_ne!(v1, v6);
        assert_ne!(v6, v1);
        assert_ne!(v3, v6);
        assert_ne!(v6, v3);
    }

    #[test]
    fn data_set_sequences() {
        let v = DataSetSequence::new(
            vec![DummyItem(1), DummyItem(2), DummyItem(5)],
            Length::defined(24),
        );

        assert_eq!(v.cardinality(), 3);
        assert_eq!(v.value_type(), ValueType::DataSetSequence);
        assert_eq!(v.items(), &[DummyItem(1), DummyItem(2), DummyItem(5)]);
        assert_eq!(v.length(), Length(24));

        let v = Value::<_, [u8; 0]>::from(v);
        assert_eq!(v.value_type(), ValueType::DataSetSequence);
        assert_eq!(v.cardinality(), 3);
        assert_eq!(
            v.items(),
            Some(&[DummyItem(1), DummyItem(2), DummyItem(5)][..])
        );
        assert_eq!(v.primitive(), None);
        assert_eq!(v.fragments(), None);
        assert_eq!(v.offset_table(), None);
        assert_eq!(v.length(), Length(24));

        // can't turn sequence to string
        assert!(matches!(
            v.to_str(),
            Err(ConvertValueError {
                original: ValueType::DataSetSequence,
                ..
            })
        ));
        // can't turn sequence to bytes
        assert!(matches!(
            v.to_bytes(),
            Err(ConvertValueError {
                requested: "bytes",
                original: ValueType::DataSetSequence,
                ..
            })
        ));

        // can turn into items
        let items = v.into_items().unwrap();
        assert_eq!(&items[..], &[DummyItem(1), DummyItem(2), DummyItem(5)][..]);
    }

    #[test]
    fn pixel_fragment_sequences() {
        let v = PixelFragmentSequence::new(vec![], vec![vec![0x55; 128]]);

        assert_eq!(v.cardinality(), 1);
        assert_eq!(v.value_type(), ValueType::PixelSequence);
        assert_eq!(v.fragments(), &[vec![0x55; 128]]);
        assert!(HasLength::length(&v).is_undefined());

        let v = Value::<EmptyObject, _>::from(v);
        assert_eq!(v.cardinality(), 1);
        assert_eq!(v.value_type(), ValueType::PixelSequence);
        assert_eq!(v.items(), None);
        assert_eq!(v.primitive(), None);
        assert_eq!(v.fragments(), Some(&[vec![0x55; 128]][..]));
        assert_eq!(v.offset_table(), Some(&[][..]));
        assert!(HasLength::length(&v).is_undefined());

        // can't turn sequence to string
        assert!(matches!(
            v.to_str(),
            Err(ConvertValueError {
                requested: "string",
                original: ValueType::PixelSequence,
                ..
            })
        ));

        // can't turn sequence to bytes
        assert!(matches!(
            v.to_bytes(),
            Err(ConvertValueError {
                requested: "bytes",
                original: ValueType::PixelSequence,
                ..
            })
        ));

        // can turn into fragments
        let fragments = v.into_fragments().unwrap();
        assert_eq!(&fragments[..], &[vec![0x55; 128]]);
    }
}
