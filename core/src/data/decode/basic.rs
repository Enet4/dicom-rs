//! This module provides implementations for basic decoders: little endian and big endian.
//! 

use super::BasicDecode;
use std::marker::PhantomData;
use byteorder::{ByteOrder, LittleEndian, BigEndian};
use error::Result;
use util::Endianness;
use std::io::Read;
use std::fmt;

/// A basic decoder of DICOM primitive elements in little endian.
pub struct LittleEndianBasicDecoder<S: Read + ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: ?Sized + Read> fmt::Debug for LittleEndianBasicDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LittleEndianBasicDecoder")
    }
}

impl<S: Read + ?Sized> Default for LittleEndianBasicDecoder<S> {
    fn default() -> LittleEndianBasicDecoder<S> {
        LittleEndianBasicDecoder {
            phantom: PhantomData::default()
        }
    }
}


impl<S: Read + ?Sized> BasicDecode for LittleEndianBasicDecoder<S> {
    type Source = S;

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        let mut buf = [0u8; 2];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_u16(&buf[..]))
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_u32(&buf[..]))
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        let mut buf = [0u8; 2];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_i16(&buf[..]))
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_i32(&buf[..]))
    }

    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_f32(&buf[..]))
    }

    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64> {
        let mut buf = [0u8; 8];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_f64(&buf[..]))
    }
}

/// A basic decoder of DICOM primitive elements in big endian.
pub struct BigEndianBasicDecoder<S: Read + ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: Read + ?Sized> fmt::Debug for BigEndianBasicDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BigEndianBasicDecoder")
    }
}

impl<S: Read + ?Sized> Default for BigEndianBasicDecoder<S> {
    fn default() -> BigEndianBasicDecoder<S> {
        BigEndianBasicDecoder {
            phantom: PhantomData::default()
        }
    }
}

impl<S: Read + ?Sized> BasicDecode for BigEndianBasicDecoder<S> {
    type Source = S;

    fn endianness(&self) -> Endianness {
        Endianness::BE
    }

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        let mut buf = [0u8; 2];
        try!(source.read_exact(&mut buf[..]));
        Ok(BigEndian::read_u16(&buf[..]))
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(BigEndian::read_u32(&buf[..]))
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        let mut buf = [0u8; 2];
        try!(source.read_exact(&mut buf[..]));
        Ok(BigEndian::read_i16(&buf[..]))
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(BigEndian::read_i32(&buf[..]))
    }

    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(BigEndian::read_f32(&buf[..]))
    }

    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64> {
        let mut buf = [0u8; 8];
        try!(source.read_exact(&mut buf[..]));
        Ok(BigEndian::read_f64(&buf[..]))
    }
}
