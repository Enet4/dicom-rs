//! This module contains all DICOM data element decoding logic.

use byteordered::Endianness;
use transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
use transfer_syntax::implicit_le::{ImplicitVRLittleEndianDecoder,
                                   StandardImplicitVRLittleEndianDecoder};
use std::io::Read;
use error::Result;
use dicom_core::header::{DataElementHeader, SequenceItemHeader};
use dicom_core::Tag;

pub mod erased;
pub mod basic;

/** Obtain the default data element decoder.
 * According to the standard, data elements are encoded in Implicit
 * VR Little Endian by default.
 */
pub fn get_default_reader<S>() -> StandardImplicitVRLittleEndianDecoder<S>
where
    S: Read,
{
    ImplicitVRLittleEndianDecoder::default()
}

/** Obtain a data element decoder for reading the data elements in a DICOM
 * file's Meta information. According to the standard, these are always
 * encoded in Explicit VR Little Endian.
 */
pub fn get_file_header_decoder<S>() -> ExplicitVRLittleEndianDecoder<S>
where
    S: Read,
{
    ExplicitVRLittleEndianDecoder::default()
}

/** Type trait for reading and decoding basic data values from a data source.
 *
 * This trait aims to provide methods for reading binary numbers based on the
 * source's endianness. Unlike `Decode`, this trait is not object safe.
 * However, it doesn't have to because there are, and only will be, two
 * possible implementations (`LittleEndianBasicDecoder` and
 * `BigEndianBasicDecoder`).
 */
pub trait BasicDecode {
    /// Retrieve the source's endianness, as expected by this decoder.
    fn endianness(&self) -> Endianness;

    /// Decode an unsigned short value from the given source.
    fn decode_us<S>(&self, source: S) -> Result<u16>
    where
        S: Read;

    /// Decode an unsigned long value from the given source.
    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read;

    /// Decode a signed short value from the given source.
    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read;

    /// Decode a signed long value from the given source.
    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read;

    /// Decode a single precision float value from the given source.
    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read;

    /// Decode a double precision float value from the given source.
    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read;

    /// Decode a DICOM attribute tag from the given source.
    fn decode_tag<S>(&self, mut source: S) -> Result<Tag>
    where
        S: Read,
    {
        let g = self.decode_us(&mut source)?;
        let e = self.decode_us(source)?;
        Ok(Tag(g, e))
    }
}

impl<T: ?Sized> BasicDecode for Box<T>
where
    T: BasicDecode,
{
    fn endianness(&self) -> Endianness {
        self.as_ref().endianness()
    }

    fn decode_us<S>(&self, source: S) -> Result<u16>
    where
        S: Read,
    {
        (**self).decode_us(source)
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        (**self).decode_ul(source)
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        (**self).decode_ss(source)
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        (**self).decode_sl(source)
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        (**self).decode_fl(source)
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        (**self).decode_fd(source)
    }

    fn decode_tag<S>(&self, source: S) -> Result<Tag>
    where
        S: Read,
    {
        (**self).decode_tag(source)
    }
}

impl<'a, T: ?Sized> BasicDecode for &'a T
where
    T: BasicDecode,
{
    fn endianness(&self) -> Endianness {
        (*self).endianness()
    }

    fn decode_us<S>(&self, source: S) -> Result<u16>
    where
        S: Read,
    {
        (**self).decode_us(source)
    }

    fn decode_ul<S>(&self, source: S) -> Result<u32>
    where
        S: Read,
    {
        (**self).decode_ul(source)
    }

    fn decode_ss<S>(&self, source: S) -> Result<i16>
    where
        S: Read,
    {
        (**self).decode_ss(source)
    }

    fn decode_sl<S>(&self, source: S) -> Result<i32>
    where
        S: Read,
    {
        (**self).decode_sl(source)
    }

    fn decode_fl<S>(&self, source: S) -> Result<f32>
    where
        S: Read,
    {
        (**self).decode_fl(source)
    }

    fn decode_fd<S>(&self, source: S) -> Result<f64>
    where
        S: Read,
    {
        (**self).decode_fd(source)
    }

    fn decode_tag<S>(&self, source: S) -> Result<Tag>
    where
        S: Read,
    {
        (**self).decode_tag(source)
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
     * 
     * Decoding an item or sequence delimiter is considered valid, and so should be properly handled
     * by the decoder. The value representation in this case should be `UN`.
     */
    fn decode_header(&self, source: &mut Self::Source) -> Result<DataElementHeader>;

    /** Fetch and decode the next sequence item head from the given source. It is a separate method
     * because value representation is always implicit when reading item headers and delimiters. 
     * This method returns only the header of the item. At the end of this operation, the source
     * will be pointing at the beginning of the item's data, which should be traversed if necessary.
     */
    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader>;

    /// Decode a DICOM attribute tag from the given source.
    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag>;
}

impl<T: ?Sized> Decode for Box<T>
where
    T: Decode,
{
    type Source = <T as Decode>::Source;

    fn decode_header(&self, source: &mut Self::Source) -> Result<DataElementHeader> {
        (**self).decode_header(source)
    }

    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader> {
        (**self).decode_item_header(source)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        (**self).decode_tag(source)
    }
}

impl<'a, T: ?Sized> Decode for &'a T
where
    T: Decode,
{
    type Source = <T as Decode>::Source;

    fn decode_header(&self, source: &mut Self::Source) -> Result<DataElementHeader> {
        (**self).decode_header(source)
    }

    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader> {
        (**self).decode_item_header(source)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        (**self).decode_tag(source)
    }
}
