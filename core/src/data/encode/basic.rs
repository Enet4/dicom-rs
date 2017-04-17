//! This module provides implementations for basic encoders: little endian and big endian.
//! 

use super::BasicEncode;
use std::marker::PhantomData;
use byteorder::{LittleEndian, BigEndian, WriteBytesExt};
use error::Result;
use util::Endianness;
use std::io::Write;
use std::fmt;

/// A basic encoder of primitive elements in little endian.
pub struct LittleEndianBasicEncoder<S: ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: ?Sized> fmt::Debug for LittleEndianBasicEncoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LittleEndianBasicEncoder")
    }
}

impl<S: ?Sized> Default for LittleEndianBasicEncoder<S> {
    fn default() -> LittleEndianBasicEncoder<S> {
        LittleEndianBasicEncoder {
            phantom: PhantomData::default()
        }
    }
}

impl<S: Write + ?Sized> BasicEncode for LittleEndianBasicEncoder<S> {
    type Writer = S;

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn encode_us(&self, value: u16, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_u16::<LittleEndian>(value)?)
    }

    fn encode_ul(&self, value: u32, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_u32::<LittleEndian>(value)?)
    }

    fn encode_ss(&self, value: i16, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_i16::<LittleEndian>(value)?)
    }

    fn encode_sl(&self, value: i32, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_i32::<LittleEndian>(value)?)
    }

    fn encode_fl(&self, value: f32, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_f32::<LittleEndian>(value)?)
    }
    
    fn encode_fd(&self, value: f64, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_f64::<LittleEndian>(value)?)
    }
}

/// A basic encoder of DICOM primitive elements in big endian.
pub struct BigEndianBasicEncoder<S: ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: Write + ?Sized> fmt::Debug for BigEndianBasicEncoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BigEndianBasicEncoder")
    }
}

impl<S: Write + ?Sized> Default for BigEndianBasicEncoder<S> {
    fn default() -> BigEndianBasicEncoder<S> {
        BigEndianBasicEncoder {
            phantom: PhantomData::default()
        }
    }
}

impl<S: Write + ?Sized> BasicEncode for BigEndianBasicEncoder<S> {
    type Writer = S;

    fn endianness(&self) -> Endianness {
        Endianness::BE
    }

    fn encode_us(&self, value: u16, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_u16::<BigEndian>(value)?)
    }

    fn encode_ul(&self, value: u32, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_u32::<BigEndian>(value)?)
    }

    fn encode_ss(&self, value: i16, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_i16::<BigEndian>(value)?)
    }

    fn encode_sl(&self, value: i32, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_i32::<BigEndian>(value)?)
    }

    fn encode_fl(&self, value: f32, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_f32::<BigEndian>(value)?)
    }
    
    fn encode_fd(&self, value: f64, to: &mut Self::Writer) -> Result<()> {
        Ok(to.write_f64::<BigEndian>(value)?)
    }

}

/// A basic encoder with support for both Little Endian an Big Endian
/// encoding, decided at run-time. Since only two values are possible,
/// this enum may become more efficient than the use of a trait object.
pub enum BasicEncoder<S: ?Sized> {
    LE(LittleEndianBasicEncoder<S>),
    BE(BigEndianBasicEncoder<S>)
}

use self::BasicEncoder::{LE, BE};

impl<S: ?Sized> fmt::Debug for BasicEncoder<S> {
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

impl<S: ?Sized + Write> BasicEncode for BasicEncoder<S> {
    type Writer = S;

    fn endianness(&self) -> Endianness {
        match *self {
            LE(_) => Endianness::LE,
            BE(_) => Endianness::BE
        }
    }

    fn encode_us(&self, value: u16, to: &mut Self::Writer) -> Result<()> {
        for_both!(self, e => e.encode_us(value, to))
    }

    fn encode_ul(&self, value: u32, to: &mut Self::Writer) -> Result<()> {
        for_both!(self, e => e.encode_ul(value, to))
    }

    fn encode_ss(&self, value: i16, to: &mut Self::Writer) -> Result<()> {
        for_both!(self, e => e.encode_ss(value, to))
    }

    fn encode_sl(&self, value: i32, to: &mut Self::Writer) -> Result<()> {
        for_both!(self, e => e.encode_sl(value, to))
    }

    fn encode_fl(&self, value: f32, to: &mut Self::Writer) -> Result<()> {
        for_both!(self, e => e.encode_fl(value, to))
    }
    
    fn encode_fd(&self, value: f64, to: &mut Self::Writer) -> Result<()> {
        for_both!(self, e => e.encode_fd(value, to))
    }

}