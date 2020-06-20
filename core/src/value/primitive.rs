//! Declaration and implementation of a DICOM primitive value.
//!
//! See [`PrimitiveValue`](./enum.PrimitiveValue.html).

use super::DicomValueType;
use crate::error::ConvertValueError;
use crate::header::{HasLength, Length, Tag};
use chrono::{Datelike, FixedOffset, Timelike};
use itertools::Itertools;
use safe_transmute::to_bytes::transmute_to_bytes;
use smallvec::SmallVec;
use std::borrow::Cow;

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
    /// even for the string variants,
    /// As
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
    /// For value which are inherently textual as per the standard,
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
    ///     .to_date(),
    ///     Ok(NaiveDate::from_ymd(2014, 10, 12)),
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveValue::Strs(smallvec![
    ///         "20141012".to_string(),
    ///     ])
    ///     .to_date(),
    ///     Ok(NaiveDate::from_ymd(2014, 10, 12)),
    /// );
    /// ```
    pub fn to_date(&self) -> Result<NaiveDate, ConvertValueError> {
        match self {
            PrimitiveValue::Date(v) if !v.is_empty() => Ok(v[0]),
            PrimitiveValue::Str(s) => super::deserialize::parse_date(s.as_bytes())
                .map(|(date, _rest)| date)
                .map_err(|err| ConvertValueError {
                    requested: "Date",
                    original: self.value_type(),
                    cause: Some(err),
                }),
            PrimitiveValue::Strs(s) => {
                super::deserialize::parse_date(s.first().map(|s| s.as_bytes()).unwrap_or(&[]))
                    .map(|(date, _rest)| date)
                    .map_err(|err| ConvertValueError {
                        requested: "Date",
                        original: self.value_type(),
                        cause: Some(err),
                    })
            }
            PrimitiveValue::U8(bytes) => super::deserialize::parse_date(bytes)
                .map(|(date, _rest)| date)
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
    use crate::dicom_value;
    use crate::value::PrimitiveValue;
    use chrono::NaiveDate;
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
        // from bytes (Bytes fake variant)
        assert_eq!(
            dicom_value!(Bytes, b"20141012").to_date().unwrap(),
            NaiveDate::from_ymd(2014, 10, 12),
        );
        // not a date
        assert!(PrimitiveValue::Str("Smith^John".to_string())
            .to_date()
            .is_err());
    }
}
