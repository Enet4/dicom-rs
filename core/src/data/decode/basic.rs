//! This module provides implementations for basic decoders: little endian and big endian.
//!

use super::BasicDecode;
use std::marker::PhantomData;
use byteorder::{LittleEndian, BigEndian, ReadBytesExt};
use error::Result;
use util::Endianness;
use std::io::Read;
use std::fmt;

/// A basic decoder of DICOM primitive elements in little endian.
pub struct LittleEndianBasicDecoder<S: ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: ?Sized> fmt::Debug for LittleEndianBasicDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LittleEndianBasicDecoder")
    }
}

impl<S: Read + ?Sized> Default for LittleEndianBasicDecoder<S> {
    fn default() -> LittleEndianBasicDecoder<S> {
        LittleEndianBasicDecoder { phantom: PhantomData::default() }
    }
}

impl<S: Read + ?Sized> BasicDecode for LittleEndianBasicDecoder<S> {
    type Source = S;

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        Ok(source.read_u16::<LittleEndian>()?)
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        Ok(source.read_u32::<LittleEndian>()?)
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        Ok(source.read_i16::<LittleEndian>()?)
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        Ok(source.read_i32::<LittleEndian>()?)
    }

    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32> {
        Ok(source.read_f32::<LittleEndian>()?)
    }

    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64> {
        Ok(source.read_f64::<LittleEndian>()?)
    }
}

/// A basic decoder of DICOM primitive elements in big endian.
pub struct BigEndianBasicDecoder<S: ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: ?Sized> fmt::Debug for BigEndianBasicDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BigEndianBasicDecoder")
    }
}

impl<S: ?Sized> Default for BigEndianBasicDecoder<S> {
    fn default() -> BigEndianBasicDecoder<S> {
        BigEndianBasicDecoder { phantom: PhantomData::default() }
    }
}

impl<S: Read + ?Sized> BasicDecode for BigEndianBasicDecoder<S> {
    type Source = S;

    fn endianness(&self) -> Endianness {
        Endianness::BE
    }

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        Ok(source.read_u16::<BigEndian>()?)
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        Ok(source.read_u32::<BigEndian>()?)
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        Ok(source.read_i16::<BigEndian>()?)
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        Ok(source.read_i32::<BigEndian>()?)
    }

    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32> {
        Ok(source.read_f32::<BigEndian>()?)
    }

    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64> {
        Ok(source.read_f64::<BigEndian>()?)
    }
}

/// A basic decoder with support for both Little Endian an Big Endian
/// encoding, decided at run-time. Since only two values are possible,
/// this enum may become more efficient than the use of a trait object.
pub enum BasicDecoder<S: ?Sized> {
    LE(LittleEndianBasicDecoder<S>),
    BE(BigEndianBasicDecoder<S>),
}

use self::BasicDecoder::{LE, BE};

impl<S: ?Sized> From<Endianness> for BasicDecoder<S>
    where S: Read
{
    fn from(endianness: Endianness) -> BasicDecoder<S> {
        match endianness {
            Endianness::LE => LE(LittleEndianBasicDecoder::default()),
            Endianness::BE => BE(BigEndianBasicDecoder::default()),
        }
    }
}

impl<S: ?Sized> fmt::Debug for BasicDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            LE(_) => write!(f, "BasicEncoder[LE]"),
            BE(_) => write!(f, "BasicEncoder[BE]"),
        }
    }
}

macro_rules! for_both {
    ($s: expr, $i: ident => $e: expr) => {
        match *$s {
            LE(ref $i) => $e,
            BE(ref $i) => $e
        }
    }
}

impl<S: ?Sized + Read> BasicDecode for BasicDecoder<S> {
    type Source = S;

    fn endianness(&self) -> Endianness {
        match *self {
            LE(_) => Endianness::LE,
            BE(_) => Endianness::BE,
        }
    }

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        for_both!(self, e => e.decode_us(source))
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        for_both!(self, e => e.decode_ul(source))
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        for_both!(self, e => e.decode_ss(source))
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        for_both!(self, e => e.decode_sl(source))
    }

    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32> {
        for_both!(self, e => e.decode_fl(source))
    }

    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64> {
        for_both!(self, e => e.decode_fd(source))
    }
}
