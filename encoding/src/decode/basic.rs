//! This module provides implementations for primitive decoders of data, which
//! may be in either Little Endian or Big Endian.

use super::BasicDecode;
use byteordered::{ByteOrdered, Endianness};
use std::io::Read;

type Result<T> = std::io::Result<T>;

/// A basic decoder of DICOM primitive elements in little endian.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
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

    fn decode_us_into<S>(&self, source: S, target: &mut [u16]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_u16_into(target)
            .map_err(Into::into)
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_u32().map_err(Into::into)
    }

    fn decode_ul_into<S>(&self, source: S, target: &mut [u32]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_u32_into(target)
            .map_err(Into::into)
    }

    fn decode_uv<S>(&self, source: S) -> Result<u64>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_u64().map_err(Into::into)
    }

    fn decode_uv_into<S>(&self, source: S, target: &mut [u64]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_u64_into(target)
            .map_err(Into::into)
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_i16().map_err(Into::into)
    }

    fn decode_ss_into<S>(&self, source: S, target: &mut [i16]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_i16_into(target)
            .map_err(Into::into)
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_i32().map_err(Into::into)
    }

    fn decode_sl_into<S>(&self, source: S, target: &mut [i32]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_i32_into(target)
            .map_err(Into::into)
    }

    fn decode_sv<S>(&self, source: S) -> Result<i64>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_i64().map_err(Into::into)
    }

    fn decode_sv_into<S>(&self, source: S, target: &mut [i64]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_i64_into(target)
            .map_err(Into::into)
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_f32().map_err(Into::into)
    }

    fn decode_fl_into<S>(&self, source: S, target: &mut [f32]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_f32_into(target)
            .map_err(Into::into)
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        ByteOrdered::le(source).read_f64().map_err(Into::into)
    }

    fn decode_fd_into<S>(&self, source: S, target: &mut [f64]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::le(source)
            .read_f64_into(target)
            .map_err(Into::into)
    }
}

/// A basic decoder of DICOM primitive elements in big endian.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
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

    fn decode_us_into<S>(&self, source: S, target: &mut [u16]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_u16_into(target)
            .map_err(Into::into)
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_u32().map_err(Into::into)
    }

    fn decode_ul_into<S>(&self, source: S, target: &mut [u32]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_u32_into(target)
            .map_err(Into::into)
    }

    fn decode_uv<S>(&self, source: S) -> Result<u64>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_u64().map_err(Into::into)
    }

    fn decode_uv_into<S>(&self, source: S, target: &mut [u64]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_u64_into(target)
            .map_err(Into::into)
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_i16().map_err(Into::into)
    }

    fn decode_ss_into<S>(&self, source: S, target: &mut [i16]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_i16_into(target)
            .map_err(Into::into)
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_i32().map_err(Into::into)
    }

    fn decode_sl_into<S>(&self, source: S, target: &mut [i32]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_i32_into(target)
            .map_err(Into::into)
    }

    fn decode_sv<S>(&self, source: S) -> Result<i64>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_i64().map_err(Into::into)
    }

    fn decode_sv_into<S>(&self, source: S, target: &mut [i64]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_i64_into(target)
            .map_err(Into::into)
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_f32().map_err(Into::into)
    }

    fn decode_fl_into<S>(&self, source: S, target: &mut [f32]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_f32_into(target)
            .map_err(Into::into)
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        ByteOrdered::be(source).read_f64().map_err(Into::into)
    }

    fn decode_fd_into<S>(&self, source: S, target: &mut [f64]) -> Result<()>
    where
        S: Read,
    {
        ByteOrdered::be(source)
            .read_f64_into(target)
            .map_err(Into::into)
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
    /// Create a basic decoder for the given byte order.
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
            BE(ref $e) => $f,
        }
    };
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

    fn decode_us_into<S>(&self, source: S, target: &mut [u16]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_us_into(source, target))
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_ul(source))
    }

    fn decode_ul_into<S>(&self, source: S, target: &mut [u32]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_ul_into(source, target))
    }

    fn decode_uv<S>(&self, source: S) -> Result<u64>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_uv(source))
    }

    fn decode_uv_into<S>(&self, source: S, target: &mut [u64]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_uv_into(source, target))
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_ss(source))
    }

    fn decode_ss_into<S>(&self, source: S, target: &mut [i16]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_ss_into(source, target))
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_sl(source))
    }

    fn decode_sl_into<S>(&self, source: S, target: &mut [i32]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_sl_into(source, target))
    }

    fn decode_sv<S>(&self, source: S) -> Result<i64>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_sv(source))
    }

    fn decode_sv_into<S>(&self, source: S, target: &mut [i64]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_sv_into(source, target))
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_fl(source))
    }

    fn decode_fl_into<S>(&self, source: S, target: &mut [f32]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_fl_into(source, target))
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_fd(source))
    }

    fn decode_fd_into<S>(&self, source: S, target: &mut [f64]) -> Result<()>
    where
        S: Read,
    {
        for_both!(self, |e| e.decode_fd_into(source, target))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_read_integers() {
        let data: &[u8] = &[0xC3, 0x3C, 0x33, 0xCC, 0x55, 0xAA, 0x55, 0xAA];

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

    #[test]
    fn test_read_integers_into() {
        let data: &[u8] = &[0xC3, 0x3C, 0x33, 0xCC, 0x55, 0xAA, 0x55, 0xAA];

        let le = LittleEndianBasicDecoder;
        let be = BigEndianBasicDecoder;

        let mut out_le = [0; 4];
        le.decode_us_into(data, &mut out_le).unwrap();
        assert_eq!(out_le, [0x3CC3, 0xCC33, 0xAA55, 0xAA55]);

        let mut out_be = [0; 4];
        be.decode_us_into(data, &mut out_be).unwrap();
        assert_eq!(out_be, [0xC33C, 0x33CC, 0x55AA, 0x55AA]);

        let mut out_le = [0; 2];
        le.decode_ul_into(data, &mut out_le).unwrap();
        assert_eq!(out_le, [0xCC33_3CC3, 0xAA55_AA55]);

        let mut out_be = [0; 2];
        be.decode_ul_into(data, &mut out_be).unwrap();
        assert_eq!(out_be, [0xC33C_33CC, 0x55AA_55AA]);

        let le = BasicDecoder::new(Endianness::Little);
        let be = BasicDecoder::new(Endianness::Big);

        let mut out_le = [0; 4];
        le.decode_us_into(data, &mut out_le).unwrap();
        assert_eq!(out_le, [0x3CC3, 0xCC33, 0xAA55, 0xAA55]);

        let mut out_be = [0; 4];
        be.decode_us_into(data, &mut out_be).unwrap();
        assert_eq!(out_be, [0xC33C, 0x33CC, 0x55AA, 0x55AA]);

        let mut out_le = [0; 2];
        le.decode_ul_into(data, &mut out_le).unwrap();
        assert_eq!(out_le, [0xCC33_3CC3, 0xAA55_AA55]);

        let mut out_be = [0; 2];
        be.decode_ul_into(data, &mut out_be).unwrap();
        assert_eq!(out_be, [0xC33C_33CC, 0x55AA_55AA]);
    }
}
