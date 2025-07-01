//! Implicit VR Big Endian syntax transfer implementation

use crate::decode::basic::LittleEndianBasicDecoder;
use crate::decode::{
    BadSequenceHeaderSnafu, BasicDecode, DecodeFrom, ReadHeaderTagSnafu, ReadLengthSnafu,
    ReadTagSnafu, Result,
};
use crate::Decode;
use byteordered::byteorder::{ByteOrder, LittleEndian};
use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry};
use dicom_core::header::{DataElementHeader, Length, SequenceItemHeader};
use dicom_core::{Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use snafu::ResultExt;
use std::fmt;
use std::io::Read;

/// An ImplicitVRLittleEndianDecoder which uses the standard data dictionary.
pub type StandardImplicitVRLittleEndianDecoder =
    ImplicitVRLittleEndianDecoder<StandardDataDictionary>;

/// A data element decoder for the Explicit VR Little Endian transfer syntax.
/// This type contains a reference to an attribute dictionary for resolving
/// value representations.
pub struct ImplicitVRLittleEndianDecoder<D> {
    dict: D,
    basic: LittleEndianBasicDecoder,
}

impl<D> fmt::Debug for ImplicitVRLittleEndianDecoder<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ImplicitVRLittleEndianDecoder")
            .field("dict", &"«omitted»")
            .field("basic", &self.basic)
            .finish()
    }
}

impl ImplicitVRLittleEndianDecoder<StandardDataDictionary> {
    /// Retrieve this decoder using the standard data dictionary.
    pub fn with_std_dict() -> Self {
        ImplicitVRLittleEndianDecoder {
            dict: StandardDataDictionary,
            basic: LittleEndianBasicDecoder,
        }
    }

    /// Retrieve this decoder using the standard data dictionary.
    pub fn new() -> Self {
        Self::with_std_dict()
    }
}

impl Default for ImplicitVRLittleEndianDecoder<StandardDataDictionary> {
    fn default() -> Self {
        ImplicitVRLittleEndianDecoder::with_std_dict()
    }
}

impl<D> ImplicitVRLittleEndianDecoder<D>
where
    D: DataDictionary,
{
    /// Retrieve this decoder using a custom data dictionary.
    pub fn with_dict(dictionary: D) -> Self {
        ImplicitVRLittleEndianDecoder {
            dict: dictionary,
            basic: LittleEndianBasicDecoder,
        }
    }
}

impl<D> Decode for ImplicitVRLittleEndianDecoder<D>
where
    D: DataDictionary,
{
    fn decode_header<S>(&self, mut source: &mut S) -> Result<(DataElementHeader, usize)>
    where
        S: ?Sized + Read,
    {
        // retrieve tag
        let tag = self
            .basic
            .decode_tag(&mut source)
            .context(ReadHeaderTagSnafu)?;

        let mut buf = [0u8; 4];
        source.read_exact(&mut buf).context(ReadLengthSnafu)?;
        let len = LittleEndian::read_u32(&buf);

        // VR resolution is done with the help of the data dictionary.
        // In Implicit VR Little Endian,
        // the VR of OW must be used for Pixel Data (7FE0,0010)
        // and Overlay Data (60xx, 3000).
        // This edge case is addressed manually here.
        let vr = if tag == Tag(0x7FE0, 0x0010) || (tag.0 >> 8 == 0x60 && tag.1 == 0x3000) {
            VR::OW
        } else {
            self.dict
                .by_tag(tag)
                .map(|entry| entry.vr().relaxed())
                .unwrap_or(VR::UN)
        };
        Ok((DataElementHeader::new(tag, vr, Length(len)), 8))
    }

    fn decode_item_header<S>(&self, mut source: &mut S) -> Result<SequenceItemHeader>
    where
        S: ?Sized + Read,
    {
        let mut buf = [0u8; 4];

        // retrieve tag
        let tag = self
            .basic
            .decode_tag(&mut source)
            .context(ReadHeaderTagSnafu)?;

        source.read_exact(&mut buf).context(ReadLengthSnafu)?;
        let len = LittleEndian::read_u32(&buf);
        SequenceItemHeader::new(tag, Length(len)).context(BadSequenceHeaderSnafu)
    }

    #[inline]
    fn decode_tag<S>(&self, source: &mut S) -> Result<Tag>
    where
        S: ?Sized + Read,
    {
        self.basic.decode_tag(source).context(ReadTagSnafu)
    }
}

impl<S: ?Sized, D> DecodeFrom<S> for ImplicitVRLittleEndianDecoder<D>
where
    S: Read,
    D: DataDictionary,
{
    #[inline]
    fn decode_header(&self, source: &mut S) -> Result<(DataElementHeader, usize)> {
        Decode::decode_header(self, source)
    }

    #[inline]
    fn decode_item_header(&self, source: &mut S) -> Result<SequenceItemHeader> {
        Decode::decode_item_header(self, source)
    }

    #[inline]
    fn decode_tag(&self, source: &mut S) -> Result<Tag> {
        Decode::decode_tag(self, source)
    }
}

#[cfg(test)]
mod tests {
    use super::ImplicitVRLittleEndianDecoder;
    use crate::decode::Decode;
    use dicom_core::dictionary::stub::StubDataDictionary;
    use dicom_core::header::{HasLength, Header, Length, VR};
    use dicom_core::Tag;
    use std::io::{Cursor, Read, Seek, SeekFrom};

    // manually crafting some DICOM data elements
    //   Tag: (0002,0002) Media Storage SOP Class UID
    //   Length: 26
    //   Value: "1.2.840.10008.5.1.4.1.1.1" (with 1 padding '\0')
    // --
    //   Tag: (0002,0010) Transfer Syntax UID
    //   Length: 20
    //   Value: "1.2.840.10008.1.2.1" (w 1 padding '\0') == ExplicitVRLittleEndian
    // --
    const RAW: &[u8; 62] = &[
        0x02, 0x00, 0x02, 0x00, 0x1a, 0x00, 0x00, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30,
        0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e,
        0x31, 0x2e, 0x31, 0x00, 0x02, 0x00, 0x10, 0x00, 0x14, 0x00, 0x00, 0x00, 0x31, 0x2e, 0x32,
        0x2e, 0x38, 0x34, 0x30, 0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e,
        0x31, 0x00,
    ];

    const DICT: &StubDataDictionary = &StubDataDictionary;

    #[test]
    fn implicit_vr_le() {
        let reader = ImplicitVRLittleEndianDecoder::with_dict(DICT);
        let mut cursor = Cursor::new(RAW.as_ref());
        {
            // read first element
            let (elem, bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0002));
            assert_eq!(elem.vr(), VR::UN);
            assert_eq!(elem.length(), Length(26));
            assert_eq!(bytes_read, 8);
            // read only half of the data
            let mut buffer: Vec<u8> = vec![0; 13];
            cursor
                .read_exact(buffer.as_mut_slice())
                .expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008".as_ref());
        }
        // cursor should now be @ #21 (there is no automatic skipping)
        assert_eq!(cursor.stream_position().unwrap(), 21);
        // cursor should now be @ #34 after skipping
        assert_eq!(cursor.seek(SeekFrom::Current(13)).unwrap(), 34);
        {
            // read second element
            let (elem, _bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0010));
            assert_eq!(elem.vr(), VR::UN);
            assert_eq!(elem.length(), Length(20));
            // read all data
            let mut buffer: Vec<u8> = vec![0; 20];
            cursor
                .read_exact(buffer.as_mut_slice())
                .expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008.1.2.1\0".as_ref());
        }
    }

    #[test]
    fn implicit_vr_le_with_standard_dictionary() {
        let reader = ImplicitVRLittleEndianDecoder::with_std_dict();
        let mut cursor = Cursor::new(RAW.as_ref());
        {
            // read first element
            let (elem, _bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(26));
            // cursor should be @ #8
            assert_eq!(cursor.stream_position().unwrap(), 8);
            // don't read any data, just skip
            // cursor should be @ #34 after skipping
            assert_eq!(
                cursor
                    .seek(SeekFrom::Current(elem.length().0 as i64))
                    .unwrap(),
                34
            );
        }
        {
            // read second element
            let (elem, _bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 16));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(20));
            // read all data
            let mut buffer: Vec<u8> = vec![0; 20];
            cursor
                .read_exact(buffer.as_mut_slice())
                .expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008.1.2.1\0".as_ref());
        }
    }

    // manually crafting some DICOM sequence/item delimiters
    //  Tag: (0008,103F) Series Description Code Sequence
    //  Implicit VR: SQ
    //  Reserved bytes: 0x0000
    //  Length: 0xFFFF_FFFF
    // --
    //  Tag: (FFFE,E000) Item
    //  Length: 0xFFFF_FFFF (unspecified)
    // --
    //  Tag: (FFFE,E00D) Item Delimitation Item
    //  Length: 0
    // --
    //  Tag: (FFFE,E0DD) Sequence Delimitation Item
    //  Length: 0
    // --
    const RAW_SEQUENCE_ITEMS: &[u8] = &[
        0x08, 0x00, 0x3F, 0x10, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFE, 0xFF, 0x0D, 0xE0, 0x00, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xDD, 0xE0, 0x00, 0x00,
        0x00, 0x00,
    ];

    #[test]
    fn decode_items() {
        let dec = ImplicitVRLittleEndianDecoder::default();
        let mut cursor = Cursor::new(RAW_SEQUENCE_ITEMS);
        {
            // read first element
            let (elem, bytes_read) = dec
                .decode_header(&mut cursor)
                .expect("should find an element header");
            assert_eq!(elem.tag(), Tag(8, 0x103F));
            assert_eq!(elem.vr(), VR::SQ);
            assert!(elem.length().is_undefined());
            assert_eq!(bytes_read, 8);
        }
        // cursor should now be @ #8
        assert_eq!(cursor.stream_position().unwrap(), 8);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE000));
            assert!(elem.length().is_undefined());
        }
        // cursor should now be @ #16
        assert_eq!(cursor.stream_position().unwrap(), 16);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE00D));
            assert_eq!(elem.length(), Length(0));
        }
        // cursor should now be @ #24
        assert_eq!(cursor.stream_position().unwrap(), 24);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_sequence_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE0DD));
            assert_eq!(elem.length(), Length(0));
        }
    }
}
