//! This module contains all DICOM data element encoding logic.
use crate::error::Result;
use byteordered::Endianness;
use dicom_core::{DataElementHeader, PrimitiveValue, Tag};
use std::fmt;
use std::io::Write;
use std::marker::PhantomData;

pub mod basic;
mod primitive_value;

/// Type trait for an encoder of basic data properties.
/// Unlike `Encode` (and similar to `BasicDecode`), this trait is not object
/// safe because it's better to just provide a dynamic implementation.
pub trait BasicEncode {
    /// Retrieve the encoder's endianness.
    fn endianness(&self) -> Endianness;

    /// Encode an unsigned short value to the given writer.
    fn encode_us<W>(&self, to: W, value: u16) -> Result<()>
    where
        W: Write;

    /// Encode an unsigned long value to the given writer.
    fn encode_ul<W>(&self, to: W, value: u32) -> Result<()>
    where
        W: Write;

    /// Encode an unsigned very long value to the given writer.
    fn encode_uv<W>(&self, to: W, value: u64) -> Result<()>
    where
        W: Write;

    /// Encode a signed short value to the given writer.
    fn encode_ss<W>(&self, to: W, value: i16) -> Result<()>
    where
        W: Write;

    /// Encode a signed long value to the given writer.
    fn encode_sl<W>(&self, to: W, value: i32) -> Result<()>
    where
        W: Write;

    /// Encode a signed very long value to the given writer.
    fn encode_sv<W>(&self, to: W, value: i64) -> Result<()>
    where
        W: Write;

    /// Encode a single precision float value to the given writer.
    fn encode_fl<W>(&self, to: W, value: f32) -> Result<()>
    where
        W: Write;

    /// Encode a double precision float value to the given writer.
    fn encode_fd<W>(&self, to: W, value: f64) -> Result<()>
    where
        W: Write;

    /// If this encoder is in Little Endian, evaluate the first function.
    /// Otherwise, evaluate the second one.
    #[inline]
    fn with_encoder<T, F1, F2>(&self, f_le: F1, f_be: F2) -> T
    where
        F1: FnOnce(self::basic::LittleEndianBasicEncoder) -> T,
        F2: FnOnce(self::basic::BigEndianBasicEncoder) -> T,
    {
        match self.endianness() {
            Endianness::Little => f_le(self::basic::LittleEndianBasicEncoder),
            Endianness::Big => f_be(self::basic::BigEndianBasicEncoder),
        }
    }

    /// Encode a primitive  float value to the given writer. The default
    /// implementation delegates to the other value encoding methods.
    fn encode_primitive<W>(&self, mut to: W, value: &PrimitiveValue) -> Result<()>
    where
        W: Write,
    {
        use PrimitiveValue::*;
        match value {
            Empty => Ok(()), // no-op
            Date(date) => encode_collection_delimited(&mut to, &*date, |to, date| {
                primitive_value::encode_date(to, *date)
            }),
            Time(time) => encode_collection_delimited(&mut to, &*time, |to, time| {
                primitive_value::encode_time(to, *time)
            }),
            DateTime(datetime) => {
                encode_collection_delimited(&mut to, &*datetime, |to, datetime| {
                    primitive_value::encode_datetime(to, *datetime)
                })
            }
            Str(s) => {
                write!(to, "{}", s)?;
                Ok(())
            }
            Strs(s) => encode_collection_delimited(&mut to, &*s, |to, s| {
                write!(to, "{}", s)?;
                Ok(())
            }),
            F32(values) => {
                for v in values {
                    self.encode_fl(&mut to, *v)?;
                }
                Ok(())
            }
            F64(values) => {
                for v in values {
                    self.encode_fd(&mut to, *v)?;
                }
                Ok(())
            }
            U64(values) => {
                for v in values {
                    self.encode_uv(&mut to, *v)?;
                }
                Ok(())
            }
            I64(values) => {
                for v in values {
                    self.encode_sv(&mut to, *v)?;
                }
                Ok(())
            }
            U32(values) => {
                for v in values {
                    self.encode_ul(&mut to, *v)?;
                }
                Ok(())
            }
            I32(values) => {
                for v in values {
                    self.encode_sl(&mut to, *v)?;
                }
                Ok(())
            }
            U16(values) => {
                for v in values {
                    self.encode_us(&mut to, *v)?;
                }
                Ok(())
            }
            I16(values) => {
                for v in values {
                    self.encode_ss(&mut to, *v)?;
                }
                Ok(())
            }
            U8(values) => {
                to.write_all(values)?;
                Ok(())
            }
            Tags(tags) => {
                for tag in tags {
                    self.encode_us(&mut to, tag.0)?;
                    self.encode_us(&mut to, tag.1)?;
                }
                Ok(())
            }
        }
    }
}

fn encode_collection_delimited<W, T, F>(
    to: &mut W,
    col: &[T],
    mut encode_element_fn: F,
) -> Result<()>
where
    W: ?Sized + Write,
    F: FnMut(&mut W, &T) -> Result<()>,
{
    for (i, v) in col.iter().enumerate() {
        encode_element_fn(to, v)?;
        if i < col.len() - 1 {
            to.write_all(b"\\")?;
        }
    }
    Ok(())
}

/// Type trait for a data element encoder.
pub trait Encode {
    /// Encode and write an element tag.
    fn encode_tag<W>(&self, to: W, tag: Tag) -> Result<()>
    where
        W: Write;

    /// Encode and write a data element header to the given destination.
    /// Returns the number of bytes effectively written on success.
    fn encode_element_header<W>(&self, to: W, de: DataElementHeader) -> Result<usize>
    where
        W: Write;

    /// Encode and write a DICOM sequence item header to the given destination.
    /* Although item element headers are always a tag and length sequence regardless of TS,
    the encoding of the length is unknown at this level. So no default impl. */
    fn encode_item_header<W>(&self, to: W, len: u32) -> Result<()>
    where
        W: Write;

    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        self.encode_tag(&mut to, Tag(0xFFFE, 0xE00D))?;
        to.write_all(&[0u8; 4])?;
        Ok(())
    }

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        self.encode_tag(&mut to, Tag(0xFFFE, 0xE0DD))?;
        to.write_all(&[0u8; 4])?;
        Ok(())
    }

    /// Encode and write a primitive DICOM value to the given destination.
    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<()>
    where
        W: Write;
}

impl<T: ?Sized> Encode for &T
where
    T: Encode,
{
    fn encode_tag<W>(&self, to: W, tag: Tag) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_tag(to, tag)
    }

    fn encode_element_header<W>(&self, to: W, de: DataElementHeader) -> Result<usize>
    where
        W: Write,
    {
        (**self).encode_element_header(to, de)
    }

    fn encode_item_header<W>(&self, to: W, len: u32) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_item_header(to, len)
    }

    fn encode_item_delimiter<W>(&self, to: W) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_item_delimiter(to)
    }

    fn encode_sequence_delimiter<W>(&self, to: W) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_sequence_delimiter(to)
    }

    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_primitive(to, value)
    }
}

/// Type trait for a data element encoder to a single known writer type `W`.
pub trait EncodeTo<W: ?Sized> {
    /// Encode and write an element tag.
    fn encode_tag(&self, to: &mut W, tag: Tag) -> Result<()>
    where
        W: Write;

    /// Encode and write a data element header to the given destination.
    /// Returns the number of bytes effectively written on success.
    fn encode_element_header(&self, to: &mut W, de: DataElementHeader) -> Result<usize>
    where
        W: Write;

    /// Encode and write a DICOM sequence item header to the given destination.
    /* Although item element headers are always a tag and length sequence regardless of TS,
    the encoding of the length is unknown at this level. So no default impl. */
    fn encode_item_header(&self, to: &mut W, len: u32) -> Result<()>
    where
        W: Write;

    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter(&self, to: &mut W) -> Result<()>
    where
        W: Write;

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()>
    where
        W: Write;

    /// Encode and write a primitive DICOM value to the given destination.
    fn encode_primitive(&self, to: &mut W, value: &PrimitiveValue) -> Result<()>
    where
        W: Write;
}

impl<T, W: ?Sized> EncodeTo<W> for T
where
    T: Encode,
    W: Write,
{
    fn encode_tag(&self, to: &mut W, tag: Tag) -> Result<()> {
        Encode::encode_tag(self, to, tag)
    }

    fn encode_element_header(&self, to: &mut W, de: DataElementHeader) -> Result<usize> {
        Encode::encode_element_header(self, to, de)
    }

    fn encode_item_header(&self, to: &mut W, len: u32) -> Result<()> {
        Encode::encode_item_header(self, to, len)
    }

    fn encode_item_delimiter(&self, to: &mut W) -> Result<()> {
        Encode::encode_item_delimiter(self, to)
    }

    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()> {
        Encode::encode_sequence_delimiter(self, to)
    }

    fn encode_primitive(&self, to: &mut W, value: &PrimitiveValue) -> Result<()> {
        Encode::encode_primitive(self, to, value)
    }
}

/// A type binding of an encoder to a target writer.
pub struct EncoderFor<T, W: ?Sized> {
    inner: T,
    phantom: PhantomData<W>,
}

impl<T: fmt::Debug, W: ?Sized> fmt::Debug for EncoderFor<T, W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ImplicitVRLittleEndianEncoder")
            .field("inner", &self.inner)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<T, W: ?Sized> Default for EncoderFor<T, W>
where
    T: Default,
{
    fn default() -> Self {
        EncoderFor {
            inner: T::default(),
            phantom: PhantomData,
        }
    }
}

impl<T, W: ?Sized> BasicEncode for EncoderFor<T, W>
where
    T: BasicEncode,
    W: Write,
{
    fn endianness(&self) -> Endianness {
        self.inner.endianness()
    }

    fn encode_us<S>(&self, to: S, value: u16) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_us(to, value)
    }

    fn encode_ul<S>(&self, to: S, value: u32) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_ul(to, value)
    }

    fn encode_uv<S>(&self, to: S, value: u64) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_uv(to, value)
    }

    fn encode_ss<S>(&self, to: S, value: i16) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_ss(to, value)
    }

    fn encode_sl<S>(&self, to: S, value: i32) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_sl(to, value)
    }

    fn encode_sv<S>(&self, to: S, value: i64) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_sv(to, value)
    }

    fn encode_fl<S>(&self, to: S, value: f32) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_fl(to, value)
    }

    fn encode_fd<S>(&self, to: S, value: f64) -> Result<()>
    where
        S: Write,
    {
        self.inner.encode_fd(to, value)
    }
}

impl<T, W: ?Sized> EncodeTo<W> for EncoderFor<T, W>
where
    T: Encode,
    W: Write,
{
    fn encode_tag(&self, to: &mut W, tag: Tag) -> Result<()> {
        self.inner.encode_tag(to, tag)
    }

    fn encode_element_header(&self, to: &mut W, de: DataElementHeader) -> Result<usize> {
        self.inner.encode_element_header(to, de)
    }

    fn encode_item_header(&self, to: &mut W, len: u32) -> Result<()> {
        self.inner.encode_item_header(to, len)
    }

    fn encode_item_delimiter(&self, to: &mut W) -> Result<()> {
        self.inner.encode_item_delimiter(to)
    }

    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()> {
        self.inner.encode_sequence_delimiter(to)
    }

    fn encode_primitive(&self, to: &mut W, value: &PrimitiveValue) -> Result<()> {
        self.inner.encode_primitive(to, value)
    }
}
