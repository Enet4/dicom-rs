//! This module contains the type-erased version of a decoder.

use byteordered::Endianness;
use crate::error::Result;
use std::io::Read;
use dicom_core::header::{DataElementHeader, Length, SequenceItemHeader};
use dicom_core::Tag;

/** Type trait for reading and decoding basic data values from a data source.
 *
 * This trait aims to provide methods for reading binary numbers based on the
 * source's endianness.
 * This is the type-erased version of `super::BasicDecode`, where the data source type is not
 * known in compile time.
 */
pub trait BasicDecode {
    /// Retrieve the source's endianness, as expected by this decoder.
    fn endianness(&self) -> Endianness;

    /// Decode an unsigned short value from the given source.
    fn erased_decode_us(&self, source: &mut Read) -> Result<u16>;

    /// Decode an unsigned long value from the given source.
    fn erased_decode_ul(&self, source: &mut Read) -> Result<u32>;

    /// Decode an unsigned very long value from the given source.
    fn erased_decode_uv(&self, source: &mut Read) -> Result<u64>;

    /// Decode a signed short value from the given source.
    fn erased_decode_ss(&self, source: &mut Read) -> Result<i16>;

    /// Decode a signed long value from the given source.
    fn erased_decode_sl(&self, source: &mut Read) -> Result<i32>;

    /// Decode a signed very long value from the given source.
    fn erased_decode_sv(&self, source: &mut Read) -> Result<i64>;

    /// Decode a single precision float value from the given source.
    fn erased_decode_fl(&self, source: &mut Read) -> Result<f32>;

    /// Decode a double precision float value from the given source.
    fn erased_decode_fd(&self, source: &mut Read) -> Result<f64>;
}

/** Type trait for reading and decoding DICOM data elements.
 *
 * The specific behaviour of decoding, even when abstracted from the original source,
 * may depend on the given transfer syntax.
 *
 * This is the type-erased version of `super::Decode`, where the data source type is not
 * known in compile time. Users of this library should not need to rely on this level
 * directly, as the given implementations provide support for converting a generic decoder
 * to a type-erased decoder.
 */
pub trait Decode: BasicDecode {
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
        let tag = self.erased_decode_tag(&mut source)?;
        let len = self.erased_decode_ul(&mut source)?;
        let header = SequenceItemHeader::new(tag, Length(len))?;
        Ok(header)
    }

    /// Decode a DICOM attribute tag from the given source.
    fn erased_decode_tag(&self, source: &mut Read) -> Result<Tag> {
        let group = self.erased_decode_us(source)?;
        let elem = self.erased_decode_us(source)?;
        Ok(Tag(group, elem))
    }
}

impl<'s> super::BasicDecode for &'s BasicDecode {
    fn endianness(&self) -> Endianness {
        (**self).endianness()
    }

    fn decode_us<S>(&self, mut source: S) -> Result<u16>
    where
        S: Read,
    {
        (**self).erased_decode_us(&mut source)
    }

    fn decode_ul<S>(&self, mut source: S) -> Result<u32>
    where
        S: Read,
    {
        (**self).erased_decode_ul(&mut source)
    }

    fn decode_uv<S>(&self, mut source: S) -> Result<u64>
    where
        S: Read,
    {
        (**self).erased_decode_uv(&mut source)
    }

    fn decode_ss<S>(&self, mut source: S) -> Result<i16>
    where
        S: Read,
    {
        (**self).erased_decode_ss(&mut source)
    }

    fn decode_sl<S>(&self, mut source: S) -> Result<i32>
    where
        S: Read,
    {
        (**self).erased_decode_sl(&mut source)
    }

    fn decode_sv<S>(&self, mut source: S) -> Result<i64>
    where
        S: Read,
    {
        (**self).erased_decode_sv(&mut source)
    }

    fn decode_fl<S>(&self, mut source: S) -> Result<f32>
    where
        S: Read,
    {
        (**self).erased_decode_fl(&mut source)
    }

    fn decode_fd<S>(&self, mut source: S) -> Result<f64>
    where
        S: Read,
    {
        (**self).erased_decode_fd(&mut source)
    }
}
