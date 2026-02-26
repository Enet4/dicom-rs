//! Explicit VR Little Endian syntax transfer implementation

use crate::encode::basic::LittleEndianBasicEncoder;
use crate::encode::{
    BasicEncode, Encode, Result, WriteHeaderSnafu, WriteItemDelimiterSnafu, WriteItemHeaderSnafu,
    WriteOffsetTableSnafu, WriteSequenceDelimiterSnafu, WriteTagSnafu,
};
use byteordered::byteorder::{ByteOrder, LittleEndian};
use byteordered::Endianness;
use dicom_core::header::{DataElementHeader, HasLength, Header};
use dicom_core::{PrimitiveValue, Tag, VR};
use snafu::ResultExt;
use std::io::{self, Write};

/// A concrete encoder for the transfer syntax ExplicitVRLittleEndian
#[derive(Debug, Default, Clone)]
pub struct ExplicitVRLittleEndianEncoder {
    basic: LittleEndianBasicEncoder,
}

impl BasicEncode for ExplicitVRLittleEndianEncoder {
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }

    fn encode_us<S>(&self, to: S, value: u16) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_us(to, value)
    }

    fn encode_ul<S>(&self, to: S, value: u32) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_ul(to, value)
    }

    fn encode_uv<S>(&self, to: S, value: u64) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_uv(to, value)
    }

    fn encode_ss<S>(&self, to: S, value: i16) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_ss(to, value)
    }

    fn encode_sl<S>(&self, to: S, value: i32) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_sl(to, value)
    }

    fn encode_sv<S>(&self, to: S, value: i64) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_sv(to, value)
    }

    fn encode_fl<S>(&self, to: S, value: f32) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_fl(to, value)
    }

    fn encode_fd<S>(&self, to: S, value: f64) -> io::Result<()>
    where
        S: Write,
    {
        self.basic.encode_fd(to, value)
    }
}

impl Encode for ExplicitVRLittleEndianEncoder {
    fn encode_tag<W>(&self, mut to: W, tag: Tag) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8, 4];
        LittleEndian::write_u16(&mut buf[..], tag.group());
        LittleEndian::write_u16(&mut buf[2..], tag.element());
        to.write_all(&buf).context(WriteTagSnafu)
    }

    fn encode_element_header<W>(&self, mut to: W, de: DataElementHeader) -> Result<usize>
    where
        W: Write,
    {
        match de.vr() {
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
                let mut buf = [0u8; 8];
                LittleEndian::write_u16(&mut buf[0..], de.tag().group());
                LittleEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                LittleEndian::write_u16(&mut buf[6..], de.length().0 as u16);
                to.write_all(&buf).context(WriteHeaderSnafu)?;
                Ok(8)
            }
            // PS3.5 7.1.2:
            // for all other VRs the 16 bits following the two byte VR Field
            // are reserved for use by later versions of the DICOM Standard.
            // These reserved bytes shall be set to 0000H and shall not be
            // used or decoded (Table 7.1-1). The Value Length Field is a
            // 32-bit unsigned integer.
            _ => {
                let mut buf = [0u8; 12];
                LittleEndian::write_u16(&mut buf[0..], de.tag().group());
                LittleEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                // buf[6..8] is kept zero'd
                LittleEndian::write_u32(&mut buf[8..], de.length().0);
                to.write_all(&buf).context(WriteHeaderSnafu)?;
                Ok(12)
            }
        }
    }

    fn encode_item_header<W>(&self, mut to: W, len: u32) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE000);
        LittleEndian::write_u32(&mut buf[4..], len);
        to.write_all(&buf).context(WriteItemHeaderSnafu)
    }

    fn encode_item_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE00D);
        to.write_all(&buf).context(WriteItemDelimiterSnafu)
    }

    fn encode_sequence_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE0DD);
        to.write_all(&buf).context(WriteSequenceDelimiterSnafu)
    }

    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write,
    {
        self.basic.encode_primitive(to, value)
    }

    fn encode_offset_table<W>(&self, mut to: W, offset_table: &[u32]) -> Result<usize>
    where
        W: Write,
    {
        for v in offset_table {
            self.basic
                .encode_ul(&mut to, *v)
                .context(WriteOffsetTableSnafu)?;
        }
        Ok(offset_table.len() * 4)
    }
}

#[cfg(test)]
mod tests {
    use super::ExplicitVRLittleEndianEncoder;
    use crate::encode::Encode;
    use dicom_core::header::{DataElementHeader, Length};
    use dicom_core::{Tag, VR};
    use std::io::{Cursor, Write};

    type Result = std::result::Result<(), Box<dyn std::error::Error>>;

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
    fn encode_data_elements() {
        let mut buf = vec![0u8; RAW.len()];

        let enc = ExplicitVRLittleEndianEncoder::default();
        let mut writer = Cursor::new(&mut buf);

        // encode first element
        let de = DataElementHeader::new(Tag(0x0002, 0x0002), VR::UI, Length(26));
        let len = enc
            .encode_element_header(&mut writer, de)
            .expect("should write it fine");
        assert_eq!(len, 8);
        writer
            .write_all(b"1.2.840.10008.5.1.4.1.1.1\0".as_ref())
            .expect("should write the value fine");

        // encode second element
        let de = DataElementHeader::new(Tag(0x0002, 0x0010), VR::UI, Length(20));
        let len = enc
            .encode_element_header(&mut writer, de)
            .expect("should write it fine");
        assert_eq!(len, 8);
        writer
            .write_all(b"1.2.840.10008.1.2.1\0".as_ref())
            .expect("should write the value fine");

        fn write_elem(
            enc: &ExplicitVRLittleEndianEncoder,
            mut writer: &mut Cursor<&mut Vec<u8>>,
            group: u16,
            element: u16,
            vr: VR,
            value: &[u8],
        ) {
            let from = writer.position() as usize;
            let de = DataElementHeader::new(
                Tag(group, element),
                vr,
                Length(value.len() as u32),
            );
            let _written_len = enc
                .encode_element_header(&mut writer, de)
                .expect("should write it fine");
            writer.write_all(value).expect("should write the value fine");
            let to = writer.position() as usize;

            // Compare the current slice
            if &writer.get_ref()[from..to] != &RAW[from..to] {
                panic!(
                    "Failure on ({:04x},{:04x})  {:?}  {:02x?}\n\
                    Expected: {:02x?}",
                    group,
                    element,
                    vr,
                    value,
                    &RAW[from..to],
                );
            }
        }

        write_elem(&enc, &mut writer, 0x0008, 0x0054, VR::AE, b"TITLE ");
        write_elem(&enc, &mut writer, 0x0010, 0x1010, VR::AS, b"8Y");
        write_elem(
            &enc,
            &mut writer,
            0x0072,
            0x0026,
            VR::AT,
            &[0x28, 0x00, 0x10, 0x21],
        );
        write_elem(&enc, &mut writer, 0x0010, 0x0040, VR::CS, b"O ");
        write_elem(&enc, &mut writer, 0x0010, 0x0030, VR::DA, b"19800101");
        write_elem(&enc, &mut writer, 0x0010, 0x1020, VR::DS, b"1.70");

        write_elem(
            &enc,
            &mut writer,
            0x0008,
            0x0015,
            VR::DT,
            b"20051231235960",
        );
        write_elem(
            &enc,
            &mut writer,
            0x0010,
            0x9431,
            VR::FL,
            &[0xDB, 0x0F, 0x49, 0x40],
        );
        write_elem(
            &enc,
            &mut writer,
            0x0040,
            0x9225,
            VR::FD,
            &[0x18, 0x2D, 0x44, 0x54, 0xFB, 0x21, 0x09, 0x40],
        );
        write_elem(&enc, &mut writer, 0x0020, 0x0013, VR::IS, b"1234567 ");
        write_elem(&enc, &mut writer, 0x0010, 0x0020, VR::LO, b"P12345678X");
        write_elem(&enc, &mut writer, 0x0010, 0x4000, VR::LT, b"None");
        write_elem(&enc, &mut writer, 0x0008, 0x041B, VR::OB, &[0x12, 0x34]);
        write_elem(
            &enc,
            &mut writer,
            0x7FE0,
            0x0009,
            VR::OD,
            &[0x18, 0x2D, 0x44, 0x54, 0xFB, 0x21, 0x09, 0x40],
        );
        write_elem(
            &enc,
            &mut writer,
            0x7FE0,
            0x0008,
            VR::OF,
            &[0xDB, 0x0F, 0x49, 0x40],
        );
        write_elem(
            &enc,
            &mut writer,
            0x0072,
            0x0075,
            VR::OL,
            &[0x78, 0x56, 0x34, 0x12],
        );
        write_elem(
            &enc,
            &mut writer,
            0x0072,
            0x0081,
            VR::OV,
            &[0x80, 0x7F, 0x6E, 0x5D, 0x4C, 0x3B, 0x2A, 0x19],
        );
        write_elem(&enc, &mut writer, 0x0072, 0x0069, VR::OW, &[0x34, 0x12]);
        write_elem(&enc, &mut writer, 0x0010, 0x0010, VR::PN, b"Doe^John");
        write_elem(&enc, &mut writer, 0x0040, 0x9210, VR::SH, b"LBL ");
        write_elem(
            &enc,
            &mut writer,
            0x0018,
            0x6020,
            VR::SL,
            &[0xB2, 0x9E, 0x43, 0xFF],
        );
        write_elem(
            &enc,
            &mut writer,
            0x0028,
            0x9503,
            VR::SS,
            &[0x29, 0xEE, 0xE1, 0x10],
        );
        write_elem(&enc, &mut writer, 0x0040, 0x0280, VR::ST, b"No comment");
        write_elem(
            &enc,
            &mut writer,
            0x0072,
            0x0082,
            VR::SV,
            &[
                0xB2, 0x0C, 0xCF, 0x59, 0xB4, 0x64, 0x49, 0xFE, 0x4E, 0xF3, 0x30, 0xA6, 0x4B, 0x9B,
                0xB6, 0x01,
            ],
        );
        write_elem(&enc, &mut writer, 0x0010, 0x0032, VR::TM, b"123456");
        write_elem(&enc, &mut writer, 0x0008, 0x0119, VR::UC, b"Code");
        write_elem(
            &enc,
            &mut writer,
            0x0018,
            0x6016,
            VR::UL,
            &[0x01, 0x00, 0x00, 0x00],
        );
        write_elem(
            &enc,
            &mut writer,
            0xC001,
            0x1234,
            VR::UN,
            &[0x1, 0x2, 0x3, 0x4, 0x5, 0x6],
        );
        write_elem(
            &enc,
            &mut writer,
            0x0008,
            0x010E,
            VR::UR,
            b"http://example.com",
        );
        write_elem(&enc, &mut writer, 0x0008, 0x0040, VR::US, &[0x07, 0x87]);
        write_elem(&enc, &mut writer, 0x0018, 0x9917, VR::UT, b"No text ");
        write_elem(
            &enc,
            &mut writer,
            0x0008,
            0x040C,
            VR::UV,
            &[0xD2, 0x0A, 0x1F, 0xEB, 0x8C, 0xA9, 0x54, 0xAB],
        );

        // Final compare of the whole buffer
        assert_eq!(&buf[..], &RAW[..]);
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
    fn encode_items() -> Result {
        let enc = ExplicitVRLittleEndianEncoder::default();
        let mut out = Vec::new();

        {
            let bytes_written = enc.encode_element_header(
                &mut out,
                DataElementHeader::new(Tag(0x0008, 0x103F), VR::SQ, Length::UNDEFINED),
            )?;
            assert_eq!(bytes_written, 12);
        }
        assert_eq!(out.len(), 12);

        enc.encode_item_header(&mut out, Length::UNDEFINED.0)?;
        assert_eq!(out.len(), 20);

        enc.encode_item_delimiter(&mut out)?;
        assert_eq!(out.len(), 28);

        enc.encode_sequence_delimiter(&mut out)?;

        assert_eq!(&out[..], RAW_SEQUENCE_ITEMS);

        Ok(())
    }
}
