//! Explicit VR Big Endian syntax transfer implementation.

use crate::decode::basic::BigEndianBasicDecoder;
use crate::decode::{
    BadSequenceHeaderSnafu, BasicDecode, Decode, DecodeFrom, ReadHeaderTagSnafu,
    ReadItemHeaderSnafu, ReadItemLengthSnafu, ReadLengthSnafu, ReadReservedSnafu, ReadTagSnafu,
    ReadVrSnafu, Result,
};
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
            // PS3.5 7.1.2:
            // for VRs of AE, AS, AT, CS, DA, DS, DT, FL, FD, IS, LO, LT, PN,
            // SH, SL, SS, ST, TM, UI, UL and US the Value Length Field is the
            // 16-bit unsigned integer following the two byte VR Field (Table
            // 7.1-2). The value of the Value Length Field shall equal the
            // length of the Value Field.
            VR::AE
            | VR::AS
            | VR::AT
            | VR::CS
            | VR::DA
            | VR::DS
            | VR::DT
            | VR::FL
            | VR::FD
            | VR::IS
            | VR::LO
            | VR::LT
            | VR::PN
            | VR::SH
            | VR::SL
            | VR::SS
            | VR::ST
            | VR::TM
            | VR::UI
            | VR::UL
            | VR::US => {
                // read 2 bytes for the data length
                source
                    .read_exact(&mut buf[0..2])
                    .context(ReadItemLengthSnafu)?;
                bytes_read = 8;
                u32::from(BigEndian::read_u16(&buf[0..2]))
            }
            // PS3.5 7.1.2:
            // for all other VRs the 16 bits following the two byte VR Field
            // are reserved for use by later versions of the DICOM Standard.
            // These reserved bytes shall be set to 0000H and shall not be
            // used or decoded (Table 7.1-1). The Value Length Field is a
            // 32-bit unsigned integer.
            _ => {
                // read 2 reserved bytes, then 4 bytes for data length
                source
                    .read_exact(&mut buf[0..2])
                    .context(ReadReservedSnafu)?;
                source.read_exact(&mut buf).context(ReadLengthSnafu)?;
                bytes_read = 12;
                BigEndian::read_u32(&buf)
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
    use std::io::{Cursor, Seek, SeekFrom};

    // manually crafting some DICOM data elements
    #[rustfmt::skip]
    const RAW: &[u8] = &[
        0x00, 0x02, 0x00, 0x02,     // (0002,0002) (BE) Media Storage SOP Class UID
            b'U', b'I',             // VR: UI (UID)
            0x00, 0x1A,             // Length: 26 bytes (BE)
                // UID: 1.2.840.10008.5.1.4.1.1.1
                b'1', b'.', b'2', b'.', b'8', b'4', b'0', b'.', b'1', b'0', b'0', b'0', b'8', b'.',
                b'5', b'.', b'1', b'.', b'4', b'.', b'1', b'.', b'1', b'.', b'1',
                0x00,               // Padding to make length even
        0x00, 0x02, 0x00, 0x10,     // (0002,0010) (BE) Transfer Syntax UID
            b'U', b'I',             // VR: UI (UID)
            0x00, 0x14,             // Length: 20 bytes (BE)
                // UID: 1.2.840.10008.1.2.1 (ExplicitVRLittleEndian)
                b'1', b'.', b'2', b'.', b'8', b'4', b'0', b'.', b'1', b'0', b'0', b'0', b'8', b'.',
                b'1', b'.', b'2', b'.', b'1',
                0x00,               // Padding to make length even
        0x00, 0x08, 0x00, 0x54,     // (0008,0054) (BE) Retrieve AE Title
            b'A', b'E',             // VR: AE (Application Entity)
            0x00, 0x06,             // Length: 6 bytes (BE)
                // String "TITLE"
                b'T', b'I', b'T', b'L', b'E',
                b' ',               // Padding to make length even
        0x00, 0x10, 0x10, 0x10,     // (0010,1010) (BE) Patient's Age
            b'A', b'S',             // VR: AS (Age String)
            0x00, 0x02,             // Length: 2 bytes (BE)
                // String "8Y"
                b'8', b'Y',
        0x00, 0x72, 0x00, 0x26,     // (0072,0026) (BE) Selector Attribute
            b'A', b'T',             // VR: AT (Attribute Tag)
            0x00, 0x04,             // Length: 4 bytes (BE)
                // Tag: (0028,2110) (BE) Lossy Image Compression
                0x00, 0x28, 0x21, 0x10,
        0x00, 0x10, 0x00, 0x40,     // (0010,0040) (BE) Patient Sex
            b'C', b'S',             // VR: CS (Code String)
            0x00, 0x02,             // Length: 2 bytes (BE)
                // Text: "O"
                b'O',
                b' ',               // Padding to make length even
        0x00, 0x10, 0x00, 0x30,     // (0010,0030) (BE) Patient's Birth Date
            b'D', b'A',             // VR: DA (Date)
            0x00, 0x08,             // Length: 8 bytes (BE)
                // String "19800101"
                b'1', b'9', b'8', b'0', b'0', b'1', b'0', b'1',
        0x00, 0x10, 0x10, 0x20,     // (0010,1020) (BE) Patient's size
            b'D', b'S',             // VR: DS (Decimal String)
            0x00, 0x04,             // Length: 4 bytes (BE)
                // String: "1.70"
                b'1', b'.', b'7', b'0',
        0x00, 0x08, 0x00, 0x15,     // (0008,0015) (BE) Instance coercion datetime
            b'D', b'T',             // VR: DT (Date Time)
            0x00, 0x0E,             // Length: 14 bytes (BE)
                // String "20051231235960"
                b'2', b'0', b'0', b'5', b'1', b'2', b'3', b'1', b'2', b'3', b'5', b'9', b'6', b'0',
        0x00, 0x10, 0x94, 0x31,     // (0010,9431) (BE) Examined Body Thickness
            b'F', b'L',             // VR: FL (Floating-point IEEE-754)
            0x00, 0x04,             // Length: 4 bytes (BE)
                // Value: 3.1415927 (BE)
                0x40, 0x49, 0x0F, 0xDB,
        0x00, 0x40, 0x92, 0x25,     // (0040,9225) (BE) Real World Value Slope
            b'F', b'D',             // VR: FD (Double-precision floating-point IEEE-754)
            0x00, 0x08,             // Length: 8 bytes (BE)
                // Value: 3.141592653589793 (BE)
                0x40, 0x09, 0x21, 0xFB, 0x54, 0x44, 0x2D, 0x18,
        0x00, 0x20, 0x00, 0x13,     // (0020,0013) (BE) Instance Number
            b'I', b'S',             // VR: IS (Integer String)
            0x00, 0x08,             // Length: 8 bytes (BE)
                // String: "1234567"
                b'1', b'2', b'3', b'4', b'5', b'6', b'7',
                b' ',               // Padding to make length even
        0x00, 0x10, 0x00, 0x20,     // (0010,0020) (BE) Patient ID
            b'L', b'O',             // VR: LO (Long string)
            0x00, 0x0A,             // Length: 10 bytes (BE)
                // String "P12345678X"
                b'P', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'X',
        0x00, 0x10, 0x40, 0x00,     // (0010,4000) (BE) Patient Comments
            b'L', b'T',             // VR: LT (Long Text)
            0x00, 0x04,             // Length: 4 bytes (BE)
                // String "None"
                b'N', b'o', b'n', b'e',
        0x00, 0x08, 0x04, 0x1B,     // (0008,041B) (BE) RecordKey
            b'O', b'B',             // VR: OB (Other Byte)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x02, // Length: 2 bytes (BE)
                // Value: [0x12, 0x34]
                0x12, 0x34,
        0x7F, 0xE0, 0x00, 0x09,     // (7FE0,0009) (BE) Double Float Pixel Data
            b'O', b'D',             // VR: OD (Other Double)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x08, // Length: 8 bytes (BE)
                // Value: [3.141592653589793] (BE)
                0x40, 0x09, 0x21, 0xFB, 0x54, 0x44, 0x2D, 0x18,
        0x7F, 0xE0, 0x00, 0x08,     // (7FE0,0008) (BE) Float Pixel Data
            b'O', b'F',             // VR: OF (Other Float)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x04, // Length: 4 bytes (BE)
                // Value: [3.1415927] (BE)
                0x40, 0x49, 0x0F, 0xDB,
        0x00, 0x72, 0x00, 0x75,     // (0072,0075) (BE) Selector OL Value
            b'O', b'L',             // VR: OL (Other Long)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x04, // Length: 4 bytes (BE)
                // Value: [0x12345678] (BE)
                0x12, 0x34, 0x56, 0x78,
        0x00, 0x72, 0x00, 0x81,     // (0072,0081) (BE) Selector OV Value
            b'O', b'V',             // VR: OV (Other Very long)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x08, // Length: 8 bytes (BE)
                // Value: [0x192A3B4C5D6E7F80] (BE)
                0x19, 0x2A, 0x3B, 0x4C, 0x5D, 0x6E, 0x7F, 0x80,
        0x00, 0x72, 0x00, 0x69,     // (0072,0069) (BE) Selector OW Value
            b'O', b'W',             // VR: OW (Other Word)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x02, // Length: 2 bytes (BE)
                // Value: [0x1234] (BE)
                0x12, 0x34,
        0x00, 0x10, 0x00, 0x10,     // (0010,0010) (BE) Patient Name
            b'P', b'N',             // VR: PN (Person Name)
            0x00, 0x08,             // Length: 8 bytes (BE)
                // String: "Doe^John"
                b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
        0x00, 0x40, 0x92, 0x10,     // (0040,9210) (BE) LUT Label
            b'S', b'H',             // VR: SH (Short string)
            0x00, 0x04,             // Length: 4 bytes (BE)
                // String: "LBL"
                b'L', b'B', b'L',
                b' ',               // Padding to make length even
        0x00, 0x18, 0x60, 0x20,     // (0018,6020) (BE) Reference Pixel X0
            b'S', b'L',             // VR: SL (Signed Long)
            0x00, 0x04,             // Length: 4 bytes (BE)
                // Value: -12345678 (BE)
                0xFF, 0x43, 0x9E, 0xB2,
        // Sequences (VR: SQ) tested elsewhere
        0x00, 0x28, 0x95, 0x03,     // (0028,9503) (BE) Vertices of the Region (VM 2-2n)
            b'S', b'S',             // VR: SS (Signed Short)
            0x00, 0x04,             // Length: 4 bytes (2 * 2) (BE)
                // Value: -4567 (BE)
                0xEE, 0x29,
                // Value: 4321 (BE)
                0x10, 0xE1,
        0x00, 0x40, 0x02, 0x80,     // (0040,0280) (BE) Comments on the Performed Procedure Step
            b'S', b'T',             // VR: ST (Short Text)
            0x00, 0x0A,             // Length: 10 bytes (BE)
                // String: "No comment"
                b'N', b'o', b' ', b'c', b'o', b'm', b'm', b'e', b'n', b't',
        0x00, 0x72, 0x00, 0x82,     // (0072,0082) (BE) SelectorSVValue (VM: 1-n)
            b'S', b'V',             // VR: SV (Signed Very long)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x10, // Length: 16 bytes (2 * 8) (BE)
                // Value: -123456789012345678 (BE)
                0xFE, 0x49, 0x64, 0xB4, 0x59, 0xCF, 0x0C, 0xB2,
                // Value: 123456789012345678 (BE)
                0x01, 0xB6, 0x9B, 0x4B, 0xA6, 0x30, 0xF3, 0x4E,
        0x00, 0x10, 0x00, 0x32,     // (0010,0032) (BE) Patient's Birth Time
            b'T', b'M',             // VR: TM (Time)
            0x00, 0x06,             // Length: 6 bytes (BE)
                // String: "123456"
                b'1', b'2', b'3', b'4', b'5', b'6',
        0x00, 0x08, 0x01, 0x19,     // (0008,0119) (BE) Long Code Value
            b'U', b'C',             // VR: UC (Unlimited Characters)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x04, // Length: 4 bytes (BE)
                // String: "Code"
                b'C', b'o', b'd', b'e',
        // UI already tested above
        0x00, 0x18, 0x60, 0x16,     // (0018,6016) (BE) Region Flags
            b'U', b'L',
            0x00, 0x04,             // Length: 4 bytes (BE)
                // Value: 1 (BE)
                0x00, 0x00, 0x00, 0x01,
        0xC0, 0x01, 0x12, 0x34,     // (C001,1234) (BE) (Private data element)
            b'U', b'N',             // VR: UN (Unknown)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x06, // Length: 6 bytes (BE)
                // Value: [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        0x00, 0x08, 0x01, 0x0E,     // (0008,010E) (BE) Coding Scheme URL
            b'U', b'R',             // VR: UR (Universal Resource Locator, URL)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x12, // Length: 18 bytes (BE)
                // String: "http://example.com"
                b'h', b't', b't', b'p', b':', b'/', b'/', b'e', b'x', b'a', b'm', b'p', b'l', b'e',
                b'.', b'c', b'o', b'm',
        0x00, 0x08, 0x00, 0x40,     // (0008,0040) (BE) Data Set Type
            b'U', b'S',             // VR: US (Unsigned Short)
            0x00, 0x02,             // Length: 2 bytes (BE)
                // Value: 34567 (BE)
                0x87, 0x07,
        0x00, 0x18, 0x99, 0x17,     // (0018,9917) (BE) Instruction Description
            b'U', b'T',             // VR: UT (Unlimited Text)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x08, // Length: 8 bytes (BE)
                // String: "No text"
                b'N', b'o', b' ', b't', b'e', b'x', b't',
                b' ',               // Padding to make length even
        0x00, 0x08, 0x04, 0x0C,     // (0008,040C) (BE) File Offset in Container
            b'U', b'V',             // VR: UV (Unsigned Very long)
            0x00, 0x00,             // Reserved, always 0
            0x00, 0x00, 0x00, 0x08, // Length: 8 bytes (BE)
                // Value: 12345678901234567890 (BE)
                0xAB, 0x54, 0xA9, 0x8C, 0xEB, 0x1F, 0x0A, 0xD2,
    ];

    #[test]
    fn decode_explicit_vr_be() {
        let dec = ExplicitVRBigEndianDecoder::default();
        let mut cursor = Cursor::new(RAW.as_ref());

        fn read_n<'a>(cursor: &mut Cursor<&'a [u8]>, n: usize) -> &'a [u8] {
            let pos = cursor.position() as usize;
            let slice = &cursor.get_ref()[pos..pos + n]; // panic if too short
            cursor.set_position((pos + n) as u64);
            slice
        }

        fn test_vr(
            dec: &ExplicitVRBigEndianDecoder,
            cursor: &mut Cursor<&[u8]>,
            group: u16,
            element: u16,
            vr: VR,
            value: &[u8],
        ) {
            let (elem, _bytes_read) = dec.decode_header(cursor).expect("should find an element");
            assert_eq!(elem.tag(), Tag(group, element));
            assert_eq!(elem.vr(), vr);
            assert_eq!(elem.length(), Length(value.len() as u32));
            let buffer = read_n(cursor, value.len());
            assert_eq!(&buffer, &value);
        }

        // read first element
        let (elem, bytes_read) = dec
            .decode_header(&mut cursor)
            .expect("should find an element");
        assert_eq!(elem.tag(), Tag(0x0002, 0x0002));
        assert_eq!(elem.vr(), VR::UI);
        assert_eq!(elem.length(), Length(26));
        assert_eq!(bytes_read, 8);
        // read only half of the value data
        let buffer = read_n(&mut cursor, 13);
        assert_eq!(&buffer, b"1.2.840.10008");

        // cursor should now be @ #21 (there is no automatic skipping)
        assert_eq!(cursor.stream_position().unwrap(), 21);
        // cursor should now be @ #34 after skipping
        assert_eq!(cursor.seek(SeekFrom::Current(13)).unwrap(), 34);

        // read second element
        let (elem, _bytes_read) = dec
            .decode_header(&mut cursor)
            .expect("should find an element");
        assert_eq!(elem.tag(), Tag(2, 16));
        assert_eq!(elem.vr(), VR::UI);
        assert_eq!(elem.length(), Length(20));
        // read all data
        let buffer = read_n(&mut cursor, 20);
        assert_eq!(&buffer, b"1.2.840.10008.1.2.1\0");

        // read various VRs
        test_vr(&dec, &mut cursor, 0x0008, 0x0054, VR::AE, b"TITLE ");
        test_vr(&dec, &mut cursor, 0x0010, 0x1010, VR::AS, b"8Y");
        test_vr(
            &dec,
            &mut cursor,
            0x0072,
            0x0026,
            VR::AT,
            &[0x00, 0x28, 0x21, 0x10],
        );
        test_vr(&dec, &mut cursor, 0x0010, 0x0040, VR::CS, b"O ");
        test_vr(&dec, &mut cursor, 0x0010, 0x0030, VR::DA, b"19800101");
        test_vr(&dec, &mut cursor, 0x0010, 0x1020, VR::DS, b"1.70");
        test_vr(
            &dec,
            &mut cursor,
            0x0008,
            0x0015,
            VR::DT,
            b"20051231235960",
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0010,
            0x9431,
            VR::FL,
            &[0x40, 0x49, 0x0F, 0xDB],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0040,
            0x9225,
            VR::FD,
            &[0x40, 0x09, 0x21, 0xFB, 0x54, 0x44, 0x2D, 0x18],
        );
        test_vr(&dec, &mut cursor, 0x0020, 0x0013, VR::IS, b"1234567 ");
        test_vr(&dec, &mut cursor, 0x0010, 0x0020, VR::LO, b"P12345678X");
        test_vr(&dec, &mut cursor, 0x0010, 0x4000, VR::LT, b"None");
        test_vr(&dec, &mut cursor, 0x0008, 0x041B, VR::OB, &[0x12, 0x34]);
        test_vr(
            &dec,
            &mut cursor,
            0x7FE0,
            0x0009,
            VR::OD,
            &[0x40, 0x09, 0x21, 0xFB, 0x54, 0x44, 0x2D, 0x18],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x7FE0,
            0x0008,
            VR::OF,
            &[0x40, 0x49, 0x0F, 0xDB],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0072,
            0x0075,
            VR::OL,
            &[0x12, 0x34, 0x56, 0x78],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0072,
            0x0081,
            VR::OV,
            &[0x19, 0x2A, 0x3B, 0x4C, 0x5D, 0x6E, 0x7F, 0x80],
        );
        test_vr(&dec, &mut cursor, 0x0072, 0x0069, VR::OW, &[0x12, 0x34]);
        test_vr(&dec, &mut cursor, 0x0010, 0x0010, VR::PN, b"Doe^John");
        test_vr(&dec, &mut cursor, 0x0040, 0x9210, VR::SH, b"LBL ");
        test_vr(
            &dec,
            &mut cursor,
            0x0018,
            0x6020,
            VR::SL,
            &[0xFF, 0x43, 0x9E, 0xB2],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0028,
            0x9503,
            VR::SS,
            &[0xEE, 0x29, 0x10, 0xE1],
        );
        test_vr(&dec, &mut cursor, 0x0040, 0x0280, VR::ST, b"No comment");
        test_vr(
            &dec,
            &mut cursor,
            0x0072,
            0x0082,
            VR::SV,
            &[
                0xFE, 0x49, 0x64, 0xB4, 0x59, 0xCF, 0x0C, 0xB2, 0x01, 0xB6, 0x9B, 0x4B, 0xA6, 0x30,
                0xF3, 0x4E,
            ],
        );
        test_vr(&dec, &mut cursor, 0x0010, 0x0032, VR::TM, b"123456");
        test_vr(&dec, &mut cursor, 0x0008, 0x0119, VR::UC, b"Code");
        test_vr(
            &dec,
            &mut cursor,
            0x0018,
            0x6016,
            VR::UL,
            &[0x00, 0x00, 0x00, 0x01],
        );
        test_vr(
            &dec,
            &mut cursor,
            0xC001,
            0x1234,
            VR::UN,
            &[0x1, 0x2, 0x3, 0x4, 0x5, 0x6],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0008,
            0x010E,
            VR::UR,
            b"http://example.com",
        );
        test_vr(&dec, &mut cursor, 0x0008, 0x0040, VR::US, &[0x87, 0x07]);
        test_vr(&dec, &mut cursor, 0x0018, 0x9917, VR::UT, b"No text ");
        test_vr(
            &dec,
            &mut cursor,
            0x0008,
            0x040C,
            VR::UV,
            &[0xAB, 0x54, 0xA9, 0x8C, 0xEB, 0x1F, 0x0A, 0xD2],
        );
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
    const RAW_SEQUENCE_ITEMS: &[u8] = &[
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
        assert_eq!(cursor.stream_position().unwrap(), 12);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE000));
            assert!(elem.length().is_undefined());
        }
        // cursor should now be @ #20
        assert_eq!(cursor.stream_position().unwrap(), 20);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE00D));
            assert_eq!(elem.length(), Length(0));
        }
        // cursor should now be @ #28
        assert_eq!(cursor.stream_position().unwrap(), 28);
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
