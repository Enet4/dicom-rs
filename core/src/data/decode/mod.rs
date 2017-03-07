//! This module contains all DICOM data element decoding logic.

use transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
use transfer_syntax::implicit_le::{ImplicitVRLittleEndianDecoder, StandardImplicitVRLittleEndianDecoder};
use std::io::Read;
use error::Result;
use data::{DataElementHeader, SequenceItemHeader};
use util::Endianness;
use data::Tag;

pub mod erased;
pub mod basic;

/** Obtain the default data element decoder.
 * According to the standard, data elements are encoded in Implicit
 * VR Little Endian by default.
 */
pub fn get_default_reader<'s, S: 's + ?Sized>() -> StandardImplicitVRLittleEndianDecoder<S>
    where S: Read
{
    ImplicitVRLittleEndianDecoder::default()
}

/** Obtain a data element decoder for reading the data elements in a DICOM
 * file's Meta information. According to the standard, these are always
 * encoded in Explicit VR Little Endian.
 */
pub fn get_file_header_decoder<'s, S: Read + ?Sized + 's>() -> ExplicitVRLittleEndianDecoder<S> {
    ExplicitVRLittleEndianDecoder::default()
}

/** Type trait for reading and decoding basic data values from a data source.
 * 
 * This trait aims to provide methods for reading binary numbers based on the
 * source's endianness.
 */
pub trait BasicDecode {
    /// The data source's type.
    type Source: ?Sized + Read;

    /// Retrieve the source's endianness, as expected by this decoder.
    fn endianness(&self) -> Endianness;

    /// Decode an unsigned short value from the given source.
    fn decode_us(&self, source: &mut Self::Source) -> Result<u16>;

    /// Decode an unsigned long value from the given source.
    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32>;

    /// Decode a signed short value from the given source.
    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16>;

    /// Decode a signed long value from the given source.
    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32>;

    /// Decode a single precision float value from the given source.
    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32>;

    /// Decode a double precision float value from the given source.
    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64>;

    /// Decode a DICOM attribute tag from the given source.
    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        let g = try!(self.decode_us(source));
        let e = try!(self.decode_us(source));
        Ok(Tag(g, e))
    }
}

impl<T: ?Sized> BasicDecode for Box<T> where T: BasicDecode {
    type Source = <T as BasicDecode>::Source;

    fn endianness(&self) -> Endianness { self.as_ref().endianness() }

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        self.as_ref().decode_us(source)
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        self.as_ref().decode_ul(source)
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        self.as_ref().decode_ss(source)
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        self.as_ref().decode_sl(source)
    }

    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32> {
        self.as_ref().decode_fl(source)
    }

    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64> {
        self.as_ref().decode_fd(source)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        self.as_ref().decode_tag(source)
    }
}

impl<'a, T: ?Sized> BasicDecode for &'a T where T: BasicDecode {
    type Source = <T as BasicDecode>::Source;

    fn endianness(&self) -> Endianness { (*self).endianness() }

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        (*self).decode_us(source)
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        (*self).decode_ul(source)
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        (*self).decode_ss(source)
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        (*self).decode_sl(source)
    }

    fn decode_fl(&self, source: &mut Self::Source) -> Result<f32> {
        (*self).decode_fl(source)
    }

    fn decode_fd(&self, source: &mut Self::Source) -> Result<f64> {
        (*self).decode_fd(source)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        (*self).decode_tag(source)
    }
}

impl<'s, S: 's + ?Sized> From<Endianness> for Box<BasicDecode<Source = S> + 's>
    where S: Read
{

    fn from(endianness: Endianness) -> Box<BasicDecode<Source = S> + 's> {
        match endianness {
            Endianness::LE => Box::new(basic::LittleEndianBasicDecoder::default()),
            Endianness::BE => Box::new(basic::BigEndianBasicDecoder::default())
        }
    }
}


/** Type trait for reading and decoding DICOM data elements.
 * 
 * The specific behaviour of decoding, even when abstracted from the original source,
 * may depend on the transfer syntax.
 */
pub trait Decode {
    /// The data source's type.
    type Source: ?Sized + Read;

    /** Fetch and decode the next data element header from the given source.
     * This method returns only the header of the element. At the end of this operation, the source
     * will be pointing at the element's value data, which should be read or skipped as necessary.
     */
    fn decode_header(&self, source: &mut Self::Source) -> Result<DataElementHeader>;

    /** Fetch and decode the next sequence item head from the given source.
     * This method returns only the header of the item. At the end of this operation, the source
     * will be pointing at the beginning of the item's data, which should be traversed if necessary.
     */
    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader>;

    /// Decode a DICOM attribute tag from the given source.
    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag>;
}

impl<T: ?Sized> Decode for Box<T> where T: Decode {
    type Source = <T as Decode>::Source;

    fn decode_header(&self, source: &mut Self::Source) -> Result<DataElementHeader> {
        self.as_ref().decode_header(source)
    }

    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader> {
        self.as_ref().decode_item_header(source)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        self.as_ref().decode_tag(source)
    }
}

impl<'a, T: ?Sized> Decode for &'a T where T: Decode {
    type Source = <T as Decode>::Source;

    fn decode_header(&self, source: &mut Self::Source) -> Result<DataElementHeader> {
        (*self).decode_header(source)
    }

    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader> {
        (*self).decode_item_header(source)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        (*self).decode_tag(source)
    }
}
