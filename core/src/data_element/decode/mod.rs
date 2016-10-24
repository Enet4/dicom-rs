//! This module contains all DICOM data element decoding logic.

use transfer_syntax::TransferSyntax;
use super::explicit_le::ExplicitVRLittleEndianDecoder;
use super::explicit_be::ExplicitVRBigEndianDecoder;
use super::implicit_le::ImplicitVRLittleEndianDecoder;
use std::io::Read;
use error::Result;
use data_element::{DataElementHeader, SequenceItemHeader};
use std::fmt::Debug;

pub mod erased;

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

/** Dynamically retrieve the appropriate decoder for the given transfer syntax and source type.
 */
pub fn get_decoder<'s, S: Read + ?Sized + 's>(ts: TransferSyntax)
                                              -> Option<Box<Decode<Source = S> + 's>> {
    match ts {
        TransferSyntax::ImplicitVRLittleEndian => {
            Some(Box::new(ImplicitVRLittleEndianDecoder::<S>::with_default_dict()))
        }
        TransferSyntax::ExplicitVRLittleEndian => {
            Some(Box::new(ExplicitVRLittleEndianDecoder::<S>::default()))
        }
        TransferSyntax::ExplicitVRBigEndian => {
            Some(Box::new(ExplicitVRBigEndianDecoder::<S>::default()))
        }
        _ => None,
    }
}

/** Type trait for reading and decoding DICOM data elements.
 * 
 * The specific behaviour of decoding, even when abstracted from the original source,
 * may depend on the transfer syntax.
 */
pub trait Decode: Debug {
    /** The data source's type. */
    type Source: Read + ?Sized;

    /** Fetch and decode the next data element header from the given source.
     * This method returns only the header of the element. At the end of this operation, the source
     * will be pointing at the element's value data, which should be read or skipped as necessary.
     */
    fn decode(&self, source: &mut Self::Source) -> Result<DataElementHeader>;

    /** Fetch and decode the next sequence item head from the given source.
     * This method returns only the header of the item. At the end of this operation, the source
     * will be pointing at the beginning of the item's data, which should be traversed if necessary.
     */
    fn decode_item(&self, source: &mut Self::Source) -> Result<SequenceItemHeader>;
}

impl<'s> Decode for &'s erased::Decode {
    type Source = Read;

    fn decode(&self, mut source: &mut Self::Source) -> Result<DataElementHeader> {
        (**self).erased_decode(&mut source)
    }

    fn decode_item(&self, mut source: &mut Self::Source) -> Result<SequenceItemHeader> {
        (**self).erased_decode_item(&mut source)
    }
}
