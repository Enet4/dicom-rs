//! This module contains the type-erased version of a decoder.

use std::io::Read;
use error::Result;
use data_element::{DataElementHeader, SequenceItemHeader};
use std::fmt::Debug;
use util::Endianness;
use attribute::tag::Tag;

/** Type trait for reading and decoding basic data values from a data source.
 * 
 * This trait aims to provide methods for reading binary numbers based on the
 * source's endianness.
 * This is the type-erased version of `super::BasicDecode`, where the data source type is not
 * known in compile time. 
 */
pub trait BasicDecode: Debug {
    /// Retrieve the source's endianness, as expected by this decoder.
    fn endianness(&self) -> Endianness;

    /// Decode an unsigned short value from the given source.
    fn erased_decode_us(&self, source: &mut Read) -> Result<u16>;

    /// Decode an unsigned long value from the given source.
    fn erased_decode_ul(&self, source: &mut Read) -> Result<u32>;

    /// Decode a signed short value from the given source.
    fn erased_decode_ss(&self, source: &mut Read) -> Result<i16>;

    /// Decode a signed long value from the given source.
    fn erased_decode_sl(&self, source: &mut Read) -> Result<i32>;

    /// Decode a single precision float value from the given source.
    fn erased_decode_fl(&self, source: &mut Read) -> Result<f32>;
    
    /// Decode a double precision float value from the given source.
    fn erased_decode_fd(&self, source: &mut Read) -> Result<f64>;
}

/** Type trait for reading and decoding DICOM data elements.
 * 
 * The specific behaviour of decoding, even when abstracted from the original source,
 * may depend on the given transfer syntax. As each element is retrieved, a temporary
 * cursor to the data is obtained, allowing for an optional reading of the full contents
 * of the element.
 * 
 * This is the type-erased version of `super::Decode`, where the data source type is not
 * known in compile time. Users of this library should not need to rely on this level
 * directly, as the given implementations provide support for converting a generic decoder
 * to a type-erased decoder and vice versa.
 */
pub trait Decode: BasicDecode + Debug {

    /** Fetch and decode the next data element header from the given source.
     * This method returns only the header of the element. At the end of this operation, the source
     * will be pointing at the element's value data, which should be read or skipped as necessary.
     */
    fn erased_decode(&self, source: &mut Read) -> Result<DataElementHeader>;

    /** Fetch and decode the next sequence item head from the given source.
     * This method returns only the header of the item. At the end of this operation, the source
     * will be pointing at the beginning of the item's data, which should be traversed if necessary.
     */
    fn erased_decode_item(&self, mut source: &mut Read) -> Result<SequenceItemHeader> {
        let tag = try!(self.erased_decode_tag(&mut source));
        let len = try!(self.erased_decode_ul(&mut source));
        SequenceItemHeader::new(tag, len)
    }

    /// Decode a DICOM attribute tag from the given source.
    fn erased_decode_tag(&self, source: &mut Read) -> Result<Tag> {
        let group = try!(self.erased_decode_us(source));
        let elem = try!(self.erased_decode_us(source));
        Ok(Tag(group, elem))
    }
}

impl<'s> super::BasicDecode for &'s BasicDecode  {
    type Source = Read;

    fn endianness(&self) -> Endianness {
        (**self).endianness()
    }

    fn decode_us(&self, mut source: &mut Self::Source) -> Result<u16> {
        (**self).erased_decode_us(&mut source)
    }

    fn decode_ul(&self, mut source: &mut Self::Source) -> Result<u32> {
        (**self).erased_decode_ul(&mut source)
    }

    fn decode_ss(&self, mut source: &mut Self::Source) -> Result<i16> {
        (**self).erased_decode_ss(&mut source)
    }

    fn decode_sl(&self, mut source: &mut Self::Source) -> Result<i32> {
        (**self).erased_decode_sl(&mut source)
    }

    fn decode_fl(&self, mut source: &mut Self::Source) -> Result<f32> {
        (**self).erased_decode_fl(&mut source)
    }

    fn decode_fd(&self, mut source: &mut Self::Source) -> Result<f64> {
        (**self).erased_decode_fd(&mut source)
    }
}

impl<'s> super::BasicDecode for &'s Decode {
    type Source = Read;

    fn endianness(&self) -> Endianness {
        (**self).endianness()
    }

    fn decode_us(&self, mut source: &mut Self::Source) -> Result<u16> {
        (**self).erased_decode_us(&mut source)
    }

    fn decode_ul(&self, mut source: &mut Self::Source) -> Result<u32> {
        (**self).erased_decode_ul(&mut source)
    }

    fn decode_ss(&self, mut source: &mut Self::Source) -> Result<i16> {
        (**self).erased_decode_ss(&mut source)
    }

    fn decode_sl(&self, mut source: &mut Self::Source) -> Result<i32> {
        (**self).erased_decode_sl(&mut source)
    }

    fn decode_fl(&self, mut source: &mut Self::Source) -> Result<f32> {
        (**self).erased_decode_fl(&mut source)
    }

    fn decode_fd(&self, mut source: &mut Self::Source) -> Result<f64> {
        (**self).erased_decode_fd(&mut source)
    }
}

impl<'s> super::Decode for &'s Decode {

    fn decode_header(&self, mut source: &mut Self::Source) -> Result<DataElementHeader> {
        (**self).erased_decode(&mut source)
    }

    fn decode_item_header(&self, mut source: &mut Self::Source) -> Result<SequenceItemHeader> {
        (**self).erased_decode_item(&mut source)
    }

    fn decode_tag(&self, mut source: &mut Self::Source) -> Result<Tag> {
        (**self).erased_decode_tag(&mut source)
    }
}
