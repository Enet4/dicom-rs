//! Implicit VR Big Endian syntax transfer implementation

use crate::encode::basic::LittleEndianBasicEncoder;
use crate::encode::*;
use byteordered::byteorder::{ByteOrder, LittleEndian};
use byteordered::Endianness;
use dicom_core::header::{DataElementHeader, HasLength, Header};
use dicom_core::{PrimitiveValue, Tag};
use std::io::{self, Write};

/// A concrete encoder for the transfer syntax ImplicitVRLittleEndian
#[derive(Debug, Default, Clone)]
pub struct ImplicitVRLittleEndianEncoder {
    basic: LittleEndianBasicEncoder,
}

impl BasicEncode for ImplicitVRLittleEndianEncoder {
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

impl Encode for ImplicitVRLittleEndianEncoder {
    fn encode_tag<W>(&self, mut to: W, tag: Tag) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8, 4];
        LittleEndian::write_u16(&mut buf[..], tag.group());
        LittleEndian::write_u16(&mut buf[2..], tag.element());
        to.write_all(&buf).context(WriteTag)
    }

    fn encode_element_header<W>(&self, mut to: W, de: DataElementHeader) -> Result<usize>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf[0..], de.tag().group());
        LittleEndian::write_u16(&mut buf[2..], de.tag().element());
        LittleEndian::write_u32(&mut buf[4..], de.length().0);
        to.write_all(&buf).context(WriteHeader)?;
        Ok(8)
    }

    fn encode_item_header<W>(&self, mut to: W, len: u32) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE000);
        LittleEndian::write_u32(&mut buf[4..], len);
        to.write_all(&buf).context(WriteItemHeader)
    }

    fn encode_item_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE00D);
        to.write_all(&buf).context(WriteItemDelimiter)
    }

    fn encode_sequence_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE0DD);
        to.write_all(&buf).context(WriteSequenceDelimiter)
    }

    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<usize>
    where
        W: Write,
    {
        self.basic.encode_primitive(to, value)
    }
}

#[cfg(test)]
mod tests {
    use super::ImplicitVRLittleEndianEncoder;
    use crate::encode::Encode;
    use dicom_core::header::{DataElementHeader, Length, Tag, VR};
    use std::io::{Cursor, Write};

    type Result = std::result::Result<(), Box<dyn std::error::Error>>;

    // manually crafting some DICOM data elements
    //   Tag: (0002,0002) Media Storage SOP Class UID
    //   Length: 26
    //   Value: "1.2.840.10008.5.1.4.1.1.1" (with 1 padding '\0')
    // --
    //   Tag: (0002,0010) Transfer Syntax UID
    //   Length: 20
    //   Value: "1.2.840.10008.1.2.1" (w 1 padding '\0') == ExplicitVRLittleEndian
    // --
    const RAW: &'static [u8; 62] = &[
        0x02, 0x00, 0x02, 0x00, 0x1a, 0x00, 0x00, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30,
        0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e,
        0x31, 0x2e, 0x31, 0x00, 0x02, 0x00, 0x10, 0x00, 0x14, 0x00, 0x00, 0x00, 0x31, 0x2e, 0x32,
        0x2e, 0x38, 0x34, 0x30, 0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e,
        0x31, 0x00,
    ];

    #[test]
    fn encode_implicit_vr_le() {
        let mut buf = [0u8; 62];
        {
            let enc = ImplicitVRLittleEndianEncoder::default();
            let mut writer = Cursor::new(&mut buf[..]);

            // encode first element
            let de = DataElementHeader::new(Tag(0x0002, 0x0002), VR::UI, Length(26));
            let len = enc
                .encode_element_header(&mut writer, de)
                .expect("should write it fine");
            assert_eq!(len, 8);
            writer
                .write_all(b"1.2.840.10008.5.1.4.1.1.1\0".as_ref())
                .expect("should write the value fine");
        }
        assert_eq!(&buf[0..8], &RAW[0..8]);
        {
            let enc = ImplicitVRLittleEndianEncoder::default();
            let mut writer = Cursor::new(&mut buf[34..]);

            // encode second element
            let de = DataElementHeader::new(Tag(0x0002, 0x0010), VR::UI, Length(20));
            let len = enc
                .encode_element_header(&mut writer, de)
                .expect("should write it fine");
            assert_eq!(len, 8);
            writer
                .write_all(b"1.2.840.10008.1.2.1\0".as_ref())
                .expect("should write the value fine");
        }
        assert_eq!(&buf[34..42], &RAW[34..42]);

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
    const RAW_SEQUENCE_ITEMS: &'static [u8] = &[
        0x08, 0x00, 0x3F, 0x10, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFE, 0xFF, 0x0D, 0xE0, 0x00, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xDD, 0xE0, 0x00, 0x00,
        0x00, 0x00,
    ];

    #[test]
    fn encode_items() -> Result {
        let enc = ImplicitVRLittleEndianEncoder::default();
        let mut out = Vec::new();

        {
            let bytes_written = enc.encode_element_header(
                &mut out,
                DataElementHeader::new(Tag(0x0008, 0x103F), VR::SQ, Length::UNDEFINED),
            )?;
            assert_eq!(bytes_written, 8);
        }
        assert_eq!(out.len(), 8);

        enc.encode_item_header(&mut out, Length::UNDEFINED.0)?;
        assert_eq!(out.len(), 16);

        enc.encode_item_delimiter(&mut out)?;
        assert_eq!(out.len(), 24);

        enc.encode_sequence_delimiter(&mut out)?;

        assert_eq!(&out[..], RAW_SEQUENCE_ITEMS,);

        Ok(())
    }
}
