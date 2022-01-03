//! This modules contains an assortment of types required for interpreting DICOM data elements.
//! It comprises a variety of basic data types, such as the DICOM attribute tag, the
//! element header, and element composite types.

use crate::value::{
    CastValueError, ConvertValueError, DicomDate, DicomDateTime, DicomTime, PrimitiveValue, Value,
};
use chrono::FixedOffset;
use num_traits::NumCast;
use snafu::{Backtrace, Snafu};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::str::{from_utf8, FromStr};

/// Error type for issues constructing a sequence item header.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum SequenceItemHeaderError {
    /// Unexpected header tag.
    /// Only Item (0xFFFE, 0xE000),
    /// Item Delimiter (0xFFFE, 0xE00D),
    /// or Sequence Delimiter (0xFFFE, 0xE0DD)
    /// are admitted.
    #[snafu(display("Unexpected tag {}", tag))]
    UnexpectedTag { tag: Tag, backtrace: Backtrace },
    /// Unexpected delimiter value length.
    /// Must be zero for item delimiters.
    #[snafu(display("Unexpected delimiter length {}", len))]
    UnexpectedDelimiterLength { len: Length, backtrace: Backtrace },
}

type Result<T, E = SequenceItemHeaderError> = std::result::Result<T, E>;

/// Trait for any DICOM entity (element or item) which may have a length.
pub trait HasLength {
    /// Retrieve the value data's length as specified by the data element or
    /// item, in bytes.
    ///
    /// It is named `length` to make it distinct from the conventional method
    /// signature `len(&self) -> usize` for the number of elements of a
    /// collection.
    ///
    /// According to the standard, the concrete value size may be undefined,
    /// which can be the case for sequence elements or specific primitive
    /// values.
    fn length(&self) -> Length;

    /// Check whether the value is empty (0 length).
    fn is_empty(&self) -> bool {
        self.length() == Length(0)
    }
}

/// A trait for a data type containing a DICOM header.
#[allow(clippy::len_without_is_empty)]
pub trait Header: HasLength {
    /// Retrieve the element's tag as a `(group, element)` tuple.
    fn tag(&self) -> Tag;

    /// Check whether this is the header of an item.
    fn is_item(&self) -> bool {
        self.tag() == Tag(0xFFFE, 0xE000)
    }

    /// Check whether this is the header of an item delimiter.
    fn is_item_delimiter(&self) -> bool {
        self.tag() == Tag(0xFFFE, 0xE00D)
    }

    /// Check whether this is the header of a sequence delimiter.
    fn is_sequence_delimiter(&self) -> bool {
        self.tag() == Tag(0xFFFE, 0xE0DD)
    }

    /// Check whether this is the header of an encapsulated pixel data.
    fn is_encapsulated_pixeldata(&self) -> bool {
        self.tag() == Tag(0x7FE0, 0x0010) && self.length().is_undefined()
    }
}

/// Stub type representing a non-existing DICOM object.
///
/// This type implements `HasLength`, but cannot be instantiated.
/// This makes it so that `Value<EmptyObject>` is sure to be either a primitive
/// value or a sequence with no items.
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub enum EmptyObject {}

impl HasLength for EmptyObject {
    fn length(&self) -> Length {
        unreachable!()
    }
}

/// A data type that represents and owns a DICOM data element.
///
/// This type is capable of representing any data element fully in memory,
/// whether it be a primitive value,
/// a nested data set (where each item contains an object of type `I`),
/// or an encapsulated pixel data sequence (each item of type `P`).
#[derive(Debug, PartialEq, Clone)]
pub struct DataElement<I = EmptyObject, P = [u8; 0]> {
    header: DataElementHeader,
    value: Value<I, P>,
}

/// A data type that represents and owns a DICOM data element
/// containing a primitive value.
#[derive(Debug, PartialEq, Clone)]
pub struct PrimitiveDataElement {
    header: DataElementHeader,
    value: PrimitiveValue,
}

impl PrimitiveDataElement {
    /// Main constructor for a primitive data element.
    pub fn new(header: DataElementHeader, value: PrimitiveValue) -> Self {
        PrimitiveDataElement { header, value }
    }
}

impl<I, P> From<PrimitiveDataElement> for DataElement<I, P> {
    fn from(o: PrimitiveDataElement) -> Self {
        DataElement {
            header: o.header,
            value: o.value.into(),
        }
    }
}

/// A data type that represents a DICOM data element with
/// a borrowed value.
#[derive(Debug, PartialEq, Clone)]
pub struct DataElementRef<'v, I: 'v, P: 'v> {
    header: DataElementHeader,
    value: &'v Value<I, P>,
}

/// A data type that represents a DICOM data element with
/// a borrowed primitive value.
#[derive(Debug, PartialEq, Clone)]
pub struct PrimitiveDataElementRef<'v> {
    header: DataElementHeader,
    value: &'v PrimitiveValue,
}

impl<'a> PrimitiveDataElementRef<'a> {
    /// Main constructor for a primitive data element reference.
    pub fn new(header: DataElementHeader, value: &'a PrimitiveValue) -> Self {
        PrimitiveDataElementRef { header, value }
    }
}
impl<I, P> HasLength for DataElement<I, P> {
    #[inline]
    fn length(&self) -> Length {
        self.header.length()
    }
}

impl<I, P> Header for DataElement<I, P> {
    #[inline]
    fn tag(&self) -> Tag {
        self.header.tag()
    }
}

impl<I, P> HasLength for &DataElement<I, P> {
    #[inline]
    fn length(&self) -> Length {
        (**self).length()
    }
}

impl<'a, I, P> Header for &'a DataElement<I, P> {
    #[inline]
    fn tag(&self) -> Tag {
        (**self).tag()
    }
}

impl<'v, I, P> HasLength for DataElementRef<'v, I, P> {
    #[inline]
    fn length(&self) -> Length {
        self.header.length()
    }
}

impl<'v, I, P> Header for DataElementRef<'v, I, P> {
    #[inline]
    fn tag(&self) -> Tag {
        self.header.tag()
    }
}

impl<I, P> DataElement<I, P> {
    /// Create an empty data element.
    pub fn empty(tag: Tag, vr: VR) -> Self {
        DataElement {
            header: DataElementHeader {
                tag,
                vr,
                len: Length(0),
            },
            value: PrimitiveValue::Empty.into(),
        }
    }

    /// Retrieve the element header.
    pub fn header(&self) -> &DataElementHeader {
        &self.header
    }

    /// Retrieve the value representation, which may be unknown or not
    /// applicable.
    pub fn vr(&self) -> VR {
        self.header.vr()
    }

    /// Retrieve the data value.
    pub fn value(&self) -> &Value<I, P> {
        &self.value
    }

    /// Move the data value out of the element, discarding the rest. If the
    /// value is a sequence, its lifetime may still be bound to its original
    /// source.
    pub fn into_value(self) -> Value<I, P> {
        self.value
    }
}

impl<I, P> DataElement<I, P>
where
    I: HasLength,
{
    /// Create a primitive data element from the given parts,
    /// where the length is inferred from the value's byte length.
    ///
    /// If the value is textual,
    /// the byte length of that value encoded in UTF-8 is assumed.
    /// If you already have a length in this context,
    /// prefer calling `new_with_len` instead.
    ///
    /// This method will not check whether the value representation is
    /// compatible with the given value.
    pub fn new<T>(tag: Tag, vr: VR, value: T) -> Self
    where
        T: Into<Value<I, P>>,
    {
        let value = value.into();
        DataElement {
            header: DataElementHeader {
                tag,
                vr,
                len: value.length(),
            },
            value,
        }
    }

    /// Create a primitive data element from the given parts.
    ///
    /// This method will not check
    /// whether the length accurately represents the given value's byte length,
    /// nor whether the value representation is compatible with the value.
    pub fn new_with_len<T>(tag: Tag, vr: VR, length: Length, value: T) -> Self
    where
        T: Into<Value<I, P>>,
    {
        let value = value.into();
        DataElement {
            header: DataElementHeader {
                tag,
                vr,
                len: length,
            },
            value,
        }
    }

    /// Retrieve the element's value as a single clean string,
    /// with no trailing whitespace.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_str(&self) -> Result<Cow<str>, CastValueError> {
        self.value.to_str()
    }

    /// Retrieve the element's value as a single raw string,
    /// with trailing whitespace kept.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_raw_str(&self) -> Result<Cow<str>, CastValueError> {
        self.value.to_raw_str()
    }

    /// Retrieves the element's value as a clean string
    #[deprecated(
        note = "`to_clean_str()` is now deprecated in favour of using `to_str()` directly. 
        `to_raw_str()` replaces the old functionality of `to_str()` and maintains all trailing whitespace."
    )]
    pub fn to_clean_str(&self) -> Result<Cow<str>, CastValueError> {
        self.value.to_str()
    }

    /// Convert the full primitive value into raw bytes.
    ///
    /// String values already encoded with the `Str` and `Strs` variants
    /// are provided in UTF-8.
    ///
    /// Returns an error if the value is not primitive.
    pub fn to_bytes(&self) -> Result<Cow<[u8]>, CastValueError> {
        self.value().to_bytes()
    }

    /// Convert the full value of the data element into a sequence of strings.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of strings as described in [`PrimitiveValue::to_multi_str`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_multi_str`]: ../enum.PrimitiveValue.html#to_multi_str
    pub fn to_multi_str(&self) -> Result<Cow<[String]>, CastValueError> {
        self.value().to_multi_str()
    }

    /// Retrieve and convert the value of the data element into an integer.
    ///
    /// If the value is a primitive,
    /// it will be converted into an integer
    /// as described in [`PrimitiveValue::to_int`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_int`]: ../enum.PrimitiveValue.html#to_int
    pub fn to_int<T>(&self) -> Result<T, ConvertValueError>
    where
        T: Clone,
        T: NumCast,
        T: FromStr<Err = std::num::ParseIntError>,
    {
        self.value().to_int()
    }

    /// Retrieve and convert the value of the data element
    /// into a sequence of integers.
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
        self.value().to_multi_int()
    }

    /// Retrieve and convert the value of the data element
    /// into a single-precision floating point number.
    ///
    /// If the value is a primitive, it will be converted into
    /// a number as described in [`PrimitiveValue::to_float32`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_float32`]: ../enum.PrimitiveValue.html#to_float32
    pub fn to_float32(&self) -> Result<f32, ConvertValueError> {
        self.value().to_float32()
    }

    /// Retrieve and convert the value of the data element
    /// into a sequence of single-precision floating point numbers.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of numbers as described in [`PrimitiveValue::to_multi_float32`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_multi_float32`]: ../enum.PrimitiveValue.html#to_multi_float32
    pub fn to_multi_float32(&self) -> Result<Vec<f32>, ConvertValueError> {
        self.value().to_multi_float32()
    }

    /// Retrieve and convert the value of the data element
    /// into a double-precision floating point number.
    ///
    /// If the value is a primitive, it will be converted into
    /// a number as described in [`PrimitiveValue::to_float64`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_float64`]: ../enum.PrimitiveValue.html#to_float64
    pub fn to_float64(&self) -> Result<f64, ConvertValueError> {
        self.value().to_float64()
    }

    /// Retrieve and convert the value of the data element
    /// into a sequence of double-precision floating point numbers.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of numbers as described in [`PrimitiveValue::to_multi_float64`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    /// [`PrimitiveValue::to_multi_float64`]: ../enum.PrimitiveValue.html#to_multi_float64
    pub fn to_multi_float64(&self) -> Result<Vec<f64>, ConvertValueError> {
        self.value().to_multi_float64()
    }

    /// Retrieve and convert the primitive value into a date.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DicomDate` as described in [`PrimitiveValue::to_date`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    pub fn to_date(&self) -> Result<DicomDate, ConvertValueError> {
        self.value().to_date()
    }

    /// Retrieve and convert the primitive value into a sequence of dates.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `DicomDate` as described in [`PrimitiveValue::to_multi_date`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    pub fn to_multi_date(&self) -> Result<Vec<DicomDate>, ConvertValueError> {
        self.value().to_multi_date()
    }

    /// Retrieve and convert the primitive value into a time.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DicomTime` as described in [`PrimitiveValue::to_time`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    pub fn to_time(&self) -> Result<DicomTime, ConvertValueError> {
        self.value().to_time()
    }

    /// Retrieve and convert the primitive value into a sequence of times.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `DicomTime` as described in [`PrimitiveValue::to_multi_time`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    pub fn to_multi_time(&self) -> Result<Vec<DicomTime>, ConvertValueError> {
        self.value().to_multi_time()
    }

    /// Retrieve and convert the primitive value into a date-time.
    ///
    /// If the value is a primitive, it will be converted into
    /// a `DicomDateTime` as described in [`PrimitiveValue::to_datetime`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    pub fn to_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<DicomDateTime, ConvertValueError> {
        self.value().to_datetime(default_offset)
    }

    /// Retrieve and convert the primitive value into a sequence of date-times.
    ///
    /// If the value is a primitive, it will be converted into
    /// a vector of `DicomDateTime` as described in [`PrimitiveValue::to_multi_datetime`].
    ///
    /// Returns an error if the value is not primitive.
    ///
    pub fn to_multi_datetime(
        &self,
        default_offset: FixedOffset,
    ) -> Result<Vec<DicomDateTime>, ConvertValueError> {
        self.value().to_multi_datetime(default_offset)
    }
}

impl<'v, I, P> DataElementRef<'v, I, P>
where
    I: HasLength,
{
    /// Create a data element from the given parts. This method will not check
    /// whether the value representation is compatible with the value. Caution
    /// is advised.
    pub fn new(tag: Tag, vr: VR, value: &'v Value<I, P>) -> Self {
        DataElementRef {
            header: DataElementHeader {
                tag,
                vr,
                len: value.length(),
            },
            value,
        }
    }

    /// Retrieves the element's value representation, which can be unknown.
    pub fn vr(&self) -> VR {
        self.header.vr()
    }

    /// Retrieves the DICOM value.
    pub fn value(&self) -> &Value<I, P> {
        self.value
    }
}

/// Macro for implementing getters to single and multi-values,
/// by delegating to `Value`.
///
/// Should be placed inside `DataElement`'s impl block.
macro_rules! impl_primitive_getters {
    ($name_single: ident, $name_multi: ident, $variant: ident, $ret: ty) => {
        /// Get a single value of the requested type.
        ///
        /// If it contains multiple values,
        /// only the first one is returned.
        /// An error is returned if the variant is not compatible.
        pub fn $name_single(&self) -> Result<$ret, CastValueError> {
            self.value().$name_single()
        }

        /// Get a sequence of values of the requested type without copying.
        ///
        /// An error is returned if the variant is not compatible.
        pub fn $name_multi(&self) -> Result<&[$ret], CastValueError> {
            self.value().$name_multi()
        }
    };
}

impl<I, P> DataElement<I, P> {
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
        self.value().string()
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
        self.value().strings()
    }

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

/// A data structure for a data element header, containing
/// a tag, value representation and specified length.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct DataElementHeader {
    /// DICOM tag
    pub tag: Tag,
    /// Value Representation
    pub vr: VR,
    /// Element length
    pub len: Length,
}

impl HasLength for DataElementHeader {
    #[inline]
    fn length(&self) -> Length {
        self.len
    }
}

impl Header for DataElementHeader {
    #[inline]
    fn tag(&self) -> Tag {
        self.tag
    }
}

impl DataElementHeader {
    /// Create a new data element header with the given properties.
    /// This is just a trivial constructor.
    #[inline]
    pub fn new<T: Into<Tag>>(tag: T, vr: VR, len: Length) -> DataElementHeader {
        DataElementHeader {
            tag: tag.into(),
            vr,
            len,
        }
    }

    /// Retrieve the element's value representation, which can be unknown.
    #[inline]
    pub fn vr(&self) -> VR {
        self.vr
    }

    /// Check whether the header suggests the value to be a sequence value:
    /// if the value representation is SQ or the length is undefined.
    #[inline]
    pub fn is_non_primitive(&self) -> bool {
        self.vr == VR::SQ || self.length().is_undefined()
    }
}

impl From<SequenceItemHeader> for DataElementHeader {
    fn from(value: SequenceItemHeader) -> DataElementHeader {
        DataElementHeader {
            tag: value.tag(),
            vr: VR::UN,
            len: value.length(),
        }
    }
}

/// Data type for describing a sequence item data element.
/// If the element represents an item, it will also contain
/// the specified length.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SequenceItemHeader {
    /// The cursor contains an item.
    Item {
        /// the length of the item in bytes (can be 0xFFFFFFFF if undefined)
        len: Length,
    },
    /// The cursor read an item delimiter.
    /// The element ends here and should not be read any further.
    ItemDelimiter,
    /// The cursor read a sequence delimiter.
    /// The element ends here and should not be read any further.
    SequenceDelimiter,
}

impl SequenceItemHeader {
    /// Create a sequence item header using the element's raw properties.
    /// An error can be raised if the given properties do not relate to a
    /// sequence item, a sequence item delimiter or a sequence delimiter.
    pub fn new<T: Into<Tag>>(tag: T, len: Length) -> Result<SequenceItemHeader> {
        match tag.into() {
            Tag(0xFFFE, 0xE000) => {
                // item
                Ok(SequenceItemHeader::Item { len })
            }
            Tag(0xFFFE, 0xE00D) => {
                // item delimiter
                // delimiters should not have a positive length
                if len != Length(0) {
                    UnexpectedDelimiterLength { len }.fail()
                } else {
                    Ok(SequenceItemHeader::ItemDelimiter)
                }
            }
            Tag(0xFFFE, 0xE0DD) => {
                // sequence delimiter
                Ok(SequenceItemHeader::SequenceDelimiter)
            }
            tag => UnexpectedTag { tag }.fail(),
        }
    }
}

impl HasLength for SequenceItemHeader {
    #[inline]
    fn length(&self) -> Length {
        match *self {
            SequenceItemHeader::Item { len } => len,
            SequenceItemHeader::ItemDelimiter | SequenceItemHeader::SequenceDelimiter => Length(0),
        }
    }
}
impl Header for SequenceItemHeader {
    #[inline]
    fn tag(&self) -> Tag {
        match *self {
            SequenceItemHeader::Item { .. } => Tag(0xFFFE, 0xE000),
            SequenceItemHeader::ItemDelimiter => Tag(0xFFFE, 0xE00D),
            SequenceItemHeader::SequenceDelimiter => Tag(0xFFFE, 0xE0DD),
        }
    }
}

/// An enum type for a DICOM value representation.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone, Ord, PartialOrd)]
pub enum VR {
    /// Application Entity
    AE,
    /// Age String
    AS,
    /// Attribute Tag
    AT,
    /// Code String
    CS,
    /// Date
    DA,
    /// Decimal String
    DS,
    /// Date Time
    DT,
    /// Floating Point Single
    FL,
    /// Floating Point Double
    FD,
    /// Integer String
    IS,
    /// Long String
    LO,
    /// Long Text
    LT,
    /// Other Byte
    OB,
    /// Other Double
    OD,
    /// Other Float
    OF,
    /// Other Long
    OL,
    /// Other Very Long
    OV,
    /// Other Word
    OW,
    /// Person Name
    PN,
    /// Short String
    SH,
    /// Signed Long
    SL,
    /// Sequence of Items
    SQ,
    /// Signed Short
    SS,
    /// Short Text
    ST,
    /// Time
    SV,
    /// Signed Very Long
    TM,
    /// Unlimited Characters
    UC,
    /// Unique Identifier (UID)
    UI,
    /// Unsigned Long
    UL,
    /// Unknown
    UN,
    /// Universal Resource Identifier or Universal Resource Locator (URI/URL)
    UR,
    /// Unsigned Short
    US,
    /// Unlimited Text
    UT,
    /// Unsigned Very Long
    UV,
}

impl VR {
    /// Obtain the value representation corresponding to the given two bytes.
    /// Each byte should represent an alphabetic character in upper case.
    pub fn from_binary(chars: [u8; 2]) -> Option<Self> {
        from_utf8(chars.as_ref())
            .ok()
            .and_then(|s| VR::from_str(s).ok())
    }

    /// Retrieve a string representation of this VR.
    pub fn to_string(self) -> &'static str {
        use VR::*;
        match self {
            AE => "AE",
            AS => "AS",
            AT => "AT",
            CS => "CS",
            DA => "DA",
            DS => "DS",
            DT => "DT",
            FL => "FL",
            FD => "FD",
            IS => "IS",
            LO => "LO",
            LT => "LT",
            OB => "OB",
            OD => "OD",
            OF => "OF",
            OL => "OL",
            OV => "OV",
            OW => "OW",
            PN => "PN",
            SH => "SH",
            SL => "SL",
            SQ => "SQ",
            SS => "SS",
            ST => "ST",
            SV => "SV",
            TM => "TM",
            UC => "UC",
            UI => "UI",
            UL => "UL",
            UN => "UN",
            UR => "UR",
            US => "US",
            UT => "UT",
            UV => "UV",
        }
    }

    /// Retrieve a copy of this VR's byte representation.
    /// The function returns two alphabetic characters in upper case.
    pub fn to_bytes(self) -> [u8; 2] {
        let bytes = self.to_string().as_bytes();
        [bytes[0], bytes[1]]
    }
}

/// Obtain the value representation corresponding to the given string.
/// The string should hold exactly two UTF-8 encoded alphabetic characters
/// in upper case, otherwise no match is made.
impl FromStr for VR {
    type Err = &'static str;

    fn from_str(string: &str) -> std::result::Result<Self, Self::Err> {
        use VR::*;
        match string {
            "AE" => Ok(AE),
            "AS" => Ok(AS),
            "AT" => Ok(AT),
            "CS" => Ok(CS),
            "DA" => Ok(DA),
            "DS" => Ok(DS),
            "DT" => Ok(DT),
            "FL" => Ok(FL),
            "FD" => Ok(FD),
            "IS" => Ok(IS),
            "LO" => Ok(LO),
            "LT" => Ok(LT),
            "OB" => Ok(OB),
            "OD" => Ok(OD),
            "OF" => Ok(OF),
            "OL" => Ok(OL),
            "OV" => Ok(OV),
            "OW" => Ok(OW),
            "PN" => Ok(PN),
            "SH" => Ok(SH),
            "SL" => Ok(SL),
            "SQ" => Ok(SQ),
            "SS" => Ok(SS),
            "ST" => Ok(ST),
            "SV" => Ok(SV),
            "TM" => Ok(TM),
            "UC" => Ok(UC),
            "UI" => Ok(UI),
            "UL" => Ok(UL),
            "UN" => Ok(UN),
            "UR" => Ok(UR),
            "US" => Ok(US),
            "UT" => Ok(UT),
            "UV" => Ok(UV),
            _ => Err("no such value representation"),
        }
    }
}

impl fmt::Display for VR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(VR::to_string(*self))
    }
}

/// Idiomatic alias for a tag's group number.
pub type GroupNumber = u16;
/// Idiomatic alias for a tag's element number.
pub type ElementNumber = u16;

/// The data type for DICOM data element tags.
///
/// Since  types will not have a monomorphized tag, and so will only support
/// a (group, element) pair. For this purpose, `Tag` also provides a method
/// for converting it to a tuple. Both `(u16, u16)` and `[u16; 2]` can be
/// efficiently converted to this type as well.
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub struct Tag(pub GroupNumber, pub ElementNumber);

impl Tag {
    /// Getter for the tag's group value.
    #[inline]
    pub fn group(self) -> GroupNumber {
        self.0
    }

    /// Getter for the tag's element value.
    #[inline]
    pub fn element(self) -> ElementNumber {
        self.1
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tag({:#06X?}, {:#06X?})", self.0, self.1)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:04X},{:04X})", self.0, self.1)
    }
}

impl PartialEq<(u16, u16)> for Tag {
    fn eq(&self, other: &(u16, u16)) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl PartialEq<[u16; 2]> for Tag {
    fn eq(&self, other: &[u16; 2]) -> bool {
        self.0 == other[0] && self.1 == other[1]
    }
}

impl From<(u16, u16)> for Tag {
    #[inline]
    fn from(value: (u16, u16)) -> Tag {
        Tag(value.0, value.1)
    }
}

impl From<[u16; 2]> for Tag {
    #[inline]
    fn from(value: [u16; 2]) -> Tag {
        Tag(value[0], value[1])
    }
}

/// A type for representing data set content length, in bytes.
/// An internal value of `0xFFFF_FFFF` represents an undefined
/// (unspecified) length, which would have to be determined
/// with a traversal based on the content's encoding.
///
/// This also means that numeric comparisons and arithmetic
/// do not function the same way as primitive number types:
///
/// Two length of undefined length are not equal.
///
/// ```
/// # use dicom_core::Length;
/// assert_ne!(Length::UNDEFINED, Length::UNDEFINED);
/// ```
///
/// Any addition or substraction with at least one undefined
/// length results in an undefined length.
///
/// ```
/// # use dicom_core::Length;
/// assert!((Length::defined(64) + Length::UNDEFINED).is_undefined());
/// assert!((Length::UNDEFINED + 8).is_undefined());
/// ```
///
/// Comparing between at least one undefined length is always `false`.
///
/// ```
/// # use dicom_core::Length;
/// assert!(Length::defined(16) < Length::defined(64));
/// assert!(!(Length::UNDEFINED < Length::defined(64)));
/// assert!(!(Length::UNDEFINED > Length::defined(64)));
///
/// assert!(!(Length::UNDEFINED < Length::UNDEFINED));
/// assert!(!(Length::UNDEFINED > Length::UNDEFINED));
/// assert!(!(Length::UNDEFINED <= Length::UNDEFINED));
/// assert!(!(Length::UNDEFINED >= Length::UNDEFINED));
/// ```
///
#[derive(Clone, Copy)]
pub struct Length(pub u32);

const UNDEFINED_LEN: u32 = 0xFFFF_FFFF;

impl Length {
    /// A length that is undefined.
    pub const UNDEFINED: Self = Length(UNDEFINED_LEN);

    /// Create a new length value from its internal representation.
    /// This is equivalent to `Length(len)`.
    #[inline]
    pub fn new(len: u32) -> Self {
        Length(len)
    }

    /// Create a new length value with the given number of bytes.
    ///
    /// # Panic
    ///
    /// This function will panic if `len` represents an undefined length.
    #[inline]
    pub fn defined(len: u32) -> Self {
        assert_ne!(len, UNDEFINED_LEN);
        Length(len)
    }
}

impl From<u32> for Length {
    #[inline]
    fn from(o: u32) -> Self {
        Length(o)
    }
}

impl PartialEq<Length> for Length {
    fn eq(&self, rhs: &Length) -> bool {
        match (self.0, rhs.0) {
            (UNDEFINED_LEN, _) | (_, UNDEFINED_LEN) => false,
            (l1, l2) => l1 == l2,
        }
    }
}

impl PartialOrd<Length> for Length {
    fn partial_cmp(&self, rhs: &Length) -> Option<Ordering> {
        match (self.0, rhs.0) {
            (UNDEFINED_LEN, _) | (_, UNDEFINED_LEN) => None,
            (l1, l2) => Some(l1.cmp(&l2)),
        }
    }
}

impl ::std::ops::Add<Length> for Length {
    type Output = Self;

    fn add(self, rhs: Length) -> Self::Output {
        match (self.0, rhs.0) {
            (UNDEFINED_LEN, _) | (_, UNDEFINED_LEN) => Length::UNDEFINED,
            (l1, l2) => {
                let o = l1 + l2;
                debug_assert!(
                    o != UNDEFINED_LEN,
                    "integer overflow (0xFFFF_FFFF reserved for undefined length)"
                );
                Length(o)
            }
        }
    }
}

impl ::std::ops::Add<i32> for Length {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        match self.0 {
            UNDEFINED_LEN => Length::UNDEFINED,
            len => {
                let o = (len as i32 + rhs) as u32;
                debug_assert!(
                    o != UNDEFINED_LEN,
                    "integer overflow (0xFFFF_FFFF reserved for undefined length)"
                );

                Length(o)
            }
        }
    }
}

impl std::ops::Sub<Length> for Length {
    type Output = Self;

    fn sub(self, rhs: Length) -> Self::Output {
        let mut o = self;
        o -= rhs;
        o
    }
}

impl std::ops::SubAssign<Length> for Length {
    fn sub_assign(&mut self, rhs: Length) {
        match (self.0, rhs.0) {
            (UNDEFINED_LEN, _) | (_, UNDEFINED_LEN) => (), // no-op
            (_, l2) => {
                self.0 -= l2;
                debug_assert!(
                    self.0 != UNDEFINED_LEN,
                    "integer overflow (0xFFFF_FFFF reserved for undefined length)"
                );
            }
        }
    }
}

impl std::ops::Sub<i32> for Length {
    type Output = Self;

    fn sub(self, rhs: i32) -> Self::Output {
        let mut o = self;
        o -= rhs;
        o
    }
}

impl std::ops::SubAssign<i32> for Length {
    fn sub_assign(&mut self, rhs: i32) {
        match self.0 {
            UNDEFINED_LEN => (), // no-op
            len => {
                self.0 = (len as i32 - rhs) as u32;
                debug_assert!(
                    self.0 != UNDEFINED_LEN,
                    "integer overflow (0xFFFF_FFFF reserved for undefined length)"
                );
            }
        }
    }
}

impl Length {
    /// Check whether this length is undefined (unknown).
    #[inline]
    pub fn is_undefined(self) -> bool {
        self.0 == UNDEFINED_LEN
    }

    /// Check whether this length is well defined (not undefined).
    #[inline]
    pub fn is_defined(self) -> bool {
        !self.is_undefined()
    }

    /// Fetch the concrete length value, if available.
    /// Returns `None` if it represents an undefined length.
    #[inline]
    pub fn get(self) -> Option<u32> {
        match self.0 {
            UNDEFINED_LEN => None,
            v => Some(v),
        }
    }

    /// Check whether the length is equally specified as another length.
    /// Unlike the implemented `PartialEq`, two undefined lengths are
    /// considered equivalent by this method.
    #[inline]
    pub fn inner_eq(self, other: Length) -> bool {
        self.0 == other.0
    }
}

impl fmt::Debug for Length {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            UNDEFINED_LEN => f.write_str("Length(Undefined)"),
            l => f.debug_tuple("Length").field(&l).finish(),
        }
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            UNDEFINED_LEN => f.write_str("U/L"),
            l => write!(f, "{}", &l),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dicom_value, DicomValue};

    #[test]
    fn to_clean_string() {
        let p = dicom_value!(U16, [256, 0, 16]);
        let val = DicomValue::new(p);
        let element = DataElement::new(Tag(0x0028, 0x3002), VR::US, val);
        assert_eq!(element.to_clean_str().unwrap(), "256\\0\\16",);
    }

    #[test]
    fn tag_from_u16_pair() {
        let t = Tag::from((0x0010u16, 0x0020u16));
        assert_eq!(0x0010u16, t.group());
        assert_eq!(0x0020u16, t.element());
    }

    #[test]
    fn tag_from_u16_array() {
        let t = Tag::from([0x0010u16, 0x0020u16]);
        assert_eq!(0x0010u16, t.group());
        assert_eq!(0x0020u16, t.element());
    }

    #[test]
    fn get_date_value() {
        let data_element: DataElement<_, _> = DataElement::new(
            Tag(0x0010, 0x0030),
            VR::DA,
            Value::new(PrimitiveValue::from("19941012")),
        );

        assert_eq!(
            data_element.to_date().unwrap(),
            DicomDate::from_ymd(1994, 10, 12).unwrap(),
        );
    }

    #[test]
    fn create_data_element_from_primitive() {
        let data_element: DataElement<EmptyObject, [u8; 0]> = DataElement::new(
            Tag(0x0028, 0x3002),
            VR::US,
            crate::dicom_value!(U16, [256, 0, 16]),
        );

        assert_eq!(data_element.uint16_slice().unwrap(), &[256, 0, 16]);
    }
}
