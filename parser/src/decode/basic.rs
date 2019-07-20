//! This module provides implementations for primitive decoders of data, which
//! may be in either Little Endian or Big Endian.

use byteordered::{ByteOrdered, Endianness};
use crate::error::Result;
use std::io::Read;
use super::BasicDecode;

/// A basic decoder of DICOM primitive elements in little endian.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LittleEndianBasicDecoder;

impl BasicDecode for LittleEndianBasicDecoder {
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }

    fn decode_us<S>(&self, source: S) -> Result<u16>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_u16().map_err(Into::into)
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_u32().map_err(Into::into)
    }

    fn decode_uv<S>(&self, source: S) -> Result<u64>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_u64().map_err(Into::into)
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_i16().map_err(Into::into)
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_i32().map_err(Into::into)
    }

    fn decode_sv<S>(&self, source: S) -> Result<i64>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_i64().map_err(Into::into)
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_f32().map_err(Into::into)
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_f64().map_err(Into::into)
    }
}

/// A basic decoder of DICOM primitive elements in big endian.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct BigEndianBasicDecoder;

impl BasicDecode for BigEndianBasicDecoder {
    fn endianness(&self) -> Endianness {
        Endianness::Big
    }

    fn decode_us<S>(&self, source: S) -> Result<u16>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_u16().map_err(Into::into)
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_u32().map_err(Into::into)
    }
    
    fn decode_uv<S>(&self, source: S) -> Result<u64>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_u64().map_err(Into::into)
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_i16().map_err(Into::into)
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_i32().map_err(Into::into)
    }

    fn decode_sv<S>(&self, source: S) -> Result<i64>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_i64().map_err(Into::into)
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_f32().map_err(Into::into)
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_f64().map_err(Into::into)
    }
}

/// A basic decoder with support for both Little Endian an Big Endian
/// encoding, decided at run-time. Since only two values are possible,
/// this enum may become more efficient than the use of a trait object.
#[derive(Debug, Clone, PartialEq)]
pub enum BasicDecoder {
    /// Decode in Little Endian
    LE(LittleEndianBasicDecoder),
    /// Decode in Big Endian
    BE(BigEndianBasicDecoder),
}

impl BasicDecoder {
    pub fn new(endianness: Endianness) -> Self {
        match endianness {
            Endianness::Little => LE(LittleEndianBasicDecoder::default()),
            Endianness::Big => BE(BigEndianBasicDecoder::default()),
        }
    }
}

use self::BasicDecoder::{BE, LE};

impl From<Endianness> for BasicDecoder {
    fn from(endianness: Endianness) -> Self {
        BasicDecoder::new(endianness)
    }
}

macro_rules! for_both {
    ($s: expr, |$e: ident| $f: expr) => {
        match *$s {
            LE(ref $e) => $f,
            BE(ref $e) => $f
        }
    }
}

impl BasicDecode for BasicDecoder {
    fn endianness(&self) -> Endianness {
        match *self {
            LE(_) => Endianness::Little,
            BE(_) => Endianness::Big,
        }
    }

    fn decode_us<S>(&self, source: S) -> Result<u16>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_us(source))
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_ul(source))
    }

    fn decode_uv<S>(&self, source: S) -> Result<u64>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_uv(source))
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_ss(source))
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_sl(source))
    }

    fn decode_sv<S>(&self, source: S) -> Result<i64>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_sv(source))
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_fl(source))
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_fd(source))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_read_integers() {
        let data: &[u8] = &[
            0xC3, 0x3C, 0x33, 0xCC, 0x55, 0xAA, 0x55, 0xAA,
        ];

        let le = LittleEndianBasicDecoder;
        let be = BigEndianBasicDecoder;

        assert_eq!(le.decode_us(data).unwrap(), 0x3CC3);
        assert_eq!(be.decode_us(data).unwrap(), 0xC33C);
        assert_eq!(le.decode_ul(data).unwrap(), 0xCC333CC3);
        assert_eq!(be.decode_ul(data).unwrap(), 0xC33C33CC);
        assert_eq!(le.decode_uv(data).unwrap(), 0xAA55AA55_CC333CC3);
        assert_eq!(be.decode_uv(data).unwrap(), 0xC33C33CC_55AA55AA);

        let le = BasicDecoder::new(Endianness::Little);
        let be = BasicDecoder::new(Endianness::Big);

        assert_eq!(le.decode_us(data).unwrap(), 0x3CC3);
        assert_eq!(be.decode_us(data).unwrap(), 0xC33C);
        assert_eq!(le.decode_ul(data).unwrap(), 0xCC333CC3);
        assert_eq!(be.decode_ul(data).unwrap(), 0xC33C33CC);
        assert_eq!(le.decode_uv(data).unwrap(), 0xAA55AA55_CC333CC3);
        assert_eq!(be.decode_uv(data).unwrap(), 0xC33C33CC_55AA55AA);
    }
}