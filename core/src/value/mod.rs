//! This module includes a high level abstraction over a DICOM data element's value.

use crate::header::{EmptyObject, HasLength, Length, Tag};
use num_traits::NumCast;
use smallvec::SmallVec;
use std::{borrow::Cow, str::FromStr};

pub mod deserialize;
mod primitive;
pub mod serialize;

pub use self::deserialize::Error as DeserializeError;
pub use self::deserialize::{DateRange, DateTimeRange, TimeRange};
pub use self::primitive::{
    CastValueError, ConvertValueError, InvalidValueReadError, PrimitiveValue, ValueType,
};

/// re-exported from chrono
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};

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
        offset_table: C<u8>,
        /// The sequence of compressed fragments.
        fragments: C<P>,
    },
}

impl<P> Value<EmptyObject, P> {
    /// Construct a DICOM pixel sequence sequence value
    /// from an offset table and a list of fragments.
    ///
    /// Note: This function does not validate the offset table
    /// against the fragments.
    pub fn new_pixel_sequence<T>(offset_table: C<u8>, fragments: T) -> Self
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

    /// Convert the full primitive value into a clean string.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_clean_str(&self) -> Result<Cow<str>, CastValueError> {
        match self {
            Value::Primitive(prim) => Ok(prim.to_clean_str()),
            _ => Err(CastValueError {
                requested: "string",
                got: self.value_type(),
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

    /// Retrieves the primitive value as a sequence of unsigned bytes
    /// without conversions.
    #[deprecated(note = "use `uint8_slice` instead")]
    pub fn as_u8(&self) -> Result<&[u8], CastValueError> {
        self.uint8_slice()
    }

    /// Retrieves the primitive value as a sequence of signed 32-bit integers
    /// without conversions.
    #[deprecated(note = "use `int32_slice` instead")]
    pub fn as_i32(&self) -> Result<&[i32], CastValueError> {
        self.int32_slice()
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

    /// Retrieve and convert the primitive value into a date.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `NaiveDate` as described in [`PrimitiveValue::to_date`].
    ///
    /// [`PrimitiveValue::to_date`]: ../enum.PrimitiveValue.html#to_date
    pub fn to_date(&self) -> Result<NaiveDate, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_date(),
            _ => Err(ConvertValueError {
                requested: "Date",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a sequence of dates.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `NaiveDate` as described in [`PrimitiveValue::to_multi_date`].
    ///
    /// [`PrimitiveValue::to_multi_date`]: ../enum.PrimitiveValue.html#to_multi_date
    pub fn to_multi_date(&self) -> Result<Vec<NaiveDate>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_date(),
            _ => Err(ConvertValueError {
                requested: "Date",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a time.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `NaiveTime` as described in [`PrimitiveValue::to_time`].
    ///
    /// [`PrimitiveValue::to_time`]: ../enum.PrimitiveValue.html#to_time
    pub fn to_time(&self) -> Result<NaiveTime, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_time(),
            _ => Err(ConvertValueError {
                requested: "Time",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a sequence of times.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `NaiveTime` as described in [`PrimitiveValue::to_multi_time`].
    ///
    /// [`PrimitiveValue::to_multi_time`]: ../enum.PrimitiveValue.html#to_multi_time
    pub fn to_multi_time(&self) -> Result<Vec<NaiveTime>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_time(),
            _ => Err(ConvertValueError {
                requested: "Time",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a date-time.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DateTime` as described in [`PrimitiveValue::to_datetime`].
    ///
    /// [`PrimitiveValue::to_datetime`]: ../enum.PrimitiveValue.html#to_datetime
    pub fn to_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<DateTime<FixedOffset>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_datetime(default_offset),
            _ => Err(ConvertValueError {
                requested: "DateTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a sequence of date-times.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `DateTime` as described in [`PrimitiveValue::to_multi_datetime`].
    ///
    /// [`PrimitiveValue::to_multi_datetime`]: ../enum.PrimitiveValue.html#to_multi_datetime
    pub fn to_multi_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<Vec<DateTime<FixedOffset>>, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_multi_datetime(default_offset),
            _ => Err(ConvertValueError {
                requested: "DateTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a date range.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DateRange`
    pub fn to_date_range(&self) -> Result<DateRange, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_date_range(),
            _ => Err(ConvertValueError {
                requested: "Date range",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a time range.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `TimeRange`
    pub fn to_time_range(&self) -> Result<TimeRange, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_time_range(),
            _ => Err(ConvertValueError {
                requested: "Time range",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve and convert the primitive value into a date-time range.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DateTimeRange`
    pub fn to_datetime_range(
        &self,
        default_offset: FixedOffset,
    ) -> Result<DateTimeRange, ConvertValueError> {
        match self {
            Value::Primitive(v) => v.to_datetime_range(default_offset),
            _ => Err(ConvertValueError {
                requested: "Date-time range",
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

    /// Retrieves the primitive value as a sequence of DICOM tags.
    #[deprecated(note = "use `tags` instead")]
    pub fn as_tags(&self) -> Result<&[Tag], CastValueError> {
        self.tags()
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
    impl_primitive_getters!(date, dates, Date, NaiveDate);
    impl_primitive_getters!(time, times, Time, NaiveTime);
    impl_primitive_getters!(datetime, datetimes, DateTime, DateTime<FixedOffset>);
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
    use chrono::TimeZone;

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
            Value::new(PrimitiveValue::Date(smallvec![NaiveDate::from_ymd(
                2014, 10, 12
            )]))
            .date()
            .unwrap(),
            NaiveDate::from_ymd(2014, 10, 12),
        );

        assert_eq!(
            Value::new(PrimitiveValue::Date(
                smallvec![NaiveDate::from_ymd(2014, 10, 12); 5]
            ))
            .dates()
            .unwrap(),
            &[NaiveDate::from_ymd(2014, 10, 12); 5]
        );

        assert!(matches!(
            Value::new(PrimitiveValue::Date(smallvec![NaiveDate::from_ymd(
                2014, 10, 12
            )]))
            .time(),
            Err(CastValueError {
                requested: "time",
                got: ValueType::Date,
                ..
            })
        ));
    }

    #[test]
    fn to_date_range() {
        // exactly two dates get orderer
        assert_eq!(
            Value::new(PrimitiveValue::Date(smallvec![
                NaiveDate::from_ymd(2015, 10, 12),
                NaiveDate::from_ymd(2014, 10, 12)
            ]))
            .to_date_range()
            .unwrap(),
            (
                Some(NaiveDate::from_ymd(2014, 10, 12)),
                Some(NaiveDate::from_ymd(2015, 10, 12))
            )
        );
        // exactly two strs get orderer
        assert_eq!(
            Value::new(PrimitiveValue::Strs(smallvec![
                String::from("20141012"),
                String::from("20131012")
            ]))
            .to_date_range()
            .unwrap(),
            (
                Some(NaiveDate::from_ymd(2013, 10, 12),),
                Some(NaiveDate::from_ymd(2014, 10, 12),)
            )
        );
        // valid range from bytes
        assert_eq!(
            Value::new(PrimitiveValue::from("20131012-20141012".as_bytes()))
                .to_date_range()
                .unwrap(),
            (
                Some(NaiveDate::from_ymd(2013, 10, 12),),
                Some(NaiveDate::from_ymd(2014, 10, 12),)
            )
        );

        // other than exactly two Dates fails
        assert!(matches!(
            Value::new(PrimitiveValue::Date(smallvec![
                NaiveDate::from_ymd(2014, 10, 1),
                NaiveDate::from_ymd(2014, 10, 2),
                NaiveDate::from_ymd(2014, 10, 3)
            ]))
            .to_date_range(),
            Err(ConvertValueError {
                requested: "Date range",
                original: ValueType::Date,
                cause: Some(InvalidValueReadError::TwoValuesForRange { len: 3 }),
            })
        ));

        // other than exactly two Str fails
        assert!(matches!(
            Value::new(PrimitiveValue::Strs(smallvec![String::from("20141012")])).to_date_range(),
            Err(ConvertValueError {
                requested: "Date range",
                original: ValueType::Strs,
                cause: Some(InvalidValueReadError::TwoValuesForRange { len: 1 }),
            })
        ));

        // not a date range
        assert!(matches!(
            Value::new(PrimitiveValue::Str("Smith^John".to_string())).to_date_range(),
            Err(ConvertValueError {
                requested: "Date range",
                original: ValueType::Str,
                cause: Some(_),
            })
        ));
    }

    #[test]
    fn to_time_range() {
        // exactly two times get orderer
        assert_eq!(
            Value::new(PrimitiveValue::Time(smallvec![
                NaiveTime::from_hms(16, 05, 05),
                NaiveTime::from_hms(15, 05, 05)
            ]))
            .to_time_range()
            .unwrap(),
            (
                Some(NaiveTime::from_hms(15, 05, 05)),
                Some(NaiveTime::from_hms(16, 05, 05))
            )
        );

        // exactly two strs get orderer
        assert_eq!(
            Value::new(PrimitiveValue::Strs(smallvec![String::from("160505"), String::from("150505")]))
                .to_time_range()
                .unwrap(),
            (
                Some(NaiveTime::from_hms(15, 05, 05)),
                Some(NaiveTime::from_hms(16, 05, 05))
            )
        );

        // valid range from bytes
        assert_eq!(
            Value::new(PrimitiveValue::from("150505-160505".as_bytes()))
                .to_time_range()
                .unwrap(),
            (
                Some(NaiveTime::from_hms(15, 05, 05)),
                Some(NaiveTime::from_hms(16, 05, 05))
            )
        );

        // other than exactly two times fails
        assert!(matches!(
            Value::new(PrimitiveValue::Time(smallvec![
                NaiveTime::from_hms(15, 05, 05),
                NaiveTime::from_hms(16, 05, 05),
                NaiveTime::from_hms(17, 05, 05)
            ]))
            .to_time_range(),
            Err(ConvertValueError {
                requested: "Time range",
                original: ValueType::Time,
                cause: Some(InvalidValueReadError::TwoValuesForRange { len: 3 }),
            })
        ));

        // other than exactly two Str fails
        assert!(matches!(
            Value::new(PrimitiveValue::Strs(smallvec![String::from("150505")])).to_time_range(),
            Err(ConvertValueError {
                requested: "Time range",
                original: ValueType::Strs,
                cause: Some(InvalidValueReadError::TwoValuesForRange { len: 1 }),
            })
        ));

        // not a time range
        assert!(matches!(
            Value::new(PrimitiveValue::Str("Smith^John".to_string())).to_time_range(),
            Err(ConvertValueError {
                requested: "Time range",
                original: ValueType::Str,
                cause: Some(_),
            })
        ));
    }

    #[test]
    fn to_datetime_range() {
        let offset = FixedOffset::east(0);

        // exactly two date-times get orderer
        assert_eq!(
            Value::new(PrimitiveValue::DateTime(smallvec![
                FixedOffset::west(3660)
                    .ymd(1980, 1, 1)
                    .and_hms_micro(15, 24, 30, 123456),
                FixedOffset::west(3660)
                    .ymd(1970, 1, 1)
                    .and_hms_micro(15, 24, 30, 123456)
            ]))
            .to_datetime_range(offset)
            .unwrap(),
            (
                Some(
                    FixedOffset::west(3660)
                        .ymd(1970, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                ),
                Some(
                    FixedOffset::west(3660)
                        .ymd(1980, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                )
            )
        );

        // exactly two strs get orderer
        assert_eq!(
            Value::new(PrimitiveValue::Strs(smallvec![
                String::from("19800101152430.123456-0101"),
                String::from("19700101152430.123456-0101")
            ]))
            .to_datetime_range(offset)
            .unwrap(),
            (
                Some(
                    FixedOffset::west(3660)
                        .ymd(1970, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                ),
                Some(
                    FixedOffset::west(3660)
                        .ymd(1980, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                )
            )
        );

        // valid range from bytes
        assert_eq!(
            Value::new(PrimitiveValue::from(
                "19700101152430.123456-0101-19800101152430.123456-0101".as_bytes()
            ))
            .to_datetime_range(offset)
            .unwrap(),
            (
                Some(
                    FixedOffset::west(3660)
                        .ymd(1970, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                ),
                Some(
                    FixedOffset::west(3660)
                        .ymd(1980, 1, 1)
                        .and_hms_micro(15, 24, 30, 123456)
                )
            )
        );

        // other than exactly two date-times fails
        assert!(matches!(
            Value::new(PrimitiveValue::DateTime(smallvec![
                FixedOffset::west(3660)
                    .ymd(1970, 1, 1)
                    .and_hms_micro(15, 24, 30, 123456),
                FixedOffset::west(3660)
                    .ymd(1980, 1, 1)
                    .and_hms_micro(15, 24, 30, 123456),
                FixedOffset::west(3660)
                    .ymd(1990, 1, 1)
                    .and_hms_micro(15, 24, 30, 123456)
            ]))
            .to_datetime_range(offset),
            Err(ConvertValueError {
                requested: "Date-time range",
                original: ValueType::DateTime,
                cause: Some(InvalidValueReadError::TwoValuesForRange { len: 3 }),
            })
        ));

        // other than exactly two Str fails
        assert!(matches!(
            Value::new(PrimitiveValue::Strs(smallvec![String::from("19700101152430.123456-0101")]))
                .to_datetime_range(offset),
            Err(ConvertValueError {
                requested: "Date-time range",
                original: ValueType::Strs,
                cause: Some(InvalidValueReadError::TwoValuesForRange { len: 1 }),
            })
        ));

        // not a date-time range
        assert!(matches!(
            Value::new(PrimitiveValue::Str("Smith^John".to_string())).to_datetime_range(offset),
            Err(ConvertValueError {
                requested: "Date-time range",
                original: ValueType::Str,
                cause: Some(_),
            })
        ));
    }

}
