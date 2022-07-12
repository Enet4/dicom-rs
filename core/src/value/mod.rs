//! This module includes a high level abstraction over a DICOM data element's value.

use crate::header::{EmptyObject, HasLength, Length, Tag};
use num_traits::NumCast;
use smallvec::SmallVec;
use std::{borrow::Cow, str::FromStr};

pub mod deserialize;
pub mod partial;
pub mod person_name;
mod primitive;
pub mod range;
pub mod serialize;

pub use self::deserialize::Error as DeserializeError;
pub use self::partial::{DicomDate, DicomDateTime, DicomTime};
pub use self::person_name::PersonName;
pub use self::range::{AsRange, DateRange, DateTimeRange, TimeRange};

pub use self::primitive::{
    CastValueError, ConvertValueError, InvalidValueReadError, PrimitiveValue, ValueType,
};

/// re-exported from chrono
use chrono::FixedOffset;

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
pub enum Value<I = EmptyObject, P = [u8; 0]> {
    /// Primitive value.
    Primitive(PrimitiveValue),
    /// A complex sequence of items.
    Sequence {
        /// Item collection.
        items: C<I>,
        /// The size in bytes (length).
        size: Length,
    },
    /// An encapsulated pixel data sequence.
    PixelSequence {
        /// The value contents of the offset table.
        offset_table: C<u32>,
        /// The sequence of compressed fragments.
        fragments: C<P>,
    },
}

impl<P> Value<EmptyObject, P> {
    /// Construct a DICOM pixel sequence sequence value
    /// from an offset rable and a list of fragments.
    ///
    /// Note: This function does not validate the offset table
    /// against the fragments.
    pub fn new_pixel_sequence<T>(offset_table: C<u32>, fragments: T) -> Self
    where
        T: Into<C<P>>,
    {
        Value::PixelSequence {
            offset_table,
            fragments: fragments.into(),
        }
    }
}
impl<I> Value<I, [u8; 0]> {
    /// Construct a full DICOM data set sequence value
    /// from a list of items and length.
    #[inline]
    pub fn new_sequence<T>(items: T, length: Length) -> Self
    where
        T: Into<C<I>>,
    {
        Value::Sequence {
            items: items.into(),
            size: length,
        }
    }
}

impl Value<EmptyObject, [u8; 0]> {
    /// Construct a DICOM value from a primitive value.
    ///
    /// This is equivalent to `Value::from` in behavior,
    /// except that suitable type parameters are specified
    /// instead of inferred.
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
    pub fn offset_table(&self) -> Option<&[u32]> {
        match self {
            Value::PixelSequence { offset_table, .. } => Some(offset_table),
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
    pub fn to_str(&self) -> Result<Cow<str>, CastValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_str()),
            _ => Err(CastValueError {
                requested: "string",
                got: self.value_type(),
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
    pub fn to_raw_str(&self) -> Result<Cow<str>, CastValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_raw_str()),
            _ => Err(CastValueError {
                requested: "string",
                got: self.value_type(),
            }),
        }
    }

    /// Convert the full primitive value into a clean string.
    ///
    /// Returns an error if the value is not primitive.
    #[deprecated(
        note = "`to_clean_str()` is now deprecated in favour of using `to_str()` directly. 
        `to_raw_str()` replaces the old functionality of `to_str()` and maintains all trailing whitespace."
    )]
    pub fn to_clean_str(&self) -> Result<Cow<str>, CastValueError> {
        self.to_str()
    }

    /// Convert the full primitive value into a sequence of strings.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of strings as described in [`PrimitiveValue::to_multi_str`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_multi_str`]: ../enum.PrimitiveValue.html#to_multi_str
    pub fn to_multi_str(&self) -> Result<Cow<[String]>, CastValueError> {
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
    pub fn to_bytes(&self) -> Result<Cow<[u8]>, CastValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_bytes()),
            _ => Err(CastValueError {
                requested: "bytes",
                got: self.value_type(),
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
    pub fn to_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<DicomDateTime, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_datetime(default_offset),
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
    pub fn to_multi_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<Vec<DicomDateTime>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_datetime(default_offset),
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
    pub fn to_datetime_range(
        &self,
        offset: FixedOffset,
    ) -> Result<DateTimeRange, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_datetime_range(offset),
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

    /// Retrieves the primitive value as a PersonName.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dicom_value;
    use crate::header::EmptyObject;
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
                original: ValueType::Item,
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
                original: ValueType::Item,
                ..
            })
        ));
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
}
