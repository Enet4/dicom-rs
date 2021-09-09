//! This module contains all DICOM data element encoding logic.
use byteordered::Endianness;
use dicom_core::{DataElementHeader, PrimitiveValue, Tag};
use snafu::{Backtrace, ResultExt, Snafu};
use std::fmt;
use std::io::{self, Write};
use std::marker::PhantomData;

pub mod basic;
pub mod explicit_be;
pub mod explicit_le;
pub mod implicit_le;

#[deprecated]
pub use dicom_core::value::serialize as primitive_value;

/// Module-level error type:
/// for errors which may occur while encoding DICOM data.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Failed to write Date value"))]
    WriteDate {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write Time value"))]
    WriteTime {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write DateTime value"))]
    WriteDateTime {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write tag"))]
    WriteTag {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write tag group"))]
    WriteTagGroup {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write tag element"))]
    WriteTagElement {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write item header"))]
    WriteItemHeader {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write element header"))]
    WriteHeader {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write item delimiter"))]
    WriteItemDelimiter {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write sequence delimiter"))]
    WriteSequenceDelimiter {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write {} value", typ))]
    WriteBinary {
        typ: &'static str,
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write string value"))]
    WriteString {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write bytes"))]
    WriteBytes {
        backtrace: Backtrace,
        source: io::Error,
    },
    #[snafu(display("Failed to write pixel data offset table"))]
    WriteOffsetTable {
        backtrace: Backtrace,
        source: io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Type trait for an encoder of basic data properties.
/// Unlike `Encode` (and similar to `BasicDecode`), this trait is not object
/// safe because it's better to just provide a dynamic implementation.
pub trait BasicEncode {
    /// Retrieve the encoder's endianness.
    fn endianness(&self) -> Endianness;

    /// Encode an unsigned short value to the given writer.
    fn encode_us<W>(&self, to: W, value: u16) -> io::Result<()>
    where
        W: Write;

    /// Encode an unsigned long value to the given writer.
    fn encode_ul<W>(&self, to: W, value: u32) -> io::Result<()>
    where
        W: Write;

    /// Encode an unsigned very long value to the given writer.
    fn encode_uv<W>(&self, to: W, value: u64) -> io::Result<()>
    where
        W: Write;

    /// Encode a signed short value to the given writer.
    fn encode_ss<W>(&self, to: W, value: i16) -> io::Result<()>
    where
        W: Write;

    /// Encode a signed long value to the given writer.
    fn encode_sl<W>(&self, to: W, value: i32) -> io::Result<()>
    where
        W: Write;

    /// Encode a signed very long value to the given writer.
    fn encode_sv<W>(&self, to: W, value: i64) -> io::Result<()>
    where
        W: Write;

    /// Encode a single precision float value to the given writer.
    fn encode_fl<W>(&self, to: W, value: f32) -> io::Result<()>
    where
        W: Write;

    /// Encode a double precision float value to the given writer.
    fn encode_fd<W>(&self, to: W, value: f64) -> io::Result<()>
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

    /// Encode a primitive value to the given writer. The default implementation
    /// delegates to the other value encoding methods.
    fn encode_primitive<W>(&self, mut to: W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write,
    {
        use PrimitiveValue::*;
        match value {
            Empty => Ok(0), // no-op
            Date(date) => encode_collection_delimited(&mut to, &*date, |to, date| {
                primitive_value::encode_date(to, *date)
            })
            .context(WriteDate),
            Time(time) => encode_collection_delimited(&mut to, &*time, |to, time| {
                primitive_value::encode_time(to, *time)
            })
            .context(WriteTime),
            DateTime(datetime) => {
                encode_collection_delimited(&mut to, &*datetime, |to, datetime| {
                    primitive_value::encode_datetime(to, *datetime)
                })
                .context(WriteDateTime)
            }
            Str(s) => {
                // Note: this will always print in UTF-8. Consumers should
                // intercept string primitive values and encode them according
                // to the expected character set.
                write!(to, "{}", s).context(WriteString)?;
                Ok(s.len())
            }
            Strs(s) => encode_collection_delimited(&mut to, &*s, |to, s| {
                // Note: this will always print in UTF-8. Consumers should
                // intercept string primitive values and encode them according
                // to the expected character set.
                write!(to, "{}", s)?;
                Ok(s.len())
            })
            .context(WriteString),
            F32(values) => {
                for v in values {
                    self.encode_fl(&mut to, *v)
                        .context(WriteBinary { typ: "F32" })?;
                }
                Ok(values.len() * 4)
            }
            F64(values) => {
                for v in values {
                    self.encode_fd(&mut to, *v)
                        .context(WriteBinary { typ: "F64" })?;
                }
                Ok(values.len() * 8)
            }
            U64(values) => {
                for v in values {
                    self.encode_uv(&mut to, *v)
                        .context(WriteBinary { typ: "U64" })?;
                }
                Ok(values.len() * 8)
            }
            I64(values) => {
                for v in values {
                    self.encode_sv(&mut to, *v)
                        .context(WriteBinary { typ: "I64" })?;
                }
                Ok(values.len() * 8)
            }
            U32(values) => {
                for v in values {
                    self.encode_ul(&mut to, *v)
                        .context(WriteBinary { typ: "U32" })?;
                }
                Ok(values.len() * 4)
            }
            I32(values) => {
                for v in values {
                    self.encode_sl(&mut to, *v)
                        .context(WriteBinary { typ: "I32" })?;
                }
                Ok(values.len() * 4)
            }
            U16(values) => {
                for v in values {
                    self.encode_us(&mut to, *v)
                        .context(WriteBinary { typ: "U16" })?;
                }
                Ok(values.len() * 2)
            }
            I16(values) => {
                for v in values {
                    self.encode_ss(&mut to, *v)
                        .context(WriteBinary { typ: "I16" })?;
                }
                Ok(values.len() * 2)
            }
            U8(values) => {
                to.write_all(values).context(WriteBytes)?;
                Ok(values.len())
            }
            Tags(tags) => {
                for tag in tags {
                    self.encode_us(&mut to, tag.0).context(WriteTagGroup)?;
                    self.encode_us(&mut to, tag.1).context(WriteTagElement)?;
                }
                Ok(tags.len() * 4)
            }
        }
    }
}

fn encode_collection_delimited<W, T, F>(
    to: &mut W,
    col: &[T],
    mut encode_element_fn: F,
) -> io::Result<usize>
where
    W: ?Sized + Write,
    F: FnMut(&mut W, &T) -> io::Result<usize>,
{
    let mut acc = 0;
    for (i, v) in col.iter().enumerate() {
        acc += encode_element_fn(to, v)?;
        if i < col.len() - 1 {
            to.write_all(b"\\")?;
            acc += 1;
        }
    }
    Ok(acc)
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
        to.write_all(&[0u8; 4]).context(WriteItemDelimiter)?;
        Ok(())
    }

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        self.encode_tag(&mut to, Tag(0xFFFE, 0xE0DD))?;
        to.write_all(&[0u8; 4]).context(WriteSequenceDelimiter)?;
        Ok(())
    }

    /// Encode and write a primitive DICOM value to the given destination.
    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write;
    
    /// Encode and write a DICOM pixel data offset table
    /// to the given destination.
    ///
    /// For convenience, returns the number of bytes written on success,
    /// equivalent to `offset_table.len() * 4`.
    // Note that offset tables might not apply to all forms of encoding,
    // but this method needs to exist nevertheless.
    fn encode_offset_table<W>(&self, to: W, offset_table: &[u32]) -> Result<usize>
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

    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write,
    {
        (**self).encode_primitive(to, value)
    }

    fn encode_offset_table<W>(&self, to: W, offset_table: &[u32]) -> Result<usize>
    where
        W: Write
    {
        (**self).encode_offset_table(to, offset_table)
    }
}

impl<T: ?Sized> Encode for Box<T>
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

    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write,
    {
        (**self).encode_primitive(to, value)
    }

    fn encode_offset_table<W>(&self, to: W, offset_table: &[u32]) -> Result<usize>
    where
        W: Write
    {
        (**self).encode_offset_table(to, offset_table)
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
    ///
    /// Note that data element header should be encoded as is,
    /// regardless of the given value length.
    /// Any eventual padding logic should be done at a higher level.
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
    fn encode_primitive(&self, to: &mut W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write;

    /// Encode and write a DICOM pixel data offset table
    /// to the given destination.
    ///
    /// For convenience, returns the number of bytes written on success,
    /// equivalent to `offset_table.len() * 4`.
    // Note that offset tables might not apply to all forms of encoding,
    // but this method needs to exist nevertheless.
    fn encode_offset_table(&self, to: &mut W, offset_table: &[u32]) -> Result<usize>
    where
        W: Write;
}

impl<T, W: ?Sized> EncodeTo<W> for &T
where
    T: EncodeTo<W>,
{
    fn encode_tag(&self, to: &mut W, tag: Tag) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_tag(to, tag)
    }

    fn encode_element_header(&self, to: &mut W, de: DataElementHeader) -> Result<usize>
    where
        W: Write,
    {
        (**self).encode_element_header(to, de)
    }

    fn encode_item_header(&self, to: &mut W, len: u32) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_item_header(to, len)
    }

    fn encode_item_delimiter(&self, to: &mut W) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_item_delimiter(to)
    }

    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_sequence_delimiter(to)
    }

    /// Encode and write a primitive DICOM value to the given destination.
    fn encode_primitive(&self, to: &mut W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write,
    {
        (**self).encode_primitive(to, value)
    }

    fn encode_offset_table(&self, to: &mut W, offset_table: &[u32]) -> Result<usize>
    where
        W: Write
    {
        (**self).encode_offset_table(to, offset_table)
    }
}

impl<T: ?Sized, W: ?Sized> EncodeTo<W> for Box<T>
where
    T: EncodeTo<W>,
{
    fn encode_tag(&self, to: &mut W, tag: Tag) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_tag(to, tag)
    }

    fn encode_element_header(&self, to: &mut W, de: DataElementHeader) -> Result<usize>
    where
        W: Write,
    {
        (**self).encode_element_header(to, de)
    }

    fn encode_item_header(&self, to: &mut W, len: u32) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_item_header(to, len)
    }

    fn encode_item_delimiter(&self, to: &mut W) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_item_delimiter(to)
    }

    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()>
    where
        W: Write,
    {
        (**self).encode_sequence_delimiter(to)
    }

    /// Encode and write a primitive DICOM value to the given destination.
    fn encode_primitive(&self, to: &mut W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write,
    {
        (**self).encode_primitive(to, value)
    }

    fn encode_offset_table(&self, to: &mut W, offset_table: &[u32]) -> Result<usize>
    where
        W: Write
    {
        (**self).encode_offset_table(to, offset_table)
    }
}

/// A type binding of an encoder to a target writer.
pub struct EncoderFor<T, W: ?Sized> {
    inner: T,
    phantom: PhantomData<W>,
}

impl<T, W: ?Sized> EncoderFor<T, W> {
    /** Using a generic encoder, create a new encoder specifically for the given
     * writer of type `W`.
     */
    pub fn new(encoder: T) -> Self {
        EncoderFor {
            inner: encoder,
            phantom: PhantomData,
        }
    }
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

    fn encode_us<S>(&self, to: S, value: u16) -> io::Result<()>
    where
        S: Write,
    {
        self.inner.encode_us(to, value)
    }

    fn encode_ul<S>(&self, to: S, value: u32) -> io::Result<()>
    where
        S: Write,
    {
        self.inner.encode_ul(to, value)
    }

    fn encode_uv<S>(&self, to: S, value: u64) -> io::Result<()>
    where
        S: Write,
    {
        self.inner.encode_uv(to, value)
    }

    fn encode_ss<S>(&self, to: S, value: i16) -> io::Result<()>
    where
        S: Write,
    {
        self.inner.encode_ss(to, value)
    }

    fn encode_sl<S>(&self, to: S, value: i32) -> io::Result<()>
    where
        S: Write,
    {
        self.inner.encode_sl(to, value)
    }

    fn encode_sv<S>(&self, to: S, value: i64) -> io::Result<()>
    where
        S: Write,
    {
        self.inner.encode_sv(to, value)
    }

    fn encode_fl<S>(&self, to: S, value: f32) -> io::Result<()>
    where
        S: Write,
    {
        self.inner.encode_fl(to, value)
    }

    fn encode_fd<S>(&self, to: S, value: f64) -> io::Result<()>
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

    fn encode_primitive(&self, to: &mut W, value: &PrimitiveValue) -> Result<usize> {
        self.inner.encode_primitive(to, value)
    }

    fn encode_offset_table(&self, to: &mut W, offset_table: &[u32]) -> Result<usize> {
        self.inner.encode_offset_table(to, offset_table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_encode<T: Encode>(_encoder: &T) {}
    fn is_encode_to<W: ?Sized, T: EncodeTo<W>>(_encoder: &T) {}

    #[allow(unused)]
    fn boxed_encode_is_encode<T>(encoder: T)
    where
        T: Encode,
        T: Copy,
    {
        is_encode(&encoder);
        is_encode_to::<dyn Write, _>(&EncoderFor::new(encoder));
        let boxed = Box::new(encoder);
        is_encode(&boxed);
        is_encode_to::<dyn Write, _>(&EncoderFor::new(boxed));
    }
}
