//! This module provides implementations for basic encoders: little endian and big endian.
//! 

use super::BasicEncode;
use std::marker::PhantomData;
use byteorder::{ByteOrder, LittleEndian, BigEndian};
use error::Result;
use util::Endianness;
use std::io::Write;
use std::fmt;

/// A basic encoder of primitive elements in little endian.
pub struct LittleEndianBasicEncoder<S: Write + ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: Write + ?Sized> fmt::Debug for LittleEndianBasicEncoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LittleEndianBasicEncoder")
    }
}

impl<S: Write + ?Sized> Default for LittleEndianBasicEncoder<S> {
    fn default() -> LittleEndianBasicEncoder<S> {
        LittleEndianBasicEncoder {
            phantom: PhantomData::default()
        }
    }
}


impl<'s, S: Write + ?Sized + 's> BasicEncode for LittleEndianBasicEncoder<S> {
    type Writer = S;

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn encode_us(&self, value: u16, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 2];
        LittleEndian::write_u16(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_ul(&self, value: u32, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        LittleEndian::write_u32(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_ss(&self, value: i16, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 2];
        LittleEndian::write_i16(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_sl(&self, value: i32, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        LittleEndian::write_i32(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_fl(&self, value: f32, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        LittleEndian::write_f32(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }
    
    fn encode_fd(&self, value: f64, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        LittleEndian::write_f64(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }
}

/// A basic encoder of DICOM primitive elements in big endian.
pub struct BigEndianBasicEncoder<S: Write + ?Sized> {
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

impl<'s, S: Write + ?Sized + 's> BasicEncode for BigEndianBasicEncoder<S> {
    type Writer = S;

    fn endianness(&self) -> Endianness {
        Endianness::BE
    }

    fn encode_us(&self, value: u16, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 2];
        BigEndian::write_u16(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_ul(&self, value: u32, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        BigEndian::write_u32(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_ss(&self, value: i16, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 2];
        BigEndian::write_i16(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_sl(&self, value: i32, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        BigEndian::write_i32(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_fl(&self, value: f32, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        BigEndian::write_f32(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }
    
    fn encode_fd(&self, value: f64, to: &mut Self::Writer) -> Result<()> {
        let mut buf = [0u8; 4];
        BigEndian::write_f64(&mut buf[..], value);
        try!(to.write_all(&buf));
        Ok(())
    }

}
