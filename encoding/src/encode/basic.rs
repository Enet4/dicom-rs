//! This module provides implementations for basic encoders: little endian and big endian.
//!

use super::BasicEncode;
use byteordered::{ByteOrdered, Endianness};
use std::io::Write;

type Result<T> = std::io::Result<T>;

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

    fn encode_uv<S>(&self, to: S, value: u64) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_u64(value)?;
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

    fn encode_sv<S>(&self, to: S, value: i64) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::le(to).write_i64(value)?;
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

    fn encode_uv<S>(&self, to: S, value: u64) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_u64(value)?;
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

    fn encode_sv<S>(&self, to: S, value: i64) -> Result<()>
    where
        S: Write,
    {
        ByteOrdered::be(to).write_i64(value)?;
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
    ($endianness: expr, |$e: ident| $f: expr) => {
        match *$endianness {
            LE(ref $e) => $f,
            BE(ref $e) => $f,
        }
    };
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

    fn encode_uv<S>(&self, to: S, value: u64) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_uv(to, value))
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

    fn encode_sv<S>(&self, to: S, value: i64) -> Result<()>
    where
        S: Write,
    {
        for_both!(self, |e| e.encode_sv(to, value))
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

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::value::DicomDate;
    use dicom_core::{PrimitiveValue, Tag};

    fn test_one_primitive_be(value: PrimitiveValue, raw: &[u8]) {
        let mut out = vec![];
        BigEndianBasicEncoder
            .encode_primitive(&mut out, &value)
            .unwrap();
        assert_eq!(&*out, raw);
    }

    fn test_one_primitive_le(value: PrimitiveValue, raw: &[u8]) {
        let mut out = vec![];
        LittleEndianBasicEncoder
            .encode_primitive(&mut out, &value)
            .unwrap();
        assert_eq!(&*out, raw);
    }

    #[test]
    fn test_basic_encode_le() {
        test_one_primitive_le(PrimitiveValue::Empty, &[]);
        test_one_primitive_le(
            PrimitiveValue::I32(vec![0x01, 0x0200, 0x0300_FFCC].into()),
            &[
                0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0xCC, 0xFF, 0x00, 0x03,
            ],
        );

        test_one_primitive_le(
            PrimitiveValue::Strs(
                ["one", "more", "time"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            &*b"one\\more\\time",
        );

        test_one_primitive_le(
            PrimitiveValue::Date(
                vec![
                    DicomDate::from_ymd(2016, 12, 01).unwrap(),
                    DicomDate::from_ymd(2123, 9, 13).unwrap(),
                ]
                .into(),
            ),
            &*b"20161201\\21230913",
        );

        test_one_primitive_le(
            PrimitiveValue::Tags(vec![Tag(0x0002, 0x0001), Tag(0xFA80, 0xBC12)].into()),
            &[0x02, 0x00, 0x01, 0x00, 0x80, 0xFA, 0x12, 0xBC],
        );
    }

    #[test]
    fn test_basic_encode_be() {
        test_one_primitive_be(PrimitiveValue::Empty, &[]);
        test_one_primitive_be(
            PrimitiveValue::I32(vec![0x01, 0x0200, 0x0300_FFCC].into()),
            &[
                0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0xFF, 0xCC,
            ],
        );

        test_one_primitive_be(
            PrimitiveValue::Strs(
                ["one", "more", "time"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            &*b"one\\more\\time",
        );

        test_one_primitive_be(
            PrimitiveValue::Date(
                vec![
                    DicomDate::from_ymd(2016, 12, 01).unwrap(),
                    DicomDate::from_ym(2123, 9).unwrap(),
                ]
                .into(),
            ),
            &*b"20161201\\212309",
        );

        test_one_primitive_be(
            PrimitiveValue::Tags(vec![Tag(0x0002, 0x0001), Tag(0xFA80, 0xBC12)].into()),
            &[0x00, 0x02, 0x00, 0x01, 0xFA, 0x80, 0xBC, 0x12],
        );
    }
}
