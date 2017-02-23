//! This module contains all DICOM data element encoding logic.
use error::Result;
use std::io::Write;
use std::fmt::Debug;
use data::DataElementHeader;
use util::Endianness;
use attribute::tag::Tag;

pub mod basic;

/// Type trait for an encoder of basic data properties
pub trait BasicEncode: Debug {
    /// The encoding destination's data type.
    type Writer: Write + ?Sized;

    /// Retrieve the encoder's endianness.
    fn endianness(&self) -> Endianness;

    /// Encode an unsigned short value to the given writer.
    fn encode_us(&self, value: u16, to: &mut Self::Writer) -> Result<()>;

    /// Encode an unsigned long value to the given writer.
    fn encode_ul(&self, value: u32, to: &mut Self::Writer) -> Result<()>;

    /// Encode a signed short value to the given writer.
    fn encode_ss(&self, value: i16, to: &mut Self::Writer) -> Result<()>;

    /// Encode a signed long value to the given writer.
    fn encode_sl(&self, value: i32, to: &mut Self::Writer) -> Result<()>;

    /// Encode a single precision float value to the given writer.
    fn encode_fl(&self, value: f32, to: &mut Self::Writer) -> Result<()>;
    
    /// Encode a double precision float value to the given writer.
    fn encode_fd(&self, value: f64, to: &mut Self::Writer) -> Result<()>;
}

/// Type trait for a data element encoder.
pub trait Encode: BasicEncode + Debug {

    /// Encode and write an element tag.
    fn encode_tag(&self, tag: Tag, to: &mut Self::Writer) -> Result<()> {
        try!(self.encode_us(tag.group(), to));
        try!(self.encode_us(tag.element(), to));
        Ok(())
    }

    /// Encode and write a data element header to the given destination.
    /// Returns the number of bytes effectively written on success.
    fn encode_element_header(&self, de: DataElementHeader, to: &mut Self::Writer) -> Result<usize>;

    /// Encode and write a DICOM sequence item header to the given destination.
    fn encode_item_header(&self, len: u32, to: &mut Self::Writer) -> Result<()> {
        // Unlike other data element headers, item element headers are always
        // a tag and length sequence, without VR, regardless of TS.
        try!(self.encode_tag(Tag(0xFFFE, 0xE000), to));
        try!(self.encode_ul(len, to));
        Ok(())
    }

    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter(&self, to: &mut Self::Writer) -> Result<()> {
        try!(self.encode_tag(Tag(0xFFFE, 0xE00D), to));
        try!(self.encode_ul(0, to));
        Ok(())
    }

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter(&self, to: &mut Self::Writer) -> Result<()> {
        try!(self.encode_tag(Tag(0xFFFE, 0xE0DD), to));
        try!(self.encode_ul(0, to));
        Ok(())
    }
}
