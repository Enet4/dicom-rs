//! Explicit VR Little Endian syntax transfer implementation

use crate::decode::basic::LittleEndianBasicDecoder;
use crate::decode::{
    BadSequenceHeaderSnafu, BasicDecode, Decode, DecodeFrom, ReadHeaderTagSnafu,
    ReadItemHeaderSnafu, ReadItemLengthSnafu, ReadLengthSnafu, ReadReservedSnafu, ReadTagSnafu,
    ReadVrSnafu, Result,
};
use byteordered::byteorder::{ByteOrder, LittleEndian};
use dicom_core::header::{DataElementHeader, Length, SequenceItemHeader};
use dicom_core::{Tag, VR};
use snafu::ResultExt;
use std::io::Read;

/// A data element decoder for the Explicit VR Little Endian transfer syntax.
#[derive(Debug, Default, Clone)]
pub struct ExplicitVRLittleEndianDecoder {
    basic: LittleEndianBasicDecoder,
}

impl Decode for ExplicitVRLittleEndianDecoder {
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
            let len = LittleEndian::read_u32(&buf);
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
                source.read_exact(&mut buf[0..2]).context(ReadLengthSnafu)?;
                bytes_read = 8;
                u32::from(LittleEndian::read_u16(&buf[0..2]))
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
                LittleEndian::read_u32(&buf)
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
        let group = LittleEndian::read_u16(&buf[0..2]);
        let element = LittleEndian::read_u16(&buf[2..4]);
        let len = LittleEndian::read_u32(&buf[4..8]);

        SequenceItemHeader::new((group, element), Length(len)).context(BadSequenceHeaderSnafu)
    }

    fn decode_tag<S>(&self, source: &mut S) -> Result<Tag>
    where
        S: ?Sized + Read,
    {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf).context(ReadTagSnafu)?;
        Ok(Tag(
            LittleEndian::read_u16(&buf[0..2]),
            LittleEndian::read_u16(&buf[2..4]),
        ))
    }
}

impl<S: ?Sized> DecodeFrom<S> for ExplicitVRLittleEndianDecoder
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
    use super::ExplicitVRLittleEndianDecoder;
    use crate::decode::Decode;
    use dicom_core::header::{HasLength, Header, Length};
    use dicom_core::{Tag, VR};
    use std::io::{Cursor, Seek, SeekFrom};

    // manually crafting some DICOM data elements
    #[rustfmt::skip]
    const RAW: &[u8] = &[
        0x02, 0x00, 0x02, 0x00,     // (0002,0002) (LE) Media Storage SOP Class UID
            b'U', b'I',             // VR: UI (UID)
            0x1A, 0x00,             // Length: 26 bytes (LE)
                // UID: 1.2.840.10008.5.1.4.1.1.1
                b'1', b'.', b'2', b'.', b'8', b'4', b'0', b'.', b'1', b'0', b'0', b'0', b'8', b'.',
                b'5', b'.', b'1', b'.', b'4', b'.', b'1', b'.', b'1', b'.', b'1',
                0x00,               // Padding to make length even
        0x02, 0x00, 0x10, 0x00,     // (0002,0010) (LE) Transfer Syntax UID
            b'U', b'I',             // VR: UI (UID)
            0x14, 0x00,             // Length: 20 bytes (LE)
                // UID: 1.2.840.10008.1.2.1 (ExplicitVRLittleEndian)
                b'1', b'.', b'2', b'.', b'8', b'4', b'0', b'.', b'1', b'0', b'0', b'0', b'8', b'.',
                b'1', b'.', b'2', b'.', b'1',
                0x00,               // Padding to make length even
        0x08, 0x00, 0x54, 0x00,     // (0008,0054) (LE) Retrieve AE Title
            b'A', b'E',             // VR: AE (Application Entity)
            0x06, 0x00,             // Length: 6 bytes (LE)
                // String "TITLE"
                b'T', b'I', b'T', b'L', b'E',
                b' ',               // Padding to make length even
        0x10, 0x00, 0x10, 0x10,     // (0010,1010) (LE) Patient's Age
            b'A', b'S',             // VR: AS (Age String)
            0x02, 0x00,             // Length: 2 bytes (LE)
                // String "8Y"
                b'8', b'Y',
        0x72, 0x00, 0x26, 0x00,     // (0072,0026) (LE) Selector Attribute
            b'A', b'T',             // VR: AT (Attribute Tag)
            0x04, 0x00,             // Length: 4 bytes (LE)
                // Tag: (0028,2110) (LE) Lossy Image Compression
                0x28, 0x00, 0x10, 0x21,
        0x10, 0x00, 0x40, 0x00,     // (0010,0040) (LE) Patient Sex
            b'C', b'S',             // VR: CS (Code String)
            0x02, 0x00,             // Length: 2 bytes (LE)
                // Text: "O"
                b'O',
                b' ',               // Padding to make length even
        0x10, 0x00, 0x30, 0x00,     // (0010,0030) (LE) Patient's Birth Date
            b'D', b'A',             // VR: DA (Date)
            0x08, 0x00,             // Length: 8 bytes (LE)
                // String "19800101"
                b'1', b'9', b'8', b'0', b'0', b'1', b'0', b'1',
        0x10, 0x00, 0x20, 0x10,     // (0010,1020) (LE) Patient's size
            b'D', b'S',             // VR: DS (Decimal String)
            0x04, 0x00,             // Length: 4 bytes (LE)
                // String: "1.70"
                b'1', b'.', b'7', b'0',
        0x08, 0x00, 0x15, 0x00,     // (0008,0015) (LE) Instance coercion datetime
            b'D', b'T',             // VR: DT (Date Time)
            0x0E, 0x00,             // Length: 14 bytes (LE)
                // String "20051231235960"
                b'2', b'0', b'0', b'5', b'1', b'2', b'3', b'1', b'2', b'3', b'5', b'9', b'6', b'0',
        0x10, 0x00, 0x31, 0x94,     // (0010,9431) (LE) Examined Body Thickness
            b'F', b'L',             // VR: FL (Floating-point IEEE-754)
            0x04, 0x00,             // Length: 4 bytes (LE)
                // Value: 3.1415927 (LE)
                0xDB, 0x0F, 0x49, 0x40,
        0x40, 0x00, 0x25, 0x92,     // (0040,9225) (LE) Real World Value Slope
            b'F', b'D',             // VR: FD (Double-precision floating-point IEEE-754)
            0x08, 0x00,             // Length: 8 bytes (LE)
                // Value: 3.141592653589793 (LE)
                0x18, 0x2D, 0x44, 0x54, 0xFB, 0x21, 0x09, 0x40,
        0x20, 0x00, 0x13, 0x00,     // (0020,0013) (LE) Instance Number
            b'I', b'S',             // VR: IS (Integer String)
            0x08, 0x00,             // Length: 8 bytes (LE)
                // String: "1234567"
                b'1', b'2', b'3', b'4', b'5', b'6', b'7',
                b' ',               // Padding to make length even
        0x10, 0x00, 0x20, 0x00,     // (0010,0020) (LE) Patient ID
            b'L', b'O',             // VR: LO (Long string)
            0x0A, 0x00,             // Length: 10 bytes (LE)
                // String "P12345678X"
                b'P', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'X',
        0x10, 0x00, 0x00, 0x40,     // (0010,4000) (LE) Patient Comments
            b'L', b'T',             // VR: LT (Long Text)
            0x04, 0x00,             // Length: 4 bytes (LE)
                // String "None"
                b'N', b'o', b'n', b'e',
        0x08, 0x00, 0x1B, 0x04,     // (0008,041B) (LE) RecordKey
            b'O', b'B',             // VR: OB (Other Byte)
            0x00, 0x00,             // Reserved, always 0
            0x02, 0x00, 0x00, 0x00, // Length: 2 bytes (LE)
                // Value: [0x12, 0x34]
                0x12, 0x34,
        0xE0, 0x7F, 0x09, 0x00,     // (7FE0,0009) (LE) Double Float Pixel Data
            b'O', b'D',             // VR: OD (Other Double)
            0x00, 0x00,             // Reserved, always 0
            0x08, 0x00, 0x00, 0x00, // Length: 8 bytes (LE)
                // Value: [3.141592653589793] (LE)
                0x18, 0x2D, 0x44, 0x54, 0xFB, 0x21, 0x09, 0x40,
        0xE0, 0x7F, 0x08, 0x00,     // (7FE0,0008) (LE) Float Pixel Data
            b'O', b'F',             // VR: OF (Other Float)
            0x00, 0x00,             // Reserved, always 0
            0x04, 0x00, 0x00, 0x00, // Length: 4 bytes (LE)
                // Value: [3.1415927] (LE)
                0xDB, 0x0F, 0x49, 0x40,
        0x72, 0x00, 0x75, 0x00,     // (0072,0075) (LE) Selector OL Value
            b'O', b'L',             // VR: OL (Other Long)
            0x00, 0x00,             // Reserved, always 0
            0x04, 0x00, 0x00, 0x00, // Length: 4 bytes (LE)
                // Value: [0x12345678] (LE)
                0x78, 0x56, 0x34, 0x12,
        0x72, 0x00, 0x81, 0x00,     // (0072,0081) (LE) Selector OV Value
            b'O', b'V',             // VR: OV (Other Very long)
            0x00, 0x00,             // Reserved, always 0
            0x08, 0x00, 0x00, 0x00, // Length: 8 bytes (LE)
                // Value: [0x192A3B4C5D6E7F80] (LE)
                0x80, 0x7F, 0x6E, 0x5D, 0x4C, 0x3B, 0x2A, 0x19,
        0x72, 0x00, 0x69, 0x00,     // (0072,0069) (LE) Selector OW Value
            b'O', b'W',             // VR: OW (Other Word)
            0x00, 0x00,             // Reserved, always 0
            0x02, 0x00, 0x00, 0x00, // Length: 2 bytes (LE)
                // Value: [0x1234] (LE)
                0x34, 0x12,
        0x10, 0x00, 0x10, 0x00,     // (0010,0010) (LE) Patient Name
            b'P', b'N',             // VR: PN (Person Name)
            0x08, 0x00,             // Length: 8 bytes (LE)
                // String: "Doe^John"
                b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
        0x40, 0x00, 0x10, 0x92,     // (0040,9210) (LE) LUT Label
            b'S', b'H',             // VR: SH (Short string)
            0x04, 0x00,             // Length: 4 bytes (LE)
                // String: "LBL"
                b'L', b'B', b'L',
                b' ',               // Padding to make length even
        0x18, 0x00, 0x20, 0x60,     // (0018,6020) (LE) Reference Pixel X0
            b'S', b'L',             // VR: SL (Signed Long)
            0x04, 0x00,             // Length: 4 bytes (LE)
                // Value: -12345678 (LE)
                0xB2, 0x9E, 0x43, 0xFF,
        // Sequences (VR: SQ) tested elsewhere
        0x28, 0x00, 0x03, 0x95,     // (0028,9503) (LE) Vertices of the Region (VM 2-2n)
            b'S', b'S',             // VR: SS (Signed Short)
            0x04, 0x00,             // Length: 4 bytes (2 * 2) (LE)
                // Value: -4567 (LE)
                0x29, 0xEE,
                // Value: 4321 (LE)
                0xE1, 0x10,
        0x40, 0x00, 0x80, 0x02,     // (0040,0280) (LE) Comments on the Performed Procedure Step
            b'S', b'T',             // VR: ST (Short Text)
            0x0A, 0x00,             // Length: 10 bytes (LE)
                // String: "No comment"
                b'N', b'o', b' ', b'c', b'o', b'm', b'm', b'e', b'n', b't',
        0x72, 0x00, 0x82, 0x00,     // (0072,0082) (LE) SelectorSVValue (VM: 1-n)
            b'S', b'V',             // VR: SV (Signed Very long)
            0x00, 0x00,             // Reserved, always 0
            0x10, 0x00, 0x00, 0x00, // Length: 16 bytes (2 * 8) (LE)
                // Value: -123456789012345678 (LE)
                0xB2, 0x0C, 0xCF, 0x59, 0xB4, 0x64, 0x49, 0xFE,
                // Value: 123456789012345678 (LE)
                0x4E, 0xF3, 0x30, 0xA6, 0x4B, 0x9B, 0xB6, 0x01,
        0x10, 0x00, 0x32, 0x00,     // (0010,0032) (LE) Patient's Birth Time
            b'T', b'M',             // VR: TM (Time)
            0x06, 0x00,             // Length: 6 bytes (LE)
                // String: "123456"
                b'1', b'2', b'3', b'4', b'5', b'6',
        0x08, 0x00, 0x19, 0x01,     // (0008,0119) (LE) Long Code Value
            b'U', b'C',             // VR: UC (Unlimited Characters)
            0x00, 0x00,             // Reserved, always 0
            0x04, 0x00, 0x00, 0x00, // Length: 4 bytes (LE)
                // String: "Code"
                b'C', b'o', b'd', b'e',
        // UI already tested above
        0x18, 0x00, 0x16, 0x60,     // (0018,6016) (LE) Region Flags
            b'U', b'L',
            0x04, 0x00,             // Length: 4 bytes (LE)
                // Value: 1 (LE)
                0x01, 0x00, 0x00, 0x00,
        0x01, 0xC0, 0x34, 0x12,     // (C001,1234) (LE) (Private data element)
            b'U', b'N',             // VR: UN (Unknown)
            0x00, 0x00,             // Reserved, always 0
            0x06, 0x00, 0x00, 0x00, // Length: 6 bytes (LE)
                // Value: [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        0x08, 0x00, 0x0E, 0x01,     // (0008,010E) (LE) Coding Scheme URL
            b'U', b'R',             // VR: UR (Universal Resource Locator, URL)
            0x00, 0x00,             // Reserved, always 0
            0x12, 0x00, 0x00, 0x00, // Length: 18 bytes (LE)
                // String: "http://example.com"
                b'h', b't', b't', b'p', b':', b'/', b'/', b'e', b'x', b'a', b'm', b'p', b'l', b'e',
                b'.', b'c', b'o', b'm',
        0x08, 0x00, 0x40, 0x00,     // (0008,0040) (LE) Data Set Type
            b'U', b'S',             // VR: US (Unsigned Short)
            0x02, 0x00,             // Length: 2 bytes (LE)
                // Value: 34567 (LE)
                0x07, 0x87,
        0x18, 0x00, 0x17, 0x99,     // (0018,9917) (LE) Instruction Description
            b'U', b'T',             // VR: UT (Unlimited Text)
            0x00, 0x00,             // Reserved, always 0
            0x08, 0x00, 0x00, 0x00, // Length: 8 bytes (LE)
                // String: "No text"
                b'N', b'o', b' ', b't', b'e', b'x', b't',
                b' ',               // Padding to make length even
        0x08, 0x00, 0x0C, 0x04,     // (0008,040C) (LE) File Offset in Container
            b'U', b'V',             // VR: UV (Unsigned Very long)
            0x00, 0x00,             // Reserved, always 0
            0x08, 0x00, 0x00, 0x00, // Length: 8 bytes (LE)
                // Value: 12345678901234567890 (LE)
                0xD2, 0x0A, 0x1F, 0xEB, 0x8C, 0xA9, 0x54, 0xAB,
    ];

    #[test]
    fn decode_data_elements() {
        let dec = ExplicitVRLittleEndianDecoder::default();
        let mut cursor = Cursor::new(RAW.as_ref());

        fn read_n<'a>(cursor: &mut Cursor<&'a [u8]>, n: usize) -> &'a [u8] {
            let pos = cursor.position() as usize;
            let slice = &cursor.get_ref()[pos..pos + n]; // panic if too short
            cursor.set_position((pos + n) as u64);
            slice
        }

        fn test_vr(
            dec: &ExplicitVRLittleEndianDecoder,
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
        assert_eq!(elem.tag(), Tag(0x0002, 0x0010));
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
            &[0x28, 0x00, 0x10, 0x21],
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
            &[0xDB, 0x0F, 0x49, 0x40],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0040,
            0x9225,
            VR::FD,
            &[0x18, 0x2D, 0x44, 0x54, 0xFB, 0x21, 0x09, 0x40],
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
            &[0x18, 0x2D, 0x44, 0x54, 0xFB, 0x21, 0x09, 0x40],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x7FE0,
            0x0008,
            VR::OF,
            &[0xDB, 0x0F, 0x49, 0x40],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0072,
            0x0075,
            VR::OL,
            &[0x78, 0x56, 0x34, 0x12],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0072,
            0x0081,
            VR::OV,
            &[0x80, 0x7F, 0x6E, 0x5D, 0x4C, 0x3B, 0x2A, 0x19],
        );
        test_vr(&dec, &mut cursor, 0x0072, 0x0069, VR::OW, &[0x34, 0x12]);
        test_vr(&dec, &mut cursor, 0x0010, 0x0010, VR::PN, b"Doe^John");
        test_vr(&dec, &mut cursor, 0x0040, 0x9210, VR::SH, b"LBL ");
        test_vr(
            &dec,
            &mut cursor,
            0x0018,
            0x6020,
            VR::SL,
            &[0xB2, 0x9E, 0x43, 0xFF],
        );
        test_vr(
            &dec,
            &mut cursor,
            0x0028,
            0x9503,
            VR::SS,
            &[0x29, 0xEE, 0xE1, 0x10],
        );
        test_vr(&dec, &mut cursor, 0x0040, 0x0280, VR::ST, b"No comment");
        test_vr(
            &dec,
            &mut cursor,
            0x0072,
            0x0082,
            VR::SV,
            &[
                0xB2, 0x0C, 0xCF, 0x59, 0xB4, 0x64, 0x49, 0xFE, 0x4E, 0xF3, 0x30, 0xA6, 0x4B, 0x9B,
                0xB6, 0x01,
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
            &[0x01, 0x00, 0x00, 0x00],
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
        test_vr(&dec, &mut cursor, 0x0008, 0x0040, VR::US, &[0x07, 0x87]);
        test_vr(&dec, &mut cursor, 0x0018, 0x9917, VR::UT, b"No text ");
        test_vr(
            &dec,
            &mut cursor,
            0x0008,
            0x040C,
            VR::UV,
            &[0xD2, 0x0A, 0x1F, 0xEB, 0x8C, 0xA9, 0x54, 0xAB],
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
        0x08, 0x00, 0x3F, 0x10, b'S', b'Q', 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0x00,
        0xE0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0x0D, 0xE0, 0x00, 0x00, 0x00, 0x00, 0xFE, 0xFF,
        0xDD, 0xE0, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn decode_items() {
        let dec = ExplicitVRLittleEndianDecoder::default();
        let mut cursor = Cursor::new(RAW_SEQUENCE_ITEMS);
        {
            // read first element
            let (elem, bytes_read) = dec
                .decode_header(&mut cursor)
                .expect("should find an element header");
            assert_eq!(elem.tag(), Tag(8, 0x103F));
            assert_eq!(elem.vr(), VR::SQ);
            assert!(elem.length().is_undefined());
            assert_eq!(bytes_read, 12);
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
