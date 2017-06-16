//! This module contains all DICOM data element encoding logic.
use error::Result;
use std::io::Write;
use data::DataElementHeader;
use util::Endianness;
use data::Tag;

pub mod basic;

/// Type trait for an encoder of basic data properties.
/// Unlike `Encode` (and similar to `BasicDecode`), this trait is not object
/// safe because it's better to just provide a dynamic implementation.
pub trait BasicEncode {

    /// Retrieve the encoder's endianness.
    fn endianness(&self) -> Endianness;

    /// Encode an unsigned short value to the given writer.
    fn encode_us<S>(&self, to: S, value: u16) -> Result<()> where S: Write;

    /// Encode an unsigned long value to the given writer.
    fn encode_ul<S>(&self, to: S, value: u32) -> Result<()> where S: Write;

    /// Encode a signed short value to the given writer.
    fn encode_ss<S>(&self, to: S, value: i16) -> Result<()> where S: Write;

    /// Encode a signed long value to the given writer.
    fn encode_sl<S>(&self, to: S, value: i32) -> Result<()> where S: Write;

    /// Encode a single precision float value to the given writer.
    fn encode_fl<S>(&self, to: S, value: f32) -> Result<()> where S: Write;

    /// Encode a double precision float value to the given writer.
    fn encode_fd<S>(&self, to: S, value: f64) -> Result<()> where S: Write;

    /// Perform
    #[inline]
    fn with_encoder<T, F1, F2>(&self, f_le: F1, f_be: F2) -> T
        where F1: FnOnce(self::basic::LittleEndianBasicEncoder) -> T,
              F2: FnOnce(self::basic::BigEndianBasicEncoder) -> T
    {
        match self.endianness() {
            Endianness::LE => f_le(self::basic::LittleEndianBasicEncoder),
            Endianness::BE => f_be(self::basic::BigEndianBasicEncoder)
        }
    }
}

/// Type trait for a data element encoder.
pub trait Encode {

    type Writer: ?Sized + Write;

    /// Encode and write an element tag.
    fn encode_tag(&self, to: &mut Self::Writer, tag: Tag) -> Result<()>;

    /// Encode and write a data element header to the given destination.
    /// Returns the number of bytes effectively written on success.
    fn encode_element_header(&self, to: &mut Self::Writer, de: DataElementHeader) -> Result<usize>;

    /// Encode and write a DICOM sequence item header to the given destination.
    /* Although item element headers are always a tag and length sequence regardless of TS,
     the encoding of the length is unknown at this level. So no default impl. */
    fn encode_item_header(&self, to: &mut Self::Writer, len: u32) -> Result<()>;
    
    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter(&self, to: &mut Self::Writer) -> Result<()> {
        self.encode_tag(to, Tag(0xFFFE, 0xE00D))?;
        to.write(&[0u8; 4])?;
        Ok(())
    }

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter(&self, to: &mut Self::Writer) -> Result<()> {
        self.encode_tag(to, Tag(0xFFFE, 0xE0DD))?;
        to.write(&[0u8; 4])?;
        Ok(())
    }
}
