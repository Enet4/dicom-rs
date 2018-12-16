//! This module provides implementations for basic encoders: little endian and big endian.
//!

use super::BasicEncode;
use byteordered::{ByteOrdered, Endianness};
use error::Result;
use std::io::Write;

/// A basic encoder of primitive elements in little endian.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LittleEndianBasicEncoder;

impl BasicEncode for LittleEndianBasicEncoder {
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }

    fn encode_us<S>(&self, to: S, value: u16) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_u16(value)?;
        Ok(())
    }

    fn encode_ul<S>(&self, to: S, value: u32) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_u32(value)?;
        Ok(())
    }

    fn encode_ss<S>(&self, to: S, value: i16) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_i16(value)?;
        Ok(())
    }

    fn encode_sl<S>(&self, to: S, value: i32) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_i32(value)?;
        Ok(())
    }

    fn encode_fl<S>(&self, to: S, value: f32) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_f32(value)?;
        Ok(())
    }

    fn encode_fd<S>(&self, to: S, value: f64) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_f64(value)?;
        Ok(())
    }
}

/// A basic encoder of DICOM primitive elements in big endian.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct BigEndianBasicEncoder;

impl BasicEncode for BigEndianBasicEncoder {
    fn endianness(&self) -> Endianness {
        Endianness::Big
    }

    fn encode_us<S>(&self, to: S, value: u16) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_u16(value)?;
        Ok(())
    }

    fn encode_ul<S>(&self, to: S, value: u32) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_u32(value)?;
        Ok(())
    }

    fn encode_ss<S>(&self, to: S, value: i16) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_i16(value)?;
        Ok(())
    }

    fn encode_sl<S>(&self, to: S, value: i32) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_i32(value)?;
        Ok(())
    }

    fn encode_fl<S>(&self, to: S, value: f32) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_f32(value)?;
        Ok(())
    }

    fn encode_fd<S>(&self, to: S, value: f64) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_f64(value)?;
        Ok(())
    }
}

/// A basic encoder with support for both Little Endian an Big Endian
/// encoding, decided at run-time. Since only two values are possible,
/// this enum may become more practical and efficient than relying on trait objects.
#[derive(Debug, Clone, PartialEq)]
pub enum BasicEncoder {
    /// Encode in Little Endian
    LE(LittleEndianBasicEncoder),
    /// Encode in Big Endian
    BE(BigEndianBasicEncoder),
}

use self::BasicEncoder::{BE, LE};

/// Handle multiple encoding tasks with the expected endianness. The parameter `$e`
/// will either yield a `LittleEndianBasicEncoder` or a `BigEndianBasicEncoder`. When
/// the specific basic encoder is still unknown in compile-time, this macro can be used
/// to resolve the endianess only once.
macro_rules! for_both {
    ($endianness: expr, |$e: ident| $f: expr) => (
        match *$endianness {
            LE(ref $e) => $f,
            BE(ref $e) => $f
        }
    )
}

impl BasicEncode for BasicEncoder {
    fn endianness(&self) -> Endianness {
        match *self {
            LE(_) => Endianness::Little,
            BE(_) => Endianness::Big,
        }
    }

    fn encode_us<S>(&self, to: S, value: u16) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_us(to, value))
    }

    fn encode_ul<S>(&self, to: S, value: u32) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_ul(to, value))
    }

    fn encode_ss<S>(&self, to: S, value: i16) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_ss(to, value))
    }

    fn encode_sl<S>(&self, to: S, value: i32) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_sl(to, value))
    }

    fn encode_fl<S>(&self, to: S, value: f32) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_fl(to, value))
    }

    fn encode_fd<S>(&self, to: S, value: f64) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_fd(to, value))
    }
}
