//! Declaration and implementation of a DICOM primitive value.
//!
//! See [`PrimitiveValue`].

use super::{AsRange, DicomValueType};
use crate::header::{HasLength, Length, Tag};
use crate::value::partial::{DateComponent, DicomDate, DicomDateTime, DicomTime};
use crate::value::person_name::PersonName;
use crate::value::range::{AmbiguousDtRangeParser, DateRange, DateTimeRange, TimeRange};
use itertools::Itertools;
use num_traits::NumCast;
use safe_transmute::to_bytes::transmute_to_bytes;
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::borrow::Cow;
use std::fmt::{self, Display};
use std::str::FromStr;

// conversion method impls are categorized to dedicated modules
mod datetime;
mod float;
mod int;

/// Triggered when a value reading attempt fails.
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
    #[snafu(display("Failed to convert into a DicomDate"))]
    IntoDicomDate {
        #[snafu(backtrace)]
        source: crate::value::partial::Error,
    },
    #[snafu(display("Failed to convert into a DicomTime"))]
    IntoDicomTime {
        #[snafu(backtrace)]
        source: crate::value::partial::Error,
    },
    #[snafu(display("Failed to convert into a DicomDateTime"))]
    IntoDicomDateTime {
        #[snafu(backtrace)]
        source: crate::value::partial::Error,
    },
    #[snafu(display("Failed to read text as a date range"))]
    ParseDateRange {
        #[snafu(backtrace)]
        source: crate::value::range::Error,
    },
    #[snafu(display("Failed to read text as a time range"))]
    ParseTimeRange {
        #[snafu(backtrace)]
        source: crate::value::range::Error,
    },
    #[snafu(display("Failed to read text as a date-time range"))]
    ParseDateTimeRange {
        #[snafu(backtrace)]
        source: crate::value::range::Error,
    },
}

/// Error type for a failed attempt to modify an existing DICOM primitive value.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum ModifyValueError {
    /// The modification using strings cannot proceed
    /// due to the value's current type,
    /// as that would lead to mixed representations.
    #[snafu(display("cannot not modify {:?} value as string values", original))]
    IncompatibleStringType { original: ValueType },

    /// The modification using numbers cannot proceed
    /// due to the value's current type,
    /// as that would lead to mixed representations.
    #[snafu(display("cannot not modify {:?} value as numeric values", original))]
    IncompatibleNumberType { original: ValueType },
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
/// [`PrimitiveValue::to_date`] instead of `date`,
/// [`PrimitiveValue::to_str`] instead of `string`, and so on.
/// The error type would then be [`ConvertValueError`].
///
/// [`PrimitiveValue::to_date`]: crate::value::primitive::PrimitiveValue::to_date
/// [`PrimitiveValue::to_str`]: crate::value::primitive::PrimitiveValue::to_str
#[derive(Debug, Clone, PartialEq)]
pub struct CastValueError {
    /// The value format requested
    pub requested: &'static str,
    /// The value's actual representation
    pub got: ValueType,
}

impl Display for CastValueError {
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
    pub cause: Option<Box<InvalidValueReadError>>,
}

impl Display for ConvertValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "could not convert {:?} to a {}: ",
            self.original, self.requested
        )?;
        if let Some(cause) = &self.cause {
            write!(f, "{cause}")?;
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
pub use chrono::{NaiveDate, NaiveTime};

/// An aggregation of one or more elements in a value.
pub type C<T> = SmallVec<[T; 2]>;

/// An enum representing a primitive value from a DICOM element.
/// The result of decoding an element's data value
/// may be one of the enumerated types
/// depending on its content and value representation.
///
/// Multiple elements are contained in a [`smallvec`] vector,
/// conveniently aliased to the type [`crate::value::C`].
///
/// See the macro [`dicom_value!`] for a more intuitive means
/// of constructing these values.
/// Alternatively, `From` conversions into [`PrimitiveValue`] exist
/// for single element types,
/// including numeric types, [`String`], and [`&str`].
///
/// [`dicom_value!`]: crate::dicom_value!
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
#[derive(Debug, Clone)]
pub enum PrimitiveValue {
    /// No data. Usually employed for zero-length values.
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

    /// The value is a sequence of unsigned 8-bit integers.
    /// Used for OB and UN.
    U8(C<u8>),

    /// The value is a sequence of signed 16-bit integers.
    /// Used for SS.
    I16(C<i16>),

    /// A sequence of unsigned 16-bit integers.
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

    /// A sequence of dates with arbitrary precision.
    /// Used for the DA representation.
    Date(C<DicomDate>),

    /// A sequence of date-time values with arbitrary precision.
    /// Used for the DT representation.
    DateTime(C<DicomDateTime>),

    /// A sequence of time values with arbitrary precision.
    /// Used for the TM representation.
    Time(C<DicomTime>),
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
impl_from_for_primitive!(DicomDate, Date);
impl_from_for_primitive!(DicomTime, Time);
impl_from_for_primitive!(DicomDateTime, DateTime);

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

impl From<PersonName<'_>> for PrimitiveValue {
    fn from(p: PersonName) -> Self {
        PrimitiveValue::Str(p.to_dicom_string())
    }
}

impl From<()> for PrimitiveValue {
    /// constructs an empty DICOM value
    #[inline]
    fn from(_value: ()) -> Self {
        PrimitiveValue::Empty
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
impl_from_array_for_primitive_1_to_8!(DicomDate, Date);
impl_from_array_for_primitive_1_to_8!(DicomTime, Time);
impl_from_array_for_primitive_1_to_8!(DicomDateTime, DateTime);

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
    pub fn new_i32(value: i32) -> Self {
        PrimitiveValue::I32(C::from_elem(value, 1))
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
            Str(s) => s.len(),
            Strs(c) => c.iter().map(|s| s.len() + 1).sum::<usize>() & !1,
            Date(c) => {
                c.iter()
                    .map(|d| PrimitiveValue::da_byte_len(d) + 1)
                    .sum::<usize>()
                    & !1
            }
            Time(c) => {
                c.iter()
                    .map(|t| PrimitiveValue::tm_byte_len(t) + 1)
                    .sum::<usize>()
                    & !1
            }
            DateTime(c) => {
                c.iter()
                    .map(|dt| PrimitiveValue::dt_byte_len(dt) + 1)
                    .sum::<usize>()
                    & !1
            }
        }
    }

    fn da_byte_len(date: &DicomDate) -> usize {
        match date.precision() {
            DateComponent::Year => 4,
            DateComponent::Month => 6,
            DateComponent::Day => 8,
            _ => panic!("Impossible precision for a DicomDate"),
        }
    }

    fn tm_byte_len(time: &DicomTime) -> usize {
        match time.precision() {
            DateComponent::Hour => 2,
            DateComponent::Minute => 4,
            DateComponent::Second => 6,
            DateComponent::Fraction => match time.fraction_and_precision() {
                None => panic!("DicomTime has fraction precision but no fraction can be retrieved"),
                Some((_, fp)) => 7 + fp as usize, // 1 is for the '.'
            },
            _ => panic!("Impossible precision for a Dicomtime"),
        }
    }

    fn dt_byte_len(datetime: &DicomDateTime) -> usize {
        PrimitiveValue::da_byte_len(datetime.date())
            + match datetime.time() {
                Some(time) => PrimitiveValue::tm_byte_len(time),
                None => 0,
            }
            + match datetime.has_time_zone() {
                true => 5,
                false => 0,
            }
    }

    /// Convert the primitive value into a string representation.
    ///
    /// String values already encoded with the [`Str`] and
    /// [`Strs`] variants
    /// are provided as is.
    /// In the case of [`Strs`], the strings are first joined together
    /// with a backslash (`'\\'`).
    /// All other type variants are first converted to a string,
    /// then joined together with a backslash.
    ///
    /// Trailing whitespace is stripped from each string.
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that [`to_str`] returns a string with
    /// the exact same byte sequence as the one originally found
    /// at the source of the value,
    /// even for the string variants.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// [`Str`]: Self::Str
    /// [`Strs`]: Self::Strs
    /// [`to_str`]: Self::to_str
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::{C, PrimitiveValue, DicomDate};
    /// # use smallvec::smallvec;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John").to_str(),
    ///     "Smith^John",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Date, DicomDate::from_y(2014)?).to_str(),
    ///     "2014",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John\0").to_str(),
    ///     "Smith^John",
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
    /// Ok(())
    /// }
    /// ```
    pub fn to_str(&self) -> Cow<'_, str> {
        match self {
            PrimitiveValue::Empty => Cow::from(""),
            PrimitiveValue::Str(values) => Cow::from(values.trim_end_matches([' ', '\u{0}'])),
            PrimitiveValue::Strs(values) => {
                if values.len() == 1 {
                    Cow::from(values[0].trim_end_matches([' ', '\u{0}']))
                } else {
                    Cow::Owned(
                        values
                            .iter()
                            .map(|s| s.trim_end_matches([' ', '\u{0}']))
                            .join("\\"),
                    )
                }
            }
            prim => Cow::from(prim.to_string()),
        }
    }

    /// Convert the primitive value into a raw string representation.
    ///
    /// String values already encoded with the [`Str`] and
    /// [`Strs`] variants
    /// are provided as is.
    /// In the case of [`Strs`], the strings are first joined together
    /// with a backslash (`'\\'`).
    /// All other type variants are first converted to a string,
    /// then joined together with a backslash.
    ///
    /// This method keeps all trailing whitespace,
    /// unlike [`to_str`].
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that [`to_raw_str`] returns a string with
    /// the exact same byte sequence as the one originally found
    /// at the source of the value,
    /// even for the string variants.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// [`Str`]: Self::Str
    /// [`Strs`]: Self::Strs
    /// [`to_str`]: Self::to_str
    /// [`to_raw_str`]: Self::to_raw_str
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::{C, DicomDate, PrimitiveValue};
    /// # use smallvec::smallvec;
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John\0").to_raw_str(),
    ///     "Smith^John\0",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Date, DicomDate::from_ymd(2014, 10, 12).unwrap()).to_raw_str(),
    ///     "2014-10-12",
    /// );
    /// assert_eq!(
    ///     dicom_value!(Strs, [
    ///         "DERIVED",
    ///         " PRIMARY ",
    ///         "WHOLE BODY",
    ///         "EMISSION ",
    ///     ])
    ///     .to_raw_str(),
    ///     "DERIVED\\ PRIMARY \\WHOLE BODY\\EMISSION ",
    /// );
    /// ```
    pub fn to_raw_str(&self) -> Cow<'_, str> {
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
    /// String values already encoded with the [`Str`] and
    /// [`Strs`] variants are provided as is.
    /// All other type variants are first converted to a string,
    /// then collected into a vector.
    ///
    /// Trailing whitespace is stripped from each string.
    /// If keeping it is desired,
    /// use [`to_raw_str`].
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that [`to_multi_str`] returns strings with
    /// the exact same byte sequence as the one originally found
    /// at the source of the value,
    /// even for the string variants.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// [`Str`]: Self::Str
    /// [`Strs`]: Self::Strs
    /// [`to_raw_str`]: Self::to_raw_str
    /// [`to_multi_str`]: Self::to_multi_str
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::{C, PrimitiveValue, DicomDate};
    /// # use smallvec::smallvec;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// assert_eq!(
    ///     dicom_value!(Strs, [
    ///         "DERIVED",
    ///         "PRIMARY",
    ///         "WHOLE BODY ",
    ///         " EMISSION ",
    ///     ])
    ///     .to_multi_str(),
    ///     &["DERIVED", "PRIMARY", "WHOLE BODY", " EMISSION"][..],
    /// );
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John").to_multi_str(),
    ///     &["Smith^John"][..],
    /// );
    /// assert_eq!(
    ///     dicom_value!(Str, "Smith^John\0").to_multi_str(),
    ///     &["Smith^John"][..],
    /// );
    /// assert_eq!(
    ///     dicom_value!(Date, DicomDate::from_ym(2014, 10)?).to_multi_str(),
    ///     &["201410"][..],
    /// );
    /// assert_eq!(
    ///     dicom_value!(I64, [128, 256, 512]).to_multi_str(),
    ///     &["128", "256", "512"][..],
    /// );
    /// Ok(())
    /// }
    /// ```
    pub fn to_multi_str(&self) -> Cow<'_, [String]> {
        /// Auxiliary function for turning a sequence of values
        /// into a sequence of strings.
        fn seq_to_str<I>(iter: I) -> Vec<String>
        where
            I: IntoIterator,
            I::Item: Display,
        {
            iter.into_iter().map(|x| x.to_string()).collect()
        }

        match self {
            PrimitiveValue::Empty => Cow::from(&[][..]),
            PrimitiveValue::Str(_) => Cow::Owned(vec![self.to_str().to_string()]),
            PrimitiveValue::Strs(_) => {
                Cow::Owned(self.to_str().split('\\').map(|s| s.to_string()).collect())
            }
            PrimitiveValue::Date(values) => values
                .into_iter()
                .map(|date| date.to_encoded())
                .collect::<Vec<_>>()
                .into(),
            PrimitiveValue::Time(values) => values
                .into_iter()
                .map(|time| time.to_encoded())
                .collect::<Vec<_>>()
                .into(),
            PrimitiveValue::DateTime(values) => values
                .into_iter()
                .map(|dt| dt.to_encoded())
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

    /// Retrieve this DICOM value as raw bytes.
    ///
    /// Binary numeric values are returned with a reinterpretation
    /// of the holding vector's occupied data block as bytes,
    /// without copying,
    /// under the platform's native byte order.
    ///
    /// String values already encoded with the [`Str`] and [`Strs`]
    /// variants are provided as their respective bytes in UTF-8.
    /// In the case of [`Strs`], the strings are first joined together
    /// with a backslash (`'\\'`).
    /// Other type variants are first converted to a string,
    /// joined together with a backslash,
    /// then turned into a byte vector.
    /// For values which are inherently textual according the standard,
    /// this is equivalent to calling [`String::as_bytes`] after [`to_str`].
    ///
    /// **Note:**
    /// As the process of reading a DICOM value
    /// may not always preserve its original nature,
    /// it is not guaranteed that [`to_bytes`] returns the same byte sequence
    /// as the one originally found at the source of the value.
    /// Therefore, this method is not reliable
    /// for compliant DICOM serialization.
    ///
    /// [`Str`]: Self::Str
    /// [`Strs`]: Self::Strs
    /// [`to_str`]: Self::to_str
    /// [`to_bytes`]: Self::to_bytes
    ///
    /// # Examples
    ///
    /// `U8` provides a straight, zero-copy slice of bytes.
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue, DicomDate};
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
    /// # use dicom_core::value::{C, PrimitiveValue, DicomDate};
    /// # use smallvec::smallvec;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// assert_eq!(
    ///     PrimitiveValue::from("Smith^John").to_bytes(),
    ///     &b"Smith^John"[..],
    /// );
    /// assert_eq!(
    ///     PrimitiveValue::from(DicomDate::from_ymd(2014, 10, 12)?)
    ///     .to_bytes(),
    ///     &b"2014-10-12"[..],
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
    /// Ok(())
    /// }
    /// ```
    pub fn to_bytes(&self) -> Cow<'_, [u8]> {
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

    /// Retrieve a single [`PersonName`] from this value.
    ///
    /// If the value is a string or sequence of strings,
    /// the first string is split to obtain a [`PersonName`].
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::value::{C, PrimitiveValue};
    /// # use std::error::Error;
    /// use dicom_core::value::PersonName;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// let value = PrimitiveValue::from("Tooms^Victor^Eugene");
    /// // PersonName contains borrowed values
    /// let pn = value.to_person_name()?;
    ///
    /// assert_eq!(pn.given(), Some("Victor"));
    /// assert_eq!(pn.middle(), Some("Eugene"));
    /// assert!(pn.prefix().is_none());
    ///
    /// let value2 = PrimitiveValue::from(pn);
    ///
    /// assert_eq!(value, value2);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_person_name(&self) -> Result<PersonName<'_>, ConvertValueError> {
        match self {
            PrimitiveValue::Str(s) => Ok(PersonName::from_text(s)),
            PrimitiveValue::Strs(s) => s.first().map_or_else(
                || {
                    Err(ConvertValueError {
                        requested: "PersonName",
                        original: self.value_type(),
                        cause: None,
                    })
                },
                |s| Ok(PersonName::from_text(s)),
            ),
            _ => Err(ConvertValueError {
                requested: "PersonName",
                original: self.value_type(),
                cause: None,
            }),
        }
    }
}

/// Macro for implementing getters to single and multi-values of each variant.
///
/// Should be placed inside [`PrimitiveValue`]'s impl block.
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
    /// see [`to_str`] instead.
    ///
    /// [`to_str`]: Self::to_str
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
    /// if the variant is either [`Str`] or
    /// [`Strs`].
    ///
    /// An error is returned if the variant is not compatible.
    ///
    /// To enable conversions of other variants to a textual representation,
    /// see [`to_str`] instead.
    ///
    /// [`Str`]: Self::Str
    /// [`Strs`]: Self::Strs
    /// [`to_str`]: Self::to_str
    pub fn strings(&self) -> Result<&[String], CastValueError> {
        use self::PrimitiveValue::*;
        match self {
            Strs(c) => Ok(c),
            Str(s) => Ok(std::slice::from_ref(s)),
            value => Err(CastValueError {
                requested: "strings",
                got: value.value_type(),
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

    /// Extend a textual value by appending
    /// more strings to an existing text or empty value.
    ///
    /// An error is returned if the current value is not textual.
    ///
    /// # Example
    ///
    /// ```
    /// use dicom_core::dicom_value;
    /// # use dicom_core::value::ModifyValueError;
    ///
    /// # fn main() -> Result<(), ModifyValueError> {
    /// let mut value = dicom_value!(Strs, ["Hello"]);
    /// value.extend_str(["DICOM"])?;
    /// assert_eq!(value.to_string(), "Hello\\DICOM");
    /// # Ok(())
    /// # }
    /// ```
    pub fn extend_str<T>(
        &mut self,
        strings: impl IntoIterator<Item = T>,
    ) -> Result<(), ModifyValueError>
    where
        T: Into<String>,
    {
        match self {
            PrimitiveValue::Empty => {
                *self = PrimitiveValue::Strs(strings.into_iter().map(T::into).collect());
                Ok(())
            }
            PrimitiveValue::Strs(elements) => {
                elements.extend(strings.into_iter().map(T::into));
                Ok(())
            }
            PrimitiveValue::Str(s) => {
                // for lack of better ways to move the string out from the mutable borrow,
                // we create a copy for now
                let s = s.clone();
                *self = PrimitiveValue::Strs(
                    std::iter::once(s)
                        .chain(strings.into_iter().map(T::into))
                        .collect(),
                );
                Ok(())
            }
            PrimitiveValue::Tags(_)
            | PrimitiveValue::U8(_)
            | PrimitiveValue::I16(_)
            | PrimitiveValue::U16(_)
            | PrimitiveValue::I32(_)
            | PrimitiveValue::U32(_)
            | PrimitiveValue::I64(_)
            | PrimitiveValue::U64(_)
            | PrimitiveValue::F32(_)
            | PrimitiveValue::F64(_)
            | PrimitiveValue::Date(_)
            | PrimitiveValue::DateTime(_)
            | PrimitiveValue::Time(_) => IncompatibleStringTypeSnafu {
                original: self.value_type(),
            }
            .fail(),
        }
    }

    /// Shorten this value by removing trailing elements
    /// to fit the given limit.
    ///
    /// Elements are counted by the number of individual value items
    /// (note that bytes in a [`U8`]
    /// are treated as individual items).
    ///
    /// Nothing is done if the value's cardinality
    /// is already lower than or equal to the limit.
    ///
    /// [`U8`]: Self::U8
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::dicom_value;
    /// # use dicom_core::value::PrimitiveValue;
    /// let mut value = dicom_value!(I32, [1, 2, 5]);
    /// value.truncate(2);
    /// assert_eq!(value.to_multi_int::<i32>()?, vec![1, 2]);
    ///
    /// value.truncate(0);
    /// assert_eq!(value.multiplicity(), 0);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn truncate(&mut self, limit: usize) {
        match self {
            PrimitiveValue::Empty | PrimitiveValue::Str(_) => { /* no-op */ }
            PrimitiveValue::Strs(l) => l.truncate(limit),
            PrimitiveValue::Tags(l) => l.truncate(limit),
            PrimitiveValue::U8(l) => l.truncate(limit),
            PrimitiveValue::I16(l) => l.truncate(limit),
            PrimitiveValue::U16(l) => l.truncate(limit),
            PrimitiveValue::I32(l) => l.truncate(limit),
            PrimitiveValue::U32(l) => l.truncate(limit),
            PrimitiveValue::I64(l) => l.truncate(limit),
            PrimitiveValue::U64(l) => l.truncate(limit),
            PrimitiveValue::F32(l) => l.truncate(limit),
            PrimitiveValue::F64(l) => l.truncate(limit),
            PrimitiveValue::Date(l) => l.truncate(limit),
            PrimitiveValue::DateTime(l) => l.truncate(limit),
            PrimitiveValue::Time(l) => l.truncate(limit),
        }
    }
}

/// The output of this method is equivalent to calling the method [`to_str`]
///
/// [`to_str`]: Self::to_str
impl Display for PrimitiveValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        /// Auxiliary function for turning a sequence of values
        /// into a backslash-delimited string.
        fn seq_to_str<I>(iter: I) -> String
        where
            I: IntoIterator,
            I::Item: Display,
        {
            iter.into_iter().map(|x| x.to_string()).join("\\")
        }

        match self {
            PrimitiveValue::Empty => Ok(()),
            PrimitiveValue::Str(_) => f.write_str(&self.to_str()),
            PrimitiveValue::Strs(_) => f.write_str(&self.to_str()),
            PrimitiveValue::Date(values) => {
                f.write_str(&values.into_iter().map(|date| date.to_string()).join("\\"))
            }
            PrimitiveValue::Time(values) => {
                f.write_str(&values.into_iter().map(|time| time.to_string()).join("\\"))
            }
            PrimitiveValue::DateTime(values) => f.write_str(
                &values
                    .into_iter()
                    .map(|datetime| datetime.to_string())
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

impl PartialEq for PrimitiveValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PrimitiveValue::Empty, PrimitiveValue::Empty) => true,
            (PrimitiveValue::Strs(v1), PrimitiveValue::Str(_)) => {
                v1.len() == 1 && self.to_str() == other.to_str()
            }
            (PrimitiveValue::Str(_), PrimitiveValue::Strs(v2)) => {
                v2.len() == 1 && self.to_str() == other.to_str()
            }
            (PrimitiveValue::Strs(_), PrimitiveValue::Strs(_)) => self.to_str() == other.to_str(),
            (PrimitiveValue::Str(_), PrimitiveValue::Str(_)) => self.to_str() == other.to_str(),
            (PrimitiveValue::Tags(v1), PrimitiveValue::Tags(v2)) => v1 == v2,
            (PrimitiveValue::U8(v1), PrimitiveValue::U8(v2)) => v1 == v2,
            (PrimitiveValue::I16(v1), PrimitiveValue::I16(v2)) => v1 == v2,
            (PrimitiveValue::U16(v1), PrimitiveValue::U16(v2)) => v1 == v2,
            (PrimitiveValue::I32(v1), PrimitiveValue::I32(v2)) => v1 == v2,
            (PrimitiveValue::U32(v1), PrimitiveValue::U32(v2)) => v1 == v2,
            (PrimitiveValue::I64(v1), PrimitiveValue::I64(v2)) => v1 == v2,
            (PrimitiveValue::U64(v1), PrimitiveValue::U64(v2)) => v1 == v2,
            (PrimitiveValue::F32(v1), PrimitiveValue::F32(v2)) => v1 == v2,
            (PrimitiveValue::F64(v1), PrimitiveValue::F64(v2)) => v1 == v2,
            (PrimitiveValue::Date(v1), PrimitiveValue::Date(v2)) => v1 == v2,
            (PrimitiveValue::DateTime(v1), PrimitiveValue::DateTime(v2)) => v1 == v2,
            (PrimitiveValue::Time(v1), PrimitiveValue::Time(v2)) => v1 == v2,
            _ => false,
        }
    }
}

impl PartialEq<str> for PrimitiveValue {
    fn eq(&self, other: &str) -> bool {
        match self {
            PrimitiveValue::Strs(v) => v.len() == 1 && v[0] == other,
            PrimitiveValue::Str(v) => v == other,
            _ => false,
        }
    }
}

impl PartialEq<&str> for PrimitiveValue {
    fn eq(&self, other: &&str) -> bool {
        self.eq(*other)
    }
}

/// An enum representing an abstraction of a DICOM element's data value type.
/// This should be the equivalent of [`PrimitiveValue`] without the content,
/// plus the [`DataSetSequence`] and [`PixelSequence`] entries.
///
/// [`DataSetSequence`]: Self::DataSetSequence
/// [`PixelSequence`]: Self::PixelSequence
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueType {
    /// No data. Used for any value of length 0.
    Empty,

    /// A data set sequence.
    /// Used for values with the SQ representation when not empty.
    DataSetSequence,

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

    /// The value is a sequence of unsigned 8-bit integers.
    /// Used for OB and UN.
    U8,

    /// The value is a sequence of signed 16-bit integers.
    /// Used for SS.
    I16,

    /// A sequence of unsigned 16-bit integers.
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

fn trim_last_whitespace(x: &[u8]) -> &[u8] {
    match x.last() {
        Some(b' ') | Some(b'\0') => &x[..x.len() - 1],
        _ => x,
    }
}

#[inline]
fn whitespace_or_null(c: char) -> bool {
    c.is_whitespace() || c == '\0'
}

#[cfg(test)]
mod tests {
    use super::CastValueError;
    use crate::dicom_value;
    use crate::value::partial::{DicomDate, DicomDateTime, DicomTime};
    use crate::value::{PrimitiveValue, ValueType};
    use chrono::FixedOffset;
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
            PrimitiveValue::Date(smallvec![DicomDate::from_ymd(2014, 10, 12).unwrap()]).to_str(),
            "2014-10-12",
        );
        assert_eq!(
            dicom_value!(Strs, ["DERIVED", "PRIMARY", "WHOLE BODY", "EMISSION"]).to_str(),
            "DERIVED\\PRIMARY\\WHOLE BODY\\EMISSION",
        );

        // sequence of numbers
        let value = PrimitiveValue::from(vec![10, 11, 12]);
        assert_eq!(value.to_str(), "10\\11\\12",);

        // now test that trailing whitespace is trimmed
        // removes whitespace at the end of a string
        let value = PrimitiveValue::from("1.2.345\0".to_string());
        assert_eq!(&value.to_str(), "1.2.345");
        let value = PrimitiveValue::from("1.2.345 ".to_string());
        assert_eq!(&value.to_str(), "1.2.345");

        // removes whitespace at the end on multiple strings
        let value = dicom_value!(Strs, ["ONE ", "TWO", "THREE", "SIX "]);
        assert_eq!(&value.to_str(), "ONE\\TWO\\THREE\\SIX");

        // maintains the leading whitespace on a string and removes at the end
        let value = PrimitiveValue::from("\x001.2.345\0".to_string());
        assert_eq!(&value.to_str(), "\x001.2.345");

        // maintains the leading whitespace on multiple strings and removes at the end
        let value = dicom_value!(Strs, [" ONE", "TWO", "THREE", " SIX "]);
        assert_eq!(&value.to_str(), " ONE\\TWO\\THREE\\ SIX");
    }

    #[test]
    fn primitive_value_to_raw_str() {
        // maintains whitespace at the end of a string
        let value = PrimitiveValue::from("1.2.345\0".to_string());
        assert_eq!(&value.to_raw_str(), "1.2.345\0");

        // maintains whitespace at the end on multiple strings
        let value = dicom_value!(Strs, ["ONE", "TWO", "THREE", "SIX "]);
        assert_eq!(&value.to_raw_str(), "ONE\\TWO\\THREE\\SIX ");

        // maintains the leading whitespace on a string and maintains at the end
        let value = PrimitiveValue::from("\x001.2.345\0".to_string());
        assert_eq!(&value.to_raw_str(), "\x001.2.345\0");

        // maintains the leading whitespace on multiple strings and maintains at the end
        let value = dicom_value!(Strs, [" ONE", "TWO", "THREE", " SIX "]);
        assert_eq!(&value.to_raw_str(), " ONE\\TWO\\THREE\\ SIX ");
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
            PrimitiveValue::Date(smallvec![DicomDate::from_ym(2014, 10).unwrap()]).to_bytes(),
            &b"2014-10"[..],
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
        // b"20141012\\202009\\20180101"
        let val = dicom_value!(
            Date,
            [
                DicomDate::from_ymd(2014, 10, 12).unwrap(),
                DicomDate::from_ym(2020, 9).unwrap(),
                DicomDate::from_ymd(2018, 1, 1).unwrap()
            ]
        );
        assert_eq!(val.calculate_byte_len(), 24);

        // multi date with padding
        // b"20141012\\2020 "
        let val = dicom_value!(
            Date,
            [
                DicomDate::from_ymd(2014, 10, 12).unwrap(),
                DicomDate::from_y(2020).unwrap()
            ]
        );
        assert_eq!(val.calculate_byte_len(), 14);

        // single time with second fragment - full precision
        // b"185530.475600 "
        let val = dicom_value!(DicomTime::from_hms_micro(18, 55, 30, 475_600).unwrap());
        assert_eq!(val.calculate_byte_len(), 14);

        // multi time with padding
        // b"185530\\185530 "
        let val = dicom_value!(
            Time,
            [
                DicomTime::from_hms(18, 55, 30).unwrap(),
                DicomTime::from_hms(18, 55, 30).unwrap()
            ]
        );
        assert_eq!(val.calculate_byte_len(), 14);

        // single date-time with time zone, no second fragment
        // b"20121221093001+0100 "
        let offset = FixedOffset::east_opt(3600).unwrap();
        let val = PrimitiveValue::from(
            DicomDateTime::from_date_and_time_with_time_zone(
                DicomDate::from_ymd(2012, 12, 21).unwrap(),
                DicomTime::from_hms(9, 30, 1).unwrap(),
                offset,
            )
            .unwrap(),
        );
        assert_eq!(val.calculate_byte_len(), 20);

        // single date-time without time zone, no second fragment
        // b"20121221093001 "
        let val = PrimitiveValue::from(
            DicomDateTime::from_date_and_time(
                DicomDate::from_ymd(2012, 12, 21).unwrap(),
                DicomTime::from_hms(9, 30, 1).unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(val.calculate_byte_len(), 14);

        // very precise date time, 0 microseconds
        let dicom_date_time = DicomDateTime::from_date_and_time_with_time_zone(
            DicomDate::from_ymd(2024, 8, 26).unwrap(),
            DicomTime::from_hms_micro(19, 41, 38, 0).unwrap(),
            FixedOffset::west_opt(0).unwrap(),
        )
        .unwrap();
        let val = PrimitiveValue::from(dicom_date_time);
        assert_eq!(val.calculate_byte_len(), 26);
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
            PrimitiveValue::Date(smallvec![DicomDate::from_ymd(2014, 10, 12).unwrap()])
                .date()
                .unwrap(),
            DicomDate::from_ymd(2014, 10, 12).unwrap(),
        );

        assert!(matches!(
            PrimitiveValue::Date(smallvec![DicomDate::from_ymd(2014, 10, 12).unwrap()]).time(),
            Err(CastValueError {
                requested: "time",
                got: ValueType::Date,
                ..
            })
        ));
    }

    /// Expect Str to be comparable to 1-element Strs.
    #[test]
    fn eq_ignores_multi_variants() {
        assert_eq!(dicom_value!(Str, "abc123"), dicom_value!(Strs, ["abc123"]),);

        assert_eq!(dicom_value!(Strs, ["abc123"]), dicom_value!(Str, "abc123"),);

        assert_eq!(dicom_value!(Str, "ABC123"), PrimitiveValue::from("ABC123"),);

        assert_eq!(dicom_value!(Str, ""), PrimitiveValue::from(""),);
    }

    #[test]
    fn eq_str() {
        assert_eq!(PrimitiveValue::from("Doe^John"), "Doe^John");
        assert_eq!(dicom_value!(Strs, ["Doe^John"]), "Doe^John");
        assert_eq!(PrimitiveValue::from("Doe^John"), &*"Doe^John".to_owned());

        assert_ne!(dicom_value!(Strs, ["Doe^John", "Silva^João"]), "Doe^John");
    }
}
