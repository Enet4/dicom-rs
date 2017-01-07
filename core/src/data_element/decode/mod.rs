//! This module contains all DICOM data element decoding logic.

use transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
use transfer_syntax::implicit_le::ImplicitVRLittleEndianDecoder;
use std::io::Read;
use error::Result;
use data_element::{DataElementHeader, SequenceItemHeader};
use std::fmt::Debug;
use util::Endianness;
use attribute::tag::Tag;

pub mod erased;
pub mod basic;

/** Obtain the default data element decoder.
 * According to the standard, data elements are encoded in Implicit
 * VR Little Endian by default.
 */
pub fn get_default_reader<'s, S: Read + ?Sized + 's>
    ()
    -> ImplicitVRLittleEndianDecoder<'static, S>
{
    ImplicitVRLittleEndianDecoder::with_default_dict()
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
pub trait BasicDecode: Debug {
    /** The data source's type. */
    type Source: Read + ?Sized;

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
}

/** Type trait for reading and decoding DICOM data elements.
 * 
 * The specific behaviour of decoding, even when abstracted from the original source,
 * may depend on the transfer syntax.
 */
pub trait Decode: BasicDecode {
    /** Fetch and decode the next data element header from the given source.
     * This method returns only the header of the element. At the end of this operation, the source
     * will be pointing at the element's value data, which should be read or skipped as necessary.
     */
    fn decode_header(&self, source: &mut Self::Source) -> Result<DataElementHeader>;

    /** Fetch and decode the next sequence item head from the given source.
     * This method returns only the header of the item. At the end of this operation, the source
     * will be pointing at the beginning of the item's data, which should be traversed if necessary.
     */
    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader> {
        let tag = try!(self.decode_tag(source));
        let len = try!(self.decode_ul(source));
        SequenceItemHeader::new(tag, len)
    }

    /// Decode a DICOM attribute tag from the given source.
    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        let group = try!(self.decode_us(source));
        let elem = try!(self.decode_us(source));
        Ok(Tag(group, elem))
    }
}
