//! Declaration and implementation of a DICOM primitive value.
//!
//! See [`PrimitiveValue`](./enum.PrimitiveValue.html).

use super::DicomValueType;
use crate::header::{HasLength, Length, Tag};
use chrono::{FixedOffset, Timelike};
use itertools::Itertools;
use num_traits::NumCast;
use safe_transmute::to_bytes::transmute_to_bytes;
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

/** Triggered when a value reading attempt fails.
 */
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum InvalidValueReadError {
    /// Attempted to retrieve a complex value as primitive.
    #[snafu(display("Sequence cannot be read as a primitive value"))]
    NonPrimitiveType { backtrace: Backtrace },
    /// Invalid or ambiguous combination of date with time.
    #[snafu(display("Invalid or ambiguous combination of date with time"))]
    DateTimeZone { backtrace: Backtrace },
    /// The value cannot be parsed to a floating point number.
    #[snafu(display("Failed to read text as a floating point number"))]
    ParseFloat {
        backtrace: Backtrace,
        source: std::num::ParseFloatError,
    },
    /// The value cannot be parsed to an integer.
    #[snafu(display("Failed to read text as an integer"))]
    ParseInteger {
        backtrace: Backtrace,
        source: std::num::ParseIntError,
    },
    /// An attempt of reading more than the number of bytes in the length attribute was made.
    #[snafu(display("Unexpected end of element"))]
    UnexpectedEndOfElement {},
    /// The value cannot be converted to the target type requested.
    #[snafu(display("Cannot convert `{}` to the target type requested", value))]
    NarrowConvert { value: String, backtrace: Backtrace },
    #[snafu(display("Failed to read text as a date"))]
    ParseDate {
        #[snafu(backtrace)]
        source: crate::value::deserialize::Error,
    },
    #[snafu(display("Failed to read text as a time"))]
    ParseTime {
        #[snafu(backtrace)]
        source: crate::value::deserialize::Error,
    },
    #[snafu(display("Failed to read text as a date-time"))]
    ParseDateTime {
        #[snafu(backtrace)]
        source: crate::value::deserialize::Error,
    },
}

/// An error type for an attempt of accessing a value
/// in one internal representation as another.
///
/// This error is raised whenever it is not possible to retrieve the requested
/// value, either because the inner representation is not compatible with the
/// requested value type, or a conversion would be required. In other words,
/// if a reference to the inner value cannot be obtained with
/// the requested target type (for example, retrieving a date from a string),
/// an error of this type is returned.
///
/// If such a conversion is acceptable, please use conversion methods instead:
/// `to_date` instead of `date`, `to_str` instead of `string`, and so on.
/// The error type would then be [`ConvertValueError`].
///
/// [`ConvertValueError`]: ./struct.ConvertValueError.html
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

impl std::error::Error for CastValueError {}

/// An error type for a failed attempt at converting a value
/// into another representation.
#[derive(Debug)]
pub struct ConvertValueError {
    /// The value format requested
    pub requested: &'static str,
    /// The value's original representation
    pub original: ValueType,
    /// The reason why the conversion was unsuccessful,
    /// or none if a conversion from the given original representation
    /// is not possible
    pub cause: Option<InvalidValueReadError>,
}

impl fmt::Display for ConvertValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "could not convert {:?} to a {}: ",
            self.original, self.requested
        )?;
        if let Some(cause) = &self.cause {
            write!(f, "{}", cause)?;
        } else {
            write!(f, "conversion not possible")?;
        }
        Ok(())
    }
}

impl std::error::Error for ConvertValueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_ref().map(|x| x as _)
    }
}

pub type Result<T, E = InvalidValueReadError> = std::result::Result<T, E>;

// Re-exported from chrono
pub use chrono::{DateTime, NaiveDate, NaiveTime};

/// An aggregation of one or more elements in a value.
pub type C<T> = SmallVec<[T; 2]>;

/// An enum representing a primitive value from a DICOM element.
/// The result of decoding an element's data value
/// may be one of the enumerated types
/// depending on its content and value representation.
///
/// Multiple elements are contained in a [`smallvec`] vector,
/// conveniently aliased to the type [`C`].
///
/// See the macro [`dicom_value!`] for a more intuitive means
/// of constructing these values.
/// Alternatively, `From` conversions into `PrimitiveValue` exist
/// for single element types,
/// including numeric types, `String`, and `&str`.
///
/// # Example
///
/// ```
/// # use dicom_core::PrimitiveValue;
/// # use smallvec::smallvec;
/// let value = PrimitiveValue::from("Smith^John");
/// assert_eq!(value, PrimitiveValue::Str("Smith^John".to_string()));
/// assert_eq!(value.multiplicity(), 1);
///
/// let value = PrimitiveValue::from(512_u16);
/// assert_eq!(value, PrimitiveValue::U16(smallvec![512]));
/// ```
///
/// [`smallvec`]: ../../smallvec/index.html
/// [`C`]: ./type.C.html
/// [`dicom_value!`]: ../macro.dicom_value.html
#[derive(Debug, PartialEq, Clone)]
pub enum PrimitiveValue {
    /// No data. Usually employed for zero-lengthed values.
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

    /// A sequence of complete dates.
    /// Used for the DA representation.
    Date(C<NaiveDate>),

    /// A sequence of complete date-time values.
    /// Used for the DT representation.
    DateTime(C<DateTime<FixedOffset>>),

    /// A sequence of complete time values.
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

impl From<Vec<u8>> for PrimitiveValue {
    fn from(value: Vec<u8>) -> Self {
        PrimitiveValue::U8(C::from(value))
    }
}

impl From<&[u8]> for PrimitiveValue {
    fn from(value: &[u8]) -> Self {
        PrimitiveValue::U8(C::from(value))
    }
}

macro_rules! impl_from_array_for_primitive {
    ($typ: ty, $variant: ident) => {
        impl From<$typ> for PrimitiveValue {
            fn from(value: $typ) -> Self {
                PrimitiveValue::$variant(C::from_slice(&value[..]))
            }
        }
    };
}

macro_rules! impl_from_array_for_primitive_1_to_8 {
    ($typ: ty, $variant: ident) => {
        impl_from_array_for_primitive!([$typ; 1], $variant);
        impl_from_array_for_primitive!([$typ; 2], $variant);
        impl_from_array_for_primitive!([$typ; 3], $variant);
        impl_from_array_for_primitive!([$typ; 4], $variant);
        impl_from_array_for_primitive!([$typ; 5], $variant);
        impl_from_array_for_primitive!([$typ; 6], $variant);
        impl_from_array_for_primitive!([$typ; 7], $variant);
        impl_from_array_for_primitive!([$typ; 8], $variant);
        impl_from_array_for_primitive!(&[$typ; 1], $variant);
        impl_from_array_for_primitive!(&[$typ; 2], $variant);
        impl_from_array_for_primitive!(&[$typ; 3], $variant);
        impl_from_array_for_primitive!(&[$typ; 4], $variant);
        impl_from_array_for_primitive!(&[$typ; 5], $variant);
        impl_from_array_for_primitive!(&[$typ; 6], $variant);
        impl_from_array_for_primitive!(&[$typ; 7], $variant);
        impl_from_array_for_primitive!(&[$typ; 8], $variant);
    };
}

impl_from_array_for_primitive_1_to_8!(u8, U8);
impl_from_array_for_primitive_1_to_8!(u16, U16);
impl_from_array_for_primitive_1_to_8!(i16, I16);
impl_from_array_for_primitive_1_to_8!(u32, U32);
impl_from_array_for_primitive_1_to_8!(i32, I32);
impl_from_array_for_primitive_1_to_8!(u64, U64);
impl_from_array_for_primitive_1_to_8!(i64, I64);
impl_from_array_for_primitive_1_to_8!(f32, F32);
impl_from_array_for_primitive_1_to_8!(f64, F64);
impl_from_array_for_primitive_1_to_8!(NaiveDate, Date);
impl_from_array_for_primitive_1_to_8!(NaiveTime, Time);
impl_from_array_for_primitive_1_to_8!(DateTime<FixedOffset>, DateTime);

impl PrimitiveValue {
    /// Create a single unsigned 16-bit value.
    pub fn new_u16(value: u16) -> Self {
        PrimitiveValue::U16(C::from_elem(value, 1))
    }

    /// Create a single unsigned 32-bit value.
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

    /// Determine the length of the DICOM value in its encoded form.
    ///
    /// In other words,
    /// this is the number of bytes that the value
    /// would need to occupy in a DICOM file,
    /// without compression and without the element header.
    /// The output is always an even number,
    /// so as to consider the mandatory trailing padding.
    ///
    /// This method is particularly useful for presenting an estimated
    /// space occupation to the end user.
    /// However, consumers should not depend on this number for
    /// decoding or encoding values.
    /// The calculated number does not need to match
    /// the length of the original byte stream
    /// from where the value was originally decoded.
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
            Date(c) => (c.len() * 9) & !1,
            Str(s) => s.as_bytes().len(),
            Strs(c) => c.iter().map(|s| s.as_bytes().len() + 1).sum::<usize>() & !1,
            Time(c) => {
                c.iter()
                    .map(|t| PrimitiveValue::tm_byte_len(*t) + 1)
                    .sum::<usize>()
                    & !1
            }
            DateTime(c) => {
                c.iter()
                    .map(|dt| PrimitiveValue::dt_byte_len(*dt) + 1)
                    .sum::<usize>()
                    & !1
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
        // Note: this implementation assumes that the date is always
        // written fully.
        // It currently cannot distinguish unspecified components from their defaults
        // (e.g. "201812" should be different from "20181201").
        // This will have to be changed at some point (issue #55)
        8 + PrimitiveValue::tm_byte_len(datetime.time())
            + if datetime.offset() == &FixedOffset::east(0) {
                0
            } else {
                5
            }
    }

    /// Convert the primitive value into a string representation.
    ///
    /// String values already encoded with the `Str` and `Strs` variants
    /// are provided as is.
    /// In the case of `Strs`, the strings are first joined together
    /// with a backslash (`'\\'`).
    /// All other type variants are first converted to a string,
    /// then joined together with a backslash.
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that `to_str()` returns a string with
    /// the exact same byte sequence as the one originally found
    /// at the source of the value,
    /// even for the string variants.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John").to_str(),
    ///     "Smith^John",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Date, NaiveDate::from_ymd(2014, 10, 12)).to_str(),
    ///     "20141012",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Strs, [
    ///         "DERIVED",
    ///         "PRIMARY",
    ///         "WHOLE BODY",
    ///         "EMISSION",
    ///     ])
    ///     .to_str(),
    ///     "DERIVED\\PRIMARY\\WHOLE BODY\\EMISSION",
    /// );
    /// ```
    pub fn to_str(&self) -> Cow<str> {
        match self {
            PrimitiveValue::Empty => Cow::from(""),
            PrimitiveValue::Str(values) => Cow::from(values.as_str()),
            PrimitiveValue::Strs(values) => {
                if values.len() == 1 {
                    Cow::from(&values[0])
                } else {
                    Cow::from(values.iter().join("\\"))
                }
            }
            prim => Cow::from(prim.to_string()),
        }
    }

    /// Convert the primitive value into a multi-string representation.
    ///
    /// String values already encoded with the `Str` and `Strs` variants
    /// are provided as is.
    /// All other type variants are first converted to a string,
    /// then collected into a vector.
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that `to_multi_str()` returns strings with
    /// the exact same byte sequence as the one originally found
    /// at the source of the value,
    /// even for the string variants.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    /// assert_eq!(
    ///     dicom_value!(Strs, [
    ///         "DERIVED",
    ///         "PRIMARY",
    ///         "WHOLE BODY",
    ///         "EMISSION",
    ///     ])
    ///     .to_multi_str(),
    ///     &["DERIVED", "PRIMARY", "WHOLE BODY", "EMISSION"][..],
    /// );
    ///
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John").to_multi_str(),
    ///     &["Smith^John"][..],
    /// );
    ///
    /// assert_eq!(
    ///     dicom_value!(Date, NaiveDate::from_ymd(2014, 10, 12)).to_multi_str(),
    ///     &["20141012"][..],
    /// );
    ///
    /// assert_eq!(
    ///     dicom_value!(I64, [128, 256, 512]).to_multi_str(),
    ///     &["128", "256", "512"][..],
    /// );
    /// ```
    pub fn to_multi_str(&self) -> Cow<[String]> {
        /// Auxilliary function for turning a sequence of values
        /// into a sequence of strings.
        fn seq_to_str<I>(iter: I) -> Vec<String>
        where
            I: IntoIterator,
            I::Item: std::fmt::Display,
        {
            iter.into_iter().map(|x| x.to_string()).collect()
        }

        match self {
            PrimitiveValue::Empty => Cow::from(&[][..]),
            PrimitiveValue::Str(values) => Cow::from(std::slice::from_ref(values)),
            PrimitiveValue::Strs(values) => Cow::from(&values[..]),
            PrimitiveValue::Date(values) => values
                .into_iter()
                .map(|date| date.format("%Y%m%d").to_string())
                .collect::<Vec<_>>()
                .into(),
            PrimitiveValue::Time(values) => values
                .into_iter()
                .map(|date| date.format("%H%M%S%.6f").to_string())
                .collect::<Vec<_>>()
                .into(),
            PrimitiveValue::DateTime(values) => values
                .into_iter()
                .map(|date| date.format("%Y%m%d%H%M%S%.6f%z").to_string())
                .collect::<Vec<_>>()
                .into(),
            PrimitiveValue::U8(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::U16(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::U32(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::I16(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::I32(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::U64(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::I64(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::F32(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::F64(values) => Cow::Owned(seq_to_str(values)),
            PrimitiveValue::Tags(values) => Cow::Owned(seq_to_str(values)),
        }
    }

    /// Convert the primitive value into a clean string representation,
    /// removing unwanted whitespaces.
    ///
    /// Leading whitespaces are preserved and are only removed at the end of a string
    ///
    /// String values already encoded with the `Str` and `Strs` variants
    /// are provided as is without the unwanted whitespaces.
    /// In the case of `Strs`, the strings are first cleaned from whitespaces
    /// and then joined together with a backslash (`'\\'`).
    /// All other type variants are first converted to a clean string,
    /// then joined together with a backslash.
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that `to_clean_str()` returns a string with
    /// the exact same byte sequence as the one originally found
    /// at the source of the value,
    /// even for the string variants.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John ").to_clean_str(),
    ///     "Smith^John",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Str, " Smith^John").to_clean_str(),
    ///     " Smith^John",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Date, NaiveDate::from_ymd(2014, 10, 12)).to_clean_str(),
    ///     "20141012",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Strs, [
    ///         "DERIVED\0",
    ///         "PRIMARY",
    ///         " WHOLE BODY",
    ///         "EMISSION",
    ///     ])
    ///     .to_clean_str(),
    ///     "DERIVED\\PRIMARY\\ WHOLE BODY\\EMISSION",
    /// );
    /// ```
    pub fn to_clean_str(&self) -> Cow<str> {
        match self {
            PrimitiveValue::Str(values) => {
                Cow::from(values.trim_end_matches(|c| c == ' ' || c == '\u{0}'))
            }
            PrimitiveValue::Strs(values) => {
                if values.len() == 1 {
                    Cow::from(values[0].trim_end_matches(|c| c == ' ' || c == '\u{0}'))
                } else {
                    Cow::Owned(
                        values
                            .iter()
                            .map(|s| s.trim_end_matches(|c| c == ' ' || c == '\u{0}'))
                            .join("\\"),
                    )
                }
            }
            prim => Cow::from(prim.to_string()),
        }
    }

    /// Retrieve this DICOM value as raw bytes.
    ///
    /// Binary numeric values are returned with a reintepretation
    /// of the holding vector's occupied data block as bytes,
    /// without copying,
    /// under the platform's native byte order.
    ///
    /// String values already encoded with the `Str` and `Strs` variants
    /// are provided as their respective bytes in UTF-8.
    /// In the case of `Strs`, the strings are first joined together
    /// with a backslash (`'\\'`).
    /// Other type variants are first converted to a string,
    /// joined together with a backslash,
    /// then turned into a byte vector.
    /// For values which are inherently textual according the standard,
    /// this is equivalent to calling `as_bytes()` after [`to_str()`].
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that `to_bytes()` returns the same byte sequence
    /// as the one originally found at the source of the value.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// [`to_str()`]: #method.to_str
    ///
    /// # Examples
    ///
    /// `U8` provides a straight, zero-copy slice of bytes.
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::U8(smallvec![
    ///         1, 2, 5,
    ///     ]).to_bytes(),
    ///     &[1, 2, 5][..],
    /// );
    /// ```
    ///
    /// Other values are converted to text first.
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    /// assert_eq!(
    ///     PrimitiveValue::from("Smith^John").to_bytes(),
    ///     &b"Smith^John"[..],
    /// );
    /// assert_eq!(
    ///     PrimitiveValue::from(NaiveDate::from_ymd(2014, 10, 12))
    ///     .to_bytes(),
    ///     &b"20141012"[..],
    /// );
    /// assert_eq!(
    ///     dicom_value!(Strs, [
    ///         "DERIVED",
    ///         "PRIMARY",
    ///         "WHOLE BODY",
    ///         "EMISSION",
    ///     ])
    ///     .to_bytes(),
    ///     &b"DERIVED\\PRIMARY\\WHOLE BODY\\EMISSION"[..],
    /// );
    /// ```
    pub fn to_bytes(&self) -> Cow<[u8]> {
        match self {
            PrimitiveValue::Empty => Cow::from(&[][..]),
            PrimitiveValue::U8(values) => Cow::from(&values[..]),
            PrimitiveValue::U16(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::I16(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::U32(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::I32(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::I64(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::U64(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::F32(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::F64(values) => Cow::Borrowed(transmute_to_bytes(values)),
            PrimitiveValue::Str(values) => Cow::from(values.as_bytes()),
            PrimitiveValue::Strs(values) => {
                if values.len() == 1 {
                    // no need to copy if it's a single string
                    Cow::from(values[0].as_bytes())
                } else {
                    Cow::from(values.iter().join("\\").into_bytes())
                }
            }
            prim => match prim.to_str() {
                Cow::Borrowed(string) => Cow::Borrowed(string.as_bytes()),
                Cow::Owned(string) => Cow::Owned(string.into_bytes()),
            },
        }
    }

    /// Retrieve a single integer of type `T` from this value.
    ///
    /// If the value is already represented as an integer,
    /// it is returned after a conversion to the target type.
    /// An error is returned if the integer cannot be represented
    /// by the given integer type.
    /// If the value is a string or sequence of strings,
    /// the first string is parsed to obtain an integer,
    /// potentially failing if the string does not represent a valid integer.
    /// The string is stripped of trailing whitespace before parsing,
    /// in order to account for the possible padding to even length.
    /// If the value is a sequence of U8 bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// Note that this method does not enable
    /// the conversion of floating point numbers to integers via truncation.
    /// If this is intentional,
    /// retrieve a float via [`to_float32`] or [`to_float64`] instead,
    /// then cast it to an integer.
    ///
    /// [`to_float32`]: #method.to_float32
    /// [`to_float64`]: #method.to_float64
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::I32(smallvec![
    ///         1, 2, 5,
    ///     ])
    ///     .to_int::<u32>().ok(),
    ///     Some(1_u32),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("505 ").to_int::<i32>().ok(),
    ///     Some(505),
    /// );
    /// ```
    pub fn to_int<T>(&self) -> Result<T, ConvertValueError>
    where
        T: NumCast,
        T: FromStr<Err = std::num::ParseIntError>,
    {
        match self {
            PrimitiveValue::Str(s) => {
                s.trim_end()
                    .parse()
                    .context(ParseInteger)
                    .map_err(|err| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::Strs(s) if !s.is_empty() => s[0]
                .trim_end()
                .parse()
                .context(ParseInteger)
                .map_err(|err| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::U8(bytes) if !bytes.is_empty() => {
                T::from(bytes[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: bytes[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U16(s) if !s.is_empty() => {
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I16(s) if !s.is_empty() => {
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U32(s) if !s.is_empty() => {
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I32(s) if !s.is_empty() => {
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U64(s) if !s.is_empty() => {
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I64(s) if !s.is_empty() => {
                T::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "integer",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            _ => Err(ConvertValueError {
                requested: "integer",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a sequence of integers of type `T` from this value.
    ///
    /// If the values is already represented as an integer,
    /// it is returned after a [`NumCast`] conversion to the target type.
    /// An error is returned if any of the integers cannot be represented
    /// by the given integer type.
    /// If the value is a string or sequence of strings,
    /// each string is parsed to obtain an integer,
    /// potentially failing if the string does not represent a valid integer.
    /// The string is stripped of trailing whitespace before parsing,
    /// in order to account for the possible padding to even length.
    /// If the value is a sequence of U8 bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// Note that this method does not enable
    /// the conversion of floating point numbers to integers via truncation.
    /// If this is intentional,
    /// retrieve a float via [`to_float32`] or [`to_float64`] instead,
    /// then cast it to an integer.
    ///
    /// [`NumCast`]: ../num_traits/cast/trait.NumCast.html
    /// [`to_float32`]: #method.to_float32
    /// [`to_float64`]: #method.to_float64
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use dicom_core::dicom_value;
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::I32(smallvec![
    ///         1, 2, 5,
    ///     ])
    ///     .to_multi_int::<u32>().ok(),
    ///     Some(vec![1_u32, 2, 5]),
    /// );
    ///
    /// assert_eq!(
    ///     dicom_value!(Strs, ["5050", "23 "]).to_multi_int::<i32>().ok(),
    ///     Some(vec![5050, 23]),
    /// );
    /// ```
    pub fn to_multi_int<T>(&self) -> Result<Vec<T>, ConvertValueError>
    where
        T: NumCast,
        T: FromStr<Err = std::num::ParseIntError>,
    {
        match self {
            PrimitiveValue::Empty => Ok(Vec::new()),
            PrimitiveValue::Str(s) => {
                let out = s.trim_end().parse().context(ParseInteger).map_err(|err| {
                    ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(err),
                    }
                })?;
                Ok(vec![out])
            }
            PrimitiveValue::Strs(s) => {
                s.iter()
                    .map(|v| {
                        v.trim_end().parse().context(ParseInteger).map_err(|err| {
                            ConvertValueError {
                                requested: "integer",
                                original: self.value_type(),
                                cause: Some(err),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            }
            PrimitiveValue::U8(bytes) => bytes
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U16(s) => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I16(s) => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U32(s) => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I32(s) if !s.is_empty() => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U64(s) if !s.is_empty() => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I64(s) if !s.is_empty() => s
                .iter()
                .map(|v| {
                    T::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "integer",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            _ => Err(ConvertValueError {
                requested: "integer",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve one single-precision floating point from this value.
    ///
    /// If the value is already represented as a number,
    /// it is returned after a conversion to `f32`.
    /// An error is returned if the number cannot be represented
    /// by the given number type.
    /// If the value is a string or sequence of strings,
    /// the first string is parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// The string is stripped of trailing whitespace before parsing,
    /// in order to account for the possible padding to even length.
    /// If the value is a sequence of U8 bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F32(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_float32().ok(),
    ///     Some(1.5_f32),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_float32().ok(),
    ///     Some(-6.75),
    /// );
    /// ```
    pub fn to_float32(&self) -> Result<f32, ConvertValueError> {
        match self {
            PrimitiveValue::Str(s) => {
                s.trim_end()
                    .parse()
                    .context(ParseFloat)
                    .map_err(|err| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::Strs(s) if !s.is_empty() => s[0]
                .trim_end()
                .parse()
                .context(ParseFloat)
                .map_err(|err| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::U8(bytes) if !bytes.is_empty() => {
                NumCast::from(bytes[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: bytes[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::F32(s) if !s.is_empty() => Ok(s[0]),
            PrimitiveValue::F64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float32",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a sequence of single-precision floating point numbers
    /// from this value.
    ///
    /// If the value is already represented as numbers,
    /// they are returned after a conversion to `f32`.
    /// An error is returned if any of the numbers cannot be represented
    /// by an `f32`.
    /// If the value is a string or sequence of strings,
    /// the strings are parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// The string is stripped of trailing whitespace before parsing,
    /// in order to account for the possible padding to even length.
    /// If the value is a sequence of U8 bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F32(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_multi_float32().ok(),
    ///     Some(vec![1.5_f32, 2., 5.]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_multi_float32().ok(),
    ///     Some(vec![-6.75]),
    /// );
    /// ```
    pub fn to_multi_float32(&self) -> Result<Vec<f32>, ConvertValueError> {
        match self {
            PrimitiveValue::Empty => Ok(Vec::new()),
            PrimitiveValue::Str(s) => {
                let out =
                    s.trim_end()
                        .parse()
                        .context(ParseFloat)
                        .map_err(|err| ConvertValueError {
                            requested: "float32",
                            original: self.value_type(),
                            cause: Some(err),
                        })?;
                Ok(vec![out])
            }
            PrimitiveValue::Strs(s) => s
                .iter()
                .map(|v| {
                    v.trim_end()
                        .parse()
                        .context(ParseFloat)
                        .map_err(|err| ConvertValueError {
                            requested: "float32",
                            original: self.value_type(),
                            cause: Some(err),
                        })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U8(bytes) => bytes
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::F32(s) => Ok(s[..].to_owned()),
            PrimitiveValue::F64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float32",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve one double-precision floating point from this value.
    ///
    /// If the value is already represented as a number,
    /// it is returned after a conversion to `f64`.
    /// An error is returned if the number cannot be represented
    /// by the given number type.
    /// If the value is a string or sequence of strings,
    /// the first string is parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// If the value is a sequence of U8 bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F64(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_float64().ok(),
    ///     Some(1.5_f64),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_float64().ok(),
    ///     Some(-6.75),
    /// );
    /// ```
    pub fn to_float64(&self) -> Result<f64, ConvertValueError> {
        match self {
            PrimitiveValue::Str(s) => {
                s.trim_end()
                    .parse()
                    .context(ParseFloat)
                    .map_err(|err| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::Strs(s) if !s.is_empty() => s[0]
                .trim_end()
                .parse()
                .context(ParseFloat)
                .map_err(|err| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::U8(bytes) if !bytes.is_empty() => {
                NumCast::from(bytes[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: bytes[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I16(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::U64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::I64(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::F32(s) if !s.is_empty() => {
                NumCast::from(s[0]).ok_or_else(|| ConvertValueError {
                    requested: "float64",
                    original: self.value_type(),
                    cause: Some(
                        NarrowConvert {
                            value: s[0].to_string(),
                        }
                        .build(),
                    ),
                })
            }
            PrimitiveValue::F64(s) if !s.is_empty() => Ok(s[0]),
            _ => Err(ConvertValueError {
                requested: "float64",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a sequence of double-precision floating point numbers
    /// from this value.
    ///
    /// If the value is already represented as numbers,
    /// they are returned after a conversion to `f64`.
    /// An error is returned if any of the numbers cannot be represented
    /// by an `f64`.
    /// If the value is a string or sequence of strings,
    /// the strings are parsed to obtain a number,
    /// potentially failing if the string does not represent a valid number.
    /// The string is stripped of trailing whitespace before parsing,
    /// in order to account for the possible padding to even length.
    /// If the value is a sequence of U8 bytes,
    /// the bytes are individually interpreted as independent numbers.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::F64(smallvec![
    ///         1.5, 2., 5.,
    ///     ])
    ///     .to_multi_float64().ok(),
    ///     Some(vec![1.5_f64, 2., 5.]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("-6.75 ").to_multi_float64().ok(),
    ///     Some(vec![-6.75]),
    /// );
    /// ```
    pub fn to_multi_float64(&self) -> Result<Vec<f64>, ConvertValueError> {
        match self {
            PrimitiveValue::Str(s) => {
                let out =
                    s.trim_end()
                        .parse()
                        .context(ParseFloat)
                        .map_err(|err| ConvertValueError {
                            requested: "float64",
                            original: self.value_type(),
                            cause: Some(err),
                        })?;
                Ok(vec![out])
            }
            PrimitiveValue::Strs(s) => s
                .iter()
                .map(|v| {
                    v.trim_end()
                        .parse()
                        .context(ParseFloat)
                        .map_err(|err| ConvertValueError {
                            requested: "float64",
                            original: self.value_type(),
                            cause: Some(err),
                        })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U8(bytes) => bytes
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I16(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::U64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::I64(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::F32(s) => s
                .iter()
                .map(|v| {
                    NumCast::from(*v).ok_or_else(|| ConvertValueError {
                        requested: "float64",
                        original: self.value_type(),
                        cause: Some(
                            NarrowConvert {
                                value: v.to_string(),
                            }
                            .build(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            PrimitiveValue::F64(s) => Ok(s[..].to_owned()),
            _ => Err(ConvertValueError {
                requested: "float32",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single DICOM date from this value.
    ///
    /// If the value is already represented as a date, it is returned as is.
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a date, potentially failing if the
    /// string does not represent a valid date.
    /// If the value is a sequence of U8 bytes, the bytes are
    /// first interpreted as an ASCII character string.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Date(smallvec![
    ///         NaiveDate::from_ymd(2014, 10, 12),
    ///     ])
    ///     .to_date().ok(),
    ///     Some(NaiveDate::from_ymd(2014, 10, 12)),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "20141012".to_string(),
    ///     ])
    ///     .to_date().ok(),
    ///     Some(NaiveDate::from_ymd(2014, 10, 12)),
    /// );
    /// ```
    pub fn to_date(&self) -> Result<NaiveDate, ConvertValueError> {
        match self {
            PrimitiveValue::Date(v) if !v.is_empty() => Ok(v[0]),
            PrimitiveValue::Str(s) => super::deserialize::parse_date(s.as_bytes())
                .map(|(date, _rest)| date)
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Date",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::Strs(s) => {
                super::deserialize::parse_date(s.first().map(|s| s.as_bytes()).unwrap_or(&[]))
                    .map(|(date, _rest)| date)
                    .context(ParseDate)
                    .map_err(|err| ConvertValueError {
                        requested: "Date",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::U8(bytes) => super::deserialize::parse_date(bytes)
                .map(|(date, _rest)| date)
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Date",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            _ => Err(ConvertValueError {
                requested: "Date",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of DICOM dates from this value.
    ///
    /// If the value is already represented as a sequence of dates,
    /// it is returned as is.
    /// If the value is a string or sequence of strings,
    /// the strings are decoded to obtain a date, potentially failing if
    /// any of the strings does not represent a valid date.
    /// If the value is a sequence of U8 bytes, the bytes are
    /// first interpreted as an ASCII character string,
    /// then as a backslash-separated list of dates.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveDate;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Date(smallvec![
    ///         NaiveDate::from_ymd(2014, 10, 12),
    ///     ]).to_multi_date().ok(),
    ///     Some(vec![NaiveDate::from_ymd(2014, 10, 12)]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "20141012".to_string(),
    ///         "20200828".to_string(),
    ///     ]).to_multi_date().ok(),
    ///     Some(vec![
    ///         NaiveDate::from_ymd(2014, 10, 12),
    ///         NaiveDate::from_ymd(2020, 8, 28),
    ///     ]),
    /// );
    /// ```
    pub fn to_multi_date(&self) -> Result<Vec<NaiveDate>, ConvertValueError> {
        match self {
            PrimitiveValue::Date(v) if !v.is_empty() => Ok(v.to_vec()),
            PrimitiveValue::Str(s) => super::deserialize::parse_date(s.as_bytes())
                .map(|(date, _rest)| vec![date])
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Date",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| super::deserialize::parse_date(s.as_bytes()).map(|(date, _rest)| date))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Date",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::U8(bytes) => bytes
                .split(|c| *c == b'\\')
                .into_iter()
                .map(|s| super::deserialize::parse_date(s).map(|(date, _rest)| date))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Date",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            _ => Err(ConvertValueError {
                requested: "Date",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single DICOM time from this value.
    ///
    /// If the value is already represented as a time, it is returned as is.
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a time, potentially failing if the
    /// string does not represent a valid time.
    /// If the value is a sequence of U8 bytes, the bytes are
    /// first interpreted as an ASCII character string.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveTime;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from(NaiveTime::from_hms(11, 2, 45)).to_time().ok(),
    ///     Some(NaiveTime::from_hms(11, 2, 45)),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("110245.78").to_time().ok(),
    ///     Some(NaiveTime::from_hms_milli(11, 2, 45, 780)),
    /// );
    /// ```
    pub fn to_time(&self) -> Result<NaiveTime, ConvertValueError> {
        match self {
            PrimitiveValue::Time(v) if !v.is_empty() => Ok(v[0]),
            PrimitiveValue::Str(s) => super::deserialize::parse_time(s.as_bytes())
                .map(|(date, _rest)| date)
                .context(ParseTime)
                .map_err(|err| ConvertValueError {
                    requested: "Time",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::Strs(s) => {
                super::deserialize::parse_time(s.first().map(|s| s.as_bytes()).unwrap_or(&[]))
                    .map(|(date, _rest)| date)
                    .context(ParseTime)
                    .map_err(|err| ConvertValueError {
                        requested: "Time",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::U8(bytes) => super::deserialize::parse_time(bytes)
                .map(|(date, _rest)| date)
                .context(ParseTime)
                .map_err(|err| ConvertValueError {
                    requested: "Time",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            _ => Err(ConvertValueError {
                requested: "Time",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of DICOM times from this value.
    ///
    /// If the value is already represented as a sequence of times,
    /// it is returned as is.
    /// If the value is a string or sequence of strings,
    /// the strings are decoded to obtain a date, potentially failing if
    /// any of the strings does not represent a valid date.
    /// If the value is a sequence of U8 bytes, the bytes are
    /// first interpreted as an ASCII character string,
    /// then as a backslash-separated list of times.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::NaiveTime;
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from(NaiveTime::from_hms(22, 58, 2)).to_multi_time().ok(),
    ///     Some(vec![NaiveTime::from_hms(22, 58, 2)]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "225802".to_string(),
    ///         "225916.742388".to_string(),
    ///     ]).to_multi_time().ok(),
    ///     Some(vec![
    ///         NaiveTime::from_hms(22, 58, 2),
    ///         NaiveTime::from_hms_micro(22, 59, 16, 742388),
    ///     ]),
    /// );
    /// ```
    pub fn to_multi_time(&self) -> Result<Vec<NaiveTime>, ConvertValueError> {
        match self {
            PrimitiveValue::Time(v) if !v.is_empty() => Ok(v.to_vec()),
            PrimitiveValue::Str(s) => super::deserialize::parse_time(s.as_bytes())
                .map(|(date, _rest)| vec![date])
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Time",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| super::deserialize::parse_time(s.as_bytes()).map(|(date, _rest)| date))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Time",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::U8(bytes) => bytes
                .split(|c| *c == b'\\')
                .into_iter()
                .map(|s| super::deserialize::parse_time(s).map(|(date, _rest)| date))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "Time",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            _ => Err(ConvertValueError {
                requested: "Time",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve a single DICOM date-time from this value.
    ///
    /// If the value is already represented as a date-time,
    /// it is returned as is.
    /// If the value is a string or sequence of strings,
    /// the first string is decoded to obtain a date-time,
    /// potentially failing if the string does not represent a valid time.
    /// If the value in its textual form does not present a time zone,
    /// `default_offset` is used.
    /// If the value is a sequence of U8 bytes, the bytes are
    /// first interpreted as an ASCII character string.
    /// Otherwise, the operation fails.
    ///
    /// Users of this method are advised to retrieve
    /// the default time zone offset
    /// from the same source of the DICOM value.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::{DateTime, FixedOffset, TimeZone};
    /// let default_offset = FixedOffset::east(0);
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from(
    ///         FixedOffset::east(0)
    ///             .ymd(2012, 12, 21)
    ///             .and_hms(9, 30, 1)
    ///     ).to_datetime(default_offset).ok(),
    ///     Some(FixedOffset::east(0)
    ///         .ymd(2012, 12, 21)
    ///         .and_hms(9, 30, 1)
    ///     ),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from("20121221093001")
    ///         .to_datetime(default_offset).ok(),
    ///     Some(FixedOffset::east(0)
    ///         .ymd(2012, 12, 21)
    ///         .and_hms(9, 30, 1)
    ///     ),
    /// );
    /// ```
    pub fn to_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<DateTime<FixedOffset>, ConvertValueError> {
        match self {
            PrimitiveValue::DateTime(v) if !v.is_empty() => Ok(v[0]),
            PrimitiveValue::Str(s) => {
                super::deserialize::parse_datetime(s.trim_end().as_bytes(), default_offset)
                    .context(ParseDateTime)
                    .map_err(|err| ConvertValueError {
                        requested: "DateTime",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::Strs(s) => super::deserialize::parse_datetime(
                s.first().map(|s| s.trim_end().as_bytes()).unwrap_or(&[]),
                default_offset,
            )
            .context(ParseDateTime)
            .map_err(|err| ConvertValueError {
                requested: "DateTime",
                original: self.value_type(),
                cause: Some(err),
            }),
            PrimitiveValue::U8(bytes) => super::deserialize::parse_datetime(bytes, default_offset)
                .context(ParseDateTime)
                .map_err(|err| ConvertValueError {
                    requested: "DateTime",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            _ => Err(ConvertValueError {
                requested: "DateTime",
                original: self.value_type(),
                cause: None,
            }),
        }
    }

    /// Retrieve the full sequence of DICOM date-times from this value.
    ///
    /// If the value is already represented as a sequence of date-times,
    /// it is returned as is.
    /// If the value is a string or sequence of strings,
    /// the strings are decoded to obtain a date, potentially failing if
    /// any of the strings does not represent a valid date.
    /// If the value is a sequence of U8 bytes, the bytes are
    /// first interpreted as an ASCII character string,
    /// then as a backslash-separated list of date-times.
    /// Otherwise, the operation fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// # use chrono::{DateTime, FixedOffset, TimeZone};
    /// let default_offset = FixedOffset::east(0);
    ///
    /// assert_eq!(
    ///     PrimitiveValue::from(
    ///         FixedOffset::east(0)
    ///             .ymd(2012, 12, 21)
    ///             .and_hms(9, 30, 1)
    ///     ).to_multi_datetime(default_offset).ok(),
    ///     Some(vec![FixedOffset::east(0)
    ///         .ymd(2012, 12, 21)
    ///         .and_hms(9, 30, 1)
    ///     ]),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "20121221093001".to_string(),
    ///         "20180102100123".to_string(),
    ///     ]).to_multi_datetime(default_offset).ok(),
    ///     Some(vec![
    ///         FixedOffset::east(0)
    ///             .ymd(2012, 12, 21)
    ///             .and_hms(9, 30, 1),
    ///         FixedOffset::east(0)
    ///             .ymd(2018, 1, 2)
    ///             .and_hms(10, 1, 23)
    ///     ]),
    /// );
    /// ```
    pub fn to_multi_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<Vec<DateTime<FixedOffset>>, ConvertValueError> {
        match self {
            PrimitiveValue::DateTime(v) if !v.is_empty() => Ok(v.to_vec()),
            PrimitiveValue::Str(s) => {
                super::deserialize::parse_datetime(s.trim_end().as_bytes(), default_offset)
                    .map(|date| vec![date])
                    .context(ParseDate)
                    .map_err(|err| ConvertValueError {
                        requested: "DateTime",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::Strs(s) => s
                .into_iter()
                .map(|s| {
                    super::deserialize::parse_datetime(s.trim_end().as_bytes(), default_offset)
                })
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "DateTime",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::U8(bytes) => bytes
                .split(|c| *c == b'\\')
                .into_iter()
                .map(|s| super::deserialize::parse_datetime(s, default_offset))
                .collect::<Result<Vec<_>, _>>()
                .context(ParseDate)
                .map_err(|err| ConvertValueError {
                    requested: "DateTime",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            _ => Err(ConvertValueError {
                requested: "Date",
                original: self.value_type(),
                cause: None,
            }),
        }
    }
}

/// Macro for implementing getters to single and multi-values of each variant.
///
/// Should be placed inside `PrimitiveValue`'s impl block.
macro_rules! impl_primitive_getters {
    ($name_single: ident, $name_multi: ident, $variant: ident, $ret: ty) => {
        /// Get a single value of the requested type.
        /// If it contains multiple values,
        /// only the first one is returned.
        /// An error is returned if the variant is not compatible.
        pub fn $name_single(&self) -> Result<$ret, CastValueError> {
            match self {
                PrimitiveValue::$variant(c) if c.is_empty() => Err(CastValueError {
                    requested: stringify!($name_single),
                    got: ValueType::Empty,
                }),
                PrimitiveValue::$variant(c) => Ok(c[0]),
                value => Err(CastValueError {
                    requested: stringify!($name_single),
                    got: value.value_type(),
                }),
            }
        }

        /// Get a sequence of values of the requested type without copying.
        /// An error is returned if the variant is not compatible.
        pub fn $name_multi(&self) -> Result<&[$ret], CastValueError> {
            match self {
                PrimitiveValue::$variant(c) => Ok(&c),
                value => Err(CastValueError {
                    requested: stringify!($name_multi),
                    got: value.value_type(),
                }),
            }
        }
    };
}

/// Per variant, strongly checked getters to DICOM values.
///
/// Conversions from one representation to another do not take place
/// when using these methods.
impl PrimitiveValue {
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
        use self::PrimitiveValue::*;
        match self {
            Strs(c) if c.is_empty() => Err(CastValueError {
                requested: "Str",
                got: ValueType::Empty,
            }),
            Strs(c) if !c.is_empty() => Ok(&c[0]),
            Str(s) => Ok(s),
            value => Err(CastValueError {
                requested: "Str",
                got: value.value_type(),
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
        use self::PrimitiveValue::*;
        match self {
            Strs(c) => Ok(&c),
            Str(s) => Ok(std::slice::from_ref(s)),
            value => Err(CastValueError {
                requested: "strings",
                got: value.value_type(),
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

/// The output of this method is equivalent to calling the method `to_str`
impl std::fmt::Display for PrimitiveValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        /// Auxilliary function for turning a sequence of values
        /// into a backslash-delimited string.
        fn seq_to_str<I>(iter: I) -> String
        where
            I: IntoIterator,
            I::Item: std::fmt::Display,
        {
            iter.into_iter().map(|x| x.to_string()).join("\\")
        }

        match self {
            PrimitiveValue::Empty => Ok(()),
            PrimitiveValue::Str(value) => f.write_str(value),
            PrimitiveValue::Strs(values) => {
                if values.len() == 1 {
                    f.write_str(&values[0])
                } else {
                    f.write_str(&seq_to_str(values))
                }
            }
            PrimitiveValue::Date(values) => f.write_str(
                &values
                    .into_iter()
                    .map(|date| date.format("%Y%m%d").to_string())
                    .join("\\"),
            ),
            PrimitiveValue::Time(values) => f.write_str(
                &values
                    .into_iter()
                    .map(|date| date.format("%H%M%S%.6f").to_string())
                    .join("\\"),
            ),
            PrimitiveValue::DateTime(values) => f.write_str(
                &values
                    .into_iter()
                    .map(|date| date.format("%Y%m%d%H%M%S%.6f%z").to_string())
                    .join("\\"),
            ),
            PrimitiveValue::U8(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::U16(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::U32(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::I16(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::I32(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::U64(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::I64(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::F32(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::F64(values) => f.write_str(&seq_to_str(values)),
            PrimitiveValue::Tags(values) => f.write_str(&seq_to_str(values)),
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
/// plus the `Item` and `PixelSequence` entries.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueType {
    /// No data. Used for any value of length 0.
    Empty,

    /// An item. Used for elements in a SQ, regardless of content.
    Item,

    /// An item. Used for the values of encapsulated pixel data.
    PixelSequence,

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

impl DicomValueType for PrimitiveValue {
    fn value_type(&self) -> ValueType {
        match *self {
            PrimitiveValue::Empty => ValueType::Empty,
            PrimitiveValue::Date(_) => ValueType::Date,
            PrimitiveValue::DateTime(_) => ValueType::DateTime,
            PrimitiveValue::F32(_) => ValueType::F32,
            PrimitiveValue::F64(_) => ValueType::F64,
            PrimitiveValue::I16(_) => ValueType::I16,
            PrimitiveValue::I32(_) => ValueType::I32,
            PrimitiveValue::I64(_) => ValueType::I64,
            PrimitiveValue::Str(_) => ValueType::Str,
            PrimitiveValue::Strs(_) => ValueType::Strs,
            PrimitiveValue::Tags(_) => ValueType::Tags,
            PrimitiveValue::Time(_) => ValueType::Time,
            PrimitiveValue::U16(_) => ValueType::U16,
            PrimitiveValue::U32(_) => ValueType::U32,
            PrimitiveValue::U64(_) => ValueType::U64,
            PrimitiveValue::U8(_) => ValueType::U8,
        }
    }

    fn cardinality(&self) -> usize {
        match self {
            PrimitiveValue::Empty => 0,
            PrimitiveValue::Str(_) => 1,
            PrimitiveValue::Date(b) => b.len(),
            PrimitiveValue::DateTime(b) => b.len(),
            PrimitiveValue::F32(b) => b.len(),
            PrimitiveValue::F64(b) => b.len(),
            PrimitiveValue::I16(b) => b.len(),
            PrimitiveValue::I32(b) => b.len(),
            PrimitiveValue::I64(b) => b.len(),
            PrimitiveValue::Strs(b) => b.len(),
            PrimitiveValue::Tags(b) => b.len(),
            PrimitiveValue::Time(b) => b.len(),
            PrimitiveValue::U16(b) => b.len(),
            PrimitiveValue::U32(b) => b.len(),
            PrimitiveValue::U64(b) => b.len(),
            PrimitiveValue::U8(b) => b.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CastValueError, ConvertValueError, InvalidValueReadError};
    use crate::dicom_value;
    use crate::value::{PrimitiveValue, ValueType};
    use chrono::{FixedOffset, NaiveDate, NaiveTime, TimeZone};
    use smallvec::smallvec;

    #[test]
    fn primitive_value_to_str() {
        assert_eq!(PrimitiveValue::Empty.to_str(), "");

        // does not copy on a single string
        let value = PrimitiveValue::Str("Smith^John".to_string());
        let string = value.to_str();
        assert_eq!(string, "Smith^John",);
        match string {
            std::borrow::Cow::Borrowed(_) => {} // good
            _ => panic!("expected string to be borrowed, but was owned"),
        }

        assert_eq!(
            PrimitiveValue::Date(smallvec![NaiveDate::from_ymd(2014, 10, 12)]).to_str(),
            "20141012",
        );
        assert_eq!(
            dicom_value!(Strs, ["DERIVED", "PRIMARY", "WHOLE BODY", "EMISSION"]).to_str(),
            "DERIVED\\PRIMARY\\WHOLE BODY\\EMISSION",
        );

        // sequence of numbers
        let value = PrimitiveValue::from(vec![10, 11, 12]);
        assert_eq!(value.to_str(), "10\\11\\12",);
    }

    #[test]
    fn primitive_value_to_clean_str() {
        //Removes whitespace at the end of a string
        let value = PrimitiveValue::from("1.2.345\0".to_string());
        assert_eq!(&value.to_clean_str(), "1.2.345");

        //Removes whitespace at the end on multiple strings
        let value = dicom_value!(Strs, ["ONE", "TWO", "THREE", "SIX "]);
        assert_eq!(&value.to_clean_str(), "ONE\\TWO\\THREE\\SIX");

        //Maintains the leading whitespace on a string and removes at the end
        let value = PrimitiveValue::from("\01.2.345\0".to_string());
        assert_eq!(&value.to_clean_str(), "\01.2.345");

        //Maintains the leading whitespace on multiple strings and removes at the end
        let value = dicom_value!(Strs, [" ONE", "TWO", "THREE", " SIX "]);
        assert_eq!(&value.to_clean_str(), " ONE\\TWO\\THREE\\ SIX");
    }

    #[test]
    fn primitive_value_to_bytes() {
        assert_eq!(PrimitiveValue::Empty.to_bytes(), &[][..]);

        if cfg!(target_endian = "little") {
            assert_eq!(
                PrimitiveValue::U16(smallvec![1, 2, 0x0601,]).to_bytes(),
                &[0x01, 0x00, 0x02, 0x00, 0x01, 0x06][..],
            );
        } else {
            assert_eq!(
                PrimitiveValue::U16(smallvec![0x0001, 0x0002, 0x0601,]).to_bytes(),
                &[0x00, 0x01, 0x00, 0x02, 0x06, 0x01][..],
            );
        }

        // does not copy on a single string
        let value = PrimitiveValue::from("Smith^John");
        let bytes = value.to_bytes();
        assert_eq!(bytes, &b"Smith^John"[..],);
        match bytes {
            std::borrow::Cow::Borrowed(_) => {} // good
            _ => panic!("expected bytes to be borrowed, but are owned"),
        }

        assert_eq!(
            PrimitiveValue::Date(smallvec![NaiveDate::from_ymd(2014, 10, 12)]).to_bytes(),
            &b"20141012"[..],
        );
        assert_eq!(
            dicom_value!(Strs, ["DERIVED", "PRIMARY", "WHOLE BODY", "EMISSION",]).to_bytes(),
            &b"DERIVED\\PRIMARY\\WHOLE BODY\\EMISSION"[..],
        );

        // does not copy on bytes
        let value = PrimitiveValue::from(vec![0x99; 16]);
        let bytes = value.to_bytes();
        assert_eq!(bytes, &[0x99; 16][..],);
        match bytes {
            std::borrow::Cow::Borrowed(_) => {} // good
            _ => panic!("expected bytes to be borrowed, but are owned"),
        }
    }

    #[test]
    fn primitive_value_to_int() {
        assert!(PrimitiveValue::Empty.to_int::<i32>().is_err());

        // exact match
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_u16),
        );
        // conversions are automatically applied
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_u32),
        );
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_i64),
        );
        assert_eq!(
            PrimitiveValue::from(0x0601_u16).to_int().ok(),
            Some(0x0601_u64),
        );

        // takes the first number
        assert_eq!(dicom_value!(I32, [1, 2, 5]).to_int().ok(), Some(1),);

        // admits an integer as text
        assert_eq!(dicom_value!(Strs, ["-73", "2"]).to_int().ok(), Some(-73),);

        // does not admit destructive conversions
        assert!(PrimitiveValue::from(-1).to_int::<u32>().is_err());

        // does not admit strings which are not numbers
        assert!(matches!(
            dicom_value!(Strs, ["Smith^John"]).to_int::<u8>(),
            Err(ConvertValueError {
                requested: _,
                original: ValueType::Strs,
                // would try to parse as an integer and fail
                cause: Some(InvalidValueReadError::ParseInteger { .. }),
            })
        ));
    }

    #[test]
    fn primitive_value_to_multi_int() {
        assert_eq!(PrimitiveValue::Empty.to_multi_int::<i32>().unwrap(), vec![]);

        let test_value = dicom_value!(U16, [0x0601, 0x5353, 3, 4]);
        // exact match
        let numbers = test_value.to_multi_int::<u16>().unwrap();
        assert_eq!(numbers, vec![0x0601, 0x5353, 3, 4],);
        // type is inferred on context
        let numbers: Vec<u32> = test_value.to_multi_int().unwrap();
        assert_eq!(numbers, vec![0x0601_u32, 0x5353, 3, 4],);
        let numbers: Vec<i64> = test_value.to_multi_int().unwrap();
        assert_eq!(numbers, vec![0x0601_i64, 0x5353, 3, 4],);
        assert_eq!(
            test_value.to_multi_int::<u64>().unwrap(),
            vec![0x0601_u64, 0x5353, 3, 4],
        );

        // takes all numbers
        assert_eq!(
            dicom_value!(I32, [1, 2, 5]).to_multi_int().ok(),
            Some(vec![1, 2, 5]),
        );

        // admits a integer as text, trailing space too
        assert_eq!(
            dicom_value!(Strs, ["-73", "2 "]).to_multi_int().ok(),
            Some(vec![-73, 2]),
        );

        // does not admit destructive conversions
        assert!(matches!(
            dicom_value!(I32, [0, 1, -1]).to_multi_int::<u64>(),
            Err(ConvertValueError {
                original: ValueType::I32,
                // the cast from -1_i32 to u32 would fail
                cause: Some(InvalidValueReadError::NarrowConvert {
                    value: x,
                   ..
                }),
                ..
            }) if &x == "-1"
        ));

        // not even from strings
        assert!(matches!(
            dicom_value!(Strs, ["0", "1", "-1"]).to_multi_int::<u16>(),
            Err(ConvertValueError {
                original: ValueType::Strs,
                // the conversion from "-1" to u32 would fail
                cause: Some(InvalidValueReadError::ParseInteger { .. }),
                ..
            })
        ));

        // does not admit strings which are not numbers
        assert!(matches!(
            dicom_value!(Strs, ["Smith^John"]).to_int::<u8>(),
            Err(ConvertValueError {
                requested: _,
                original: ValueType::Strs,
                // would try to parse as an integer and fail
                cause: Some(InvalidValueReadError::ParseInteger { .. }),
            })
        ));
    }

    #[test]
    fn primitive_value_to_multi_floats() {
        assert_eq!(PrimitiveValue::Empty.to_multi_float32().ok(), Some(vec![]));

        let test_value = dicom_value!(U16, [1, 2, 3, 4]);

        assert_eq!(
            test_value.to_multi_float32().ok(),
            Some(vec![1., 2., 3., 4.]),
        );
        assert_eq!(
            test_value.to_multi_float64().ok(),
            Some(vec![1., 2., 3., 4.]),
        );

        // admits a number as text, trailing space too
        assert_eq!(
            dicom_value!(Strs, ["7.25", "-12.5 "])
                .to_multi_float64()
                .ok(),
            Some(vec![7.25, -12.5]),
        );

        // does not admit strings which are not numbers
        assert!(matches!(
            dicom_value!(Strs, ["Smith^John"]).to_multi_float64(),
            Err(ConvertValueError {
                requested: _,
                original: ValueType::Strs,
                // would try to parse as a float and fail
                cause: Some(InvalidValueReadError::ParseFloat { .. }),
            })
        ));
    }

    #[test]
    fn primitive_value_to_date() {
        // trivial conversion
        assert_eq!(
            PrimitiveValue::Date(smallvec![NaiveDate::from_ymd(2014, 10, 12)])
                .to_date()
                .unwrap(),
            NaiveDate::from_ymd(2014, 10, 12),
        );
        // from text (Str)
        assert_eq!(
            dicom_value!(Str, "20141012").to_date().unwrap(),
            NaiveDate::from_ymd(2014, 10, 12),
        );
        // from text (Strs)
        assert_eq!(
            dicom_value!(Strs, ["20141012"]).to_date().unwrap(),
            NaiveDate::from_ymd(2014, 10, 12),
        );
        // from bytes
        assert_eq!(
            PrimitiveValue::from(b"20141012").to_date().unwrap(),
            NaiveDate::from_ymd(2014, 10, 12),
        );
        // not a date
        assert!(PrimitiveValue::Str("Smith^John".to_string())
            .to_date()
            .is_err());
        // not a date
        assert!(matches!(
            PrimitiveValue::Str("Smith^John".to_string()).to_date(),
            Err(ConvertValueError {
                requested: "Date",
                original: ValueType::Str,
                // would try to parse as a date and fail
                cause: Some(_),
            })
        ));
    }

    #[test]
    fn primitive_value_to_time() {
        // trivial conversion
        assert_eq!(
            PrimitiveValue::from(NaiveTime::from_hms(11, 9, 26))
                .to_time()
                .unwrap(),
            NaiveTime::from_hms(11, 9, 26),
        );
        // from text (Str)
        assert_eq!(
            dicom_value!(Str, "110926").to_time().unwrap(),
            NaiveTime::from_hms(11, 9, 26),
        );
        // from text (Strs)
        assert_eq!(
            dicom_value!(Strs, ["110926"]).to_time().unwrap(),
            NaiveTime::from_hms(11, 9, 26),
        );
        // from text (Strs) with fraction of a second
        assert_eq!(
            dicom_value!(Strs, ["110926.123456"]).to_time().unwrap(),
            NaiveTime::from_hms_micro(11, 9, 26, 123_456),
        );
        // from bytes with fraction of a second
        assert_eq!(
            PrimitiveValue::from(&b"110926.987"[..]).to_time().unwrap(),
            NaiveTime::from_hms_milli(11, 9, 26, 987),
        );
        // not a time
        assert!(matches!(
            PrimitiveValue::Str("Smith^John".to_string()).to_time(),
            Err(ConvertValueError {
                requested: "Time",
                original: ValueType::Str,
                ..
            })
        ));
    }

    #[test]
    fn calculate_byte_len() {
        // single even string
        // b"ABCD"
        let val = dicom_value!("ABCD");
        assert_eq!(val.calculate_byte_len(), 4);

        // multi string, no padding
        // b"ABCD\\EFG"
        let val = dicom_value!(Strs, ["ABCD", "EFG"]);
        assert_eq!(val.calculate_byte_len(), 8);

        // multi string with padding
        // b"ABCD\\EFGH "
        let val = dicom_value!(Strs, ["ABCD", "EFGH"]);
        assert_eq!(val.calculate_byte_len(), 10);

        // multi date, no padding
        // b"20141012\\20200915\\20180101"
        let val = dicom_value!(
            Date,
            [
                NaiveDate::from_ymd(2014, 10, 12),
                NaiveDate::from_ymd(2020, 9, 15),
                NaiveDate::from_ymd(2018, 1, 1)
            ]
        );
        assert_eq!(val.calculate_byte_len(), 26);

        // multi date with padding
        // b"20141012\\20200915 "
        let val = dicom_value!(
            Date,
            [
                NaiveDate::from_ymd(2014, 10, 12),
                NaiveDate::from_ymd(2020, 9, 15)
            ]
        );
        assert_eq!(val.calculate_byte_len(), 18);

        // single time with second fragment
        // b"185530.4756"
        let val = dicom_value!(NaiveTime::from_hms_micro(18, 55, 30, 475600));
        assert_eq!(val.calculate_byte_len(), 12);

        // multi time with padding
        // b"185530\\185530 "
        let val = dicom_value!(
            Time,
            [
                NaiveTime::from_hms(18, 55, 30),
                NaiveTime::from_hms(18, 55, 30)
            ]
        );
        assert_eq!(val.calculate_byte_len(), 14);

        // single date-time with time zone, no second fragment
        // b"20121221093001+0100"
        let val = PrimitiveValue::from(FixedOffset::east(1).ymd(2012, 12, 21).and_hms(9, 30, 1));
        assert_eq!(val.calculate_byte_len(), 20);
    }

    #[test]
    fn primitive_value_get() {
        assert_eq!(
            dicom_value!(Strs, ["Smith^John"]).string().unwrap(),
            "Smith^John"
        );

        assert_eq!(
            dicom_value!(Strs, ["Smith^John"]).strings().unwrap(),
            &["Smith^John"]
        );

        assert_eq!(dicom_value!(I32, [1, 2, 5]).int32().unwrap(), 1,);

        assert_eq!(
            dicom_value!(I32, [1, 2, 5]).int32_slice().unwrap(),
            &[1, 2, 5],
        );

        assert!(matches!(
            dicom_value!(I32, [1, 2, 5]).uint32(),
            Err(CastValueError {
                requested: "uint32",
                got: ValueType::I32,
                ..
            })
        ));

        assert!(matches!(
            dicom_value!(I32, [1, 2, 5]).strings(),
            Err(CastValueError {
                requested: "strings",
                got: ValueType::I32,
                ..
            })
        ));

        assert_eq!(
            PrimitiveValue::Date(smallvec![NaiveDate::from_ymd(2014, 10, 12)])
                .date()
                .unwrap(),
            NaiveDate::from_ymd(2014, 10, 12),
        );

        assert!(matches!(
            PrimitiveValue::Date(smallvec![NaiveDate::from_ymd(2014, 10, 12)]).time(),
            Err(CastValueError {
                requested: "time",
                got: ValueType::Date,
                ..
            })
        ));
    }
}
