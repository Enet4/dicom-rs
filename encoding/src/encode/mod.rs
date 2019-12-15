//! This module contains all DICOM data element encoding logic.
use crate::error::Result;
use byteordered::Endianness;
use dicom_core::{DataElementHeader, Tag};
use std::fmt;
use std::io::Write;
use std::marker::PhantomData;

pub mod basic;

/// Type trait for an encoder of basic data properties.
/// Unlike `Encode` (and similar to `BasicDecode`), this trait is not object
/// safe because it's better to just provide a dynamic implementation.
pub trait BasicEncode {
    /// Retrieve the encoder's endianness.
    fn endianness(&self) -> Endianness;

    /// Encode an unsigned short value to the given writer.
    fn encode_us<S>(&self, to: S, value: u16) -> Result<()>
    where
        S: Write;

    /// Encode an unsigned long value to the given writer.
    fn encode_ul<S>(&self, to: S, value: u32) -> Result<()>
    where
        S: Write;

    /// Encode a signed short value to the given writer.
    fn encode_ss<S>(&self, to: S, value: i16) -> Result<()>
    where
        S: Write;

    /// Encode a signed long value to the given writer.
    fn encode_sl<S>(&self, to: S, value: i32) -> Result<()>
    where
        S: Write;

    /// Encode a single precision float value to the given writer.
    fn encode_fl<S>(&self, to: S, value: f32) -> Result<()>
    where
        S: Write;

    /// Encode a double precision float value to the given writer.
    fn encode_fd<S>(&self, to: S, value: f64) -> Result<()>
    where
        S: Write;

    /// Perform
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
}

/// Type trait for a data element encoder.
pub trait Encode {
    /// Encode and write an element tag.
    fn encode_tag<W>(&self, to: &mut W, tag: Tag) -> Result<()>
    where
        W: ?Sized + Write;

    /// Encode and write a data element header to the given destination.
    /// Returns the number of bytes effectively written on success.
    fn encode_element_header<W>(&self, to: &mut W, de: DataElementHeader) -> Result<usize>
    where
        W: ?Sized + Write;

    /// Encode and write a DICOM sequence item header to the given destination.
    /* Although item element headers are always a tag and length sequence regardless of TS,
    the encoding of the length is unknown at this level. So no default impl. */
    fn encode_item_header<W>(&self, to: &mut W, len: u32) -> Result<()>
    where
        W: ?Sized + Write;

    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter<W>(&self, to: &mut W) -> Result<()>
    where
        W: ?Sized + Write,
    {
        self.encode_tag(to, Tag(0xFFFE, 0xE00D))?;
        to.write_all(&[0u8; 4])?;
        Ok(())
    }

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter<W>(&self, to: &mut W) -> Result<()>
    where
        W: ?Sized + Write,
    {
        self.encode_tag(to, Tag(0xFFFE, 0xE0DD))?;
        to.write_all(&[0u8; 4])?;
        Ok(())
    }
}

impl<T: ?Sized> Encode for &T
where
    T: Encode,
{
    fn encode_tag<W>(&self, to: &mut W, tag: Tag) -> Result<()>
    where
        W: ?Sized + Write,
    {
        (**self).encode_tag(to, tag)
    }

    fn encode_element_header<W>(&self, to: &mut W, de: DataElementHeader) -> Result<usize>
    where
        W: ?Sized + Write,
    {
        (**self).encode_element_header(to, de)
    }

    fn encode_item_header<W>(&self, to: &mut W, len: u32) -> Result<()>
    where
        W: ?Sized + Write,
    {
        (**self).encode_item_header(to, len)
    }

    fn encode_item_delimiter<W>(&self, to: &mut W) -> Result<()>
    where
        W: ?Sized + Write,
    {
        (**self).encode_item_delimiter(to)
    }

    fn encode_sequence_delimiter<W>(&self, to: &mut W) -> Result<()>
    where
        W: ?Sized + Write,
    {
        (**self).encode_sequence_delimiter(to)
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
        W: Write,
    {
        self.encode_tag(to, Tag(0xFFFE, 0xE00D))?;
        to.write_all(&[0u8; 4])?;
        Ok(())
    }

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()>
    where
        W: Write,
    {
        self.encode_tag(to, Tag(0xFFFE, 0xE0DD))?;
        to.write_all(&[0u8; 4])?;
        Ok(())
    }
}

impl<T, W> EncodeTo<W> for T
where
    T: Encode,
    W: ?Sized + Write,
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
}
