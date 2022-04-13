//! Explicit VR Big Endian syntax transfer implementation.

use crate::decode::basic::BigEndianBasicDecoder;
use crate::decode::*;
use crate::decode::{BasicDecode, Decode, DecodeFrom};
use byteordered::byteorder::{BigEndian, ByteOrder};
use dicom_core::header::{DataElementHeader, Length, SequenceItemHeader};
use dicom_core::{Tag, VR};
use snafu::ResultExt;
use std::io::Read;

/// A data element decoder for the Explicit VR Big Endian transfer syntax.
#[derive(Debug, Default, Clone)]
pub struct ExplicitVRBigEndianDecoder {
    basic: BigEndianBasicDecoder,
}

impl Decode for ExplicitVRBigEndianDecoder {
    fn decode_header<S>(&self, mut source: &mut S) -> Result<(DataElementHeader, usize)>
    where
        S: ?Sized + Read,
    {
        // retrieve tag
        let Tag(group, element) = self
            .basic
            .decode_tag(&mut source)
            .context(ReadHeaderTagSnafu)?;

        let mut buf = [0u8; 4];
        if group == 0xFFFE {
            // item delimiters do not have VR or reserved field
            source.read_exact(&mut buf).context(ReadItemLengthSnafu)?;
            let len = BigEndian::read_u32(&buf);
            return Ok((
                DataElementHeader::new((group, element), VR::UN, Length(len)),
                8, // tag + len
            ));
        }

        // retrieve explicit VR
        source.read_exact(&mut buf[0..2]).context(ReadVrSnafu)?;
        let vr = VR::from_binary([buf[0], buf[1]]).unwrap_or(VR::UN);

        let bytes_read;

        // retrieve data length
        let len = match vr {
            VR::OB
            | VR::OD
            | VR::OF
            | VR::OL
            | VR::OW
            | VR::SQ
            | VR::UC
            | VR::UR
            | VR::UT
            | VR::UN => {
                // read 2 reserved bytes, then 4 bytes for data length
                source
                    .read_exact(&mut buf[0..2])
                    .context(ReadReservedSnafu)?;
                source.read_exact(&mut buf).context(ReadLengthSnafu)?;
                bytes_read = 12;
                BigEndian::read_u32(&buf)
            }
            _ => {
                // read 2 bytes for the data length
                source.read_exact(&mut buf[0..2]).context(ReadLengthSnafu)?;
                bytes_read = 8;
                u32::from(BigEndian::read_u16(&buf[0..2]))
            }
        };

        Ok((
            DataElementHeader::new((group, element), vr, Length(len)),
            bytes_read,
        ))
    }

    fn decode_item_header<S>(&self, source: &mut S) -> Result<SequenceItemHeader>
    where
        S: ?Sized + Read,
    {
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf).context(ReadItemHeaderSnafu)?;
        // retrieve tag
        let group = BigEndian::read_u16(&buf[0..2]);
        let element = BigEndian::read_u16(&buf[2..4]);
        let len = BigEndian::read_u32(&buf[4..8]);

        SequenceItemHeader::new((group, element), Length(len)).context(BadSequenceHeaderSnafu)
    }

    fn decode_tag<S>(&self, source: &mut S) -> Result<Tag>
    where
        S: ?Sized + Read,
    {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf).context(ReadTagSnafu)?;
        Ok(Tag(
            BigEndian::read_u16(&buf[0..2]),
            BigEndian::read_u16(&buf[2..4]),
        ))
    }
}

impl<S: ?Sized> DecodeFrom<S> for ExplicitVRBigEndianDecoder
where
    S: Read,
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
    use super::ExplicitVRBigEndianDecoder;
    use crate::decode::Decode;
    use dicom_core::header::{HasLength, Header, Length};
    use dicom_core::{Tag, VR};
    use std::io::{Cursor, Read, Seek, SeekFrom};

    // manually crafting some DICOM data elements
    //  Tag: (0002,0002) Media Storage SOP Class UID
    //  VR: UI
    //  Length: 26
    //  Value: "1.2.840.10008.5.1.4.1.1.1" (with 1 padding '\0')
    // --
    //  Tag: (0002,0010) Transfer Syntax UID
    //  VR: UI
    //  Length: 20
    //  Value: "1.2.840.10008.1.2.1" (w 1 padding '\0') == ExplicitVRLittleEndian
    // --
    const RAW: &'static [u8; 62] = &[
        0x00, 0x02, 0x00, 0x02, 0x55, 0x49, 0x00, 0x1a, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30,
        0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e,
        0x31, 0x2e, 0x31, 0x00, 0x00, 0x02, 0x00, 0x10, 0x55, 0x49, 0x00, 0x14, 0x31, 0x2e, 0x32,
        0x2e, 0x38, 0x34, 0x30, 0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e,
        0x31, 0x00,
    ];

    #[test]
    fn decode_explicit_vr_be() {
        let reader = ExplicitVRBigEndianDecoder::default();
        let mut cursor = Cursor::new(RAW.as_ref());
        {
            // read first element
            let (elem, bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(26));
            assert_eq!(bytes_read, 8);
            // read only half of the data
            let mut buffer = [0; 13];
            cursor.read_exact(&mut buffer).expect("should read it fine");
            assert_eq!(&buffer, b"1.2.840.10008".as_ref());
        }
        // cursor should now be @ #21 (there is no automatic skipping)
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 21);
        // cursor should now be @ #34 after skipping
        assert_eq!(cursor.seek(SeekFrom::Current(13)).unwrap(), 34);
        {
            // read second element
            let (elem, _bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 16));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(20));
            // read all data
            let mut buffer = [0; 20];
            cursor.read_exact(&mut buffer).expect("should read it fine");
            assert_eq!(&buffer, b"1.2.840.10008.1.2.1\0".as_ref());
        }
    }

    // manually crafting some DICOM sequence/item delimiters
    //  Tag: (0008,103F) Series Description Code Sequence
    //  VR: SQ
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
    const RAW_SEQUENCE_ITEMS: &'static [u8] = &[
        0x00, 0x08, 0x10, 0x3F, b'S', b'Q', 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xE0,
        0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xE0, 0x0D, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFE,
        0xE0, 0xDD, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn decode_items() {
        let dec = ExplicitVRBigEndianDecoder::default();
        let mut cursor = Cursor::new(RAW_SEQUENCE_ITEMS);
        {
            // read first element
            let (elem, _bytes_read) = dec
                .decode_header(&mut cursor)
                .expect("should find an element header");
            assert_eq!(elem.tag(), Tag(8, 0x103F));
            assert_eq!(elem.vr(), VR::SQ);
            assert!(elem.length().is_undefined());
        }
        // cursor should now be @ #12
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 12);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE000));
            assert!(elem.length().is_undefined());
        }
        // cursor should now be @ #20
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 20);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE00D));
            assert_eq!(elem.length(), Length(0));
        }
        // cursor should now be @ #28
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 28);
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
