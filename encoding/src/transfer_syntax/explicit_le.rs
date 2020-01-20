//! Explicit VR Little Endian syntax transfer implementation

use crate::decode::basic::LittleEndianBasicDecoder;
use crate::decode::{BasicDecode, Decode, DecodeFrom};
use crate::encode::basic::LittleEndianBasicEncoder;
use crate::encode::{BasicEncode, Encode};
use crate::error::Result;
use byteordered::byteorder::{ByteOrder, LittleEndian};
use byteordered::Endianness;
use dicom_core::header::{DataElementHeader, Header, Length, SequenceItemHeader};
use dicom_core::{PrimitiveValue, Tag, VR};
use std::io::{Read, Write};

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
        let Tag(group, element) = self.basic.decode_tag(&mut source)?;

        let mut buf = [0u8; 4];
        if group == 0xFFFE {
            // item delimiters do not have VR or reserved field
            source.read_exact(&mut buf)?;
            let len = LittleEndian::read_u32(&buf);
            return Ok((
                DataElementHeader::new((group, element), VR::UN, Length(len)),
                8, // tag + len
            ));
        }

        // retrieve explicit VR
        source.read_exact(&mut buf[0..2])?;
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
                source.read_exact(&mut buf[0..2])?;
                source.read_exact(&mut buf)?;
                bytes_read = 12;
                LittleEndian::read_u32(&buf)
            }
            _ => {
                // read 2 bytes for the data length
                source.read_exact(&mut buf[0..2])?;
                bytes_read = 8;
                u32::from(LittleEndian::read_u16(&buf[0..2]))
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
        source.read_exact(&mut buf)?;
        // retrieve tag
        let group = LittleEndian::read_u16(&buf[0..2]);
        let element = LittleEndian::read_u16(&buf[2..4]);
        let len = LittleEndian::read_u32(&buf[4..8]);

        let header = SequenceItemHeader::new((group, element), Length(len))?;
        Ok(header)
    }

    fn decode_tag<S>(&self, source: &mut S) -> Result<Tag>
    where
        S: ?Sized + Read,
    {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf)?;
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
    fn decode_header(&self, source: &mut S) -> Result<(DataElementHeader, usize)> {
        Decode::decode_header(self, source)
    }

    fn decode_item_header(&self, source: &mut S) -> Result<SequenceItemHeader> {
        Decode::decode_item_header(self, source)
    }

    fn decode_tag(&self, source: &mut S) -> Result<Tag> {
        Decode::decode_tag(self, source)
    }
}

/// A concrete encoder for the transfer syntax ExplicitVRLittleEndian
#[derive(Debug, Default, Clone)]
pub struct ExplicitVRLittleEndianEncoder {
    basic: LittleEndianBasicEncoder,
}

impl BasicEncode for ExplicitVRLittleEndianEncoder {
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }

    fn encode_us<S>(&self, to: S, value: u16) -> Result<()>
    where
        S: Write,
    {
        self.basic.encode_us(to, value)
    }

    fn encode_ul<S>(&self, to: S, value: u32) -> Result<()>
    where
        S: Write,
    {
        self.basic.encode_ul(to, value)
    }

    fn encode_uv<S>(&self, to: S, value: u64) -> Result<()>
    where
        S: Write,
    {
        self.basic.encode_uv(to, value)
    }

    fn encode_ss<S>(&self, to: S, value: i16) -> Result<()>
    where
        S: Write,
    {
        self.basic.encode_ss(to, value)
    }

    fn encode_sl<S>(&self, to: S, value: i32) -> Result<()>
    where
        S: Write,
    {
        self.basic.encode_sl(to, value)
    }

    fn encode_sv<S>(&self, to: S, value: i64) -> Result<()>
    where
        S: Write,
    {
        self.basic.encode_sv(to, value)
    }

    fn encode_fl<S>(&self, to: S, value: f32) -> Result<()>
    where
        S: Write,
    {
        self.basic.encode_fl(to, value)
    }

    fn encode_fd<S>(&self, to: S, value: f64) -> Result<()>
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
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_element_header<W>(&self, mut to: W, de: DataElementHeader) -> Result<usize>
    where
        W: Write,
    {
        match de.vr() {
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
                let mut buf = [0u8; 12];
                LittleEndian::write_u16(&mut buf[0..], de.tag().group());
                LittleEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                // buf[6..8] is kept zero'd
                LittleEndian::write_u32(&mut buf[8..], de.len().0);
                to.write_all(&buf)?;
                Ok(12)
            }
            _ => {
                let mut buf = [0u8; 8];
                LittleEndian::write_u16(&mut buf[0..], de.tag().group());
                LittleEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                LittleEndian::write_u16(&mut buf[6..], de.len().0 as u16);
                to.write_all(&buf)?;
                Ok(8)
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
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_item_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE00D);
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_sequence_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf[2..], 0xE0DD);
        to.write_all(&buf)?;
        Ok(())
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
    use super::ExplicitVRLittleEndianDecoder;
    use super::ExplicitVRLittleEndianEncoder;
    use crate::decode::Decode;
    use crate::encode::Encode;
    use dicom_core::header::{DataElementHeader, Header, Length};
    use dicom_core::{Tag, VR};
    use std::io::{Cursor, Read, Seek, SeekFrom, Write};

    // manually crafting some DICOM data elements
    //  Tag: (0002,0002) Media Storage SOP Class UID
    //  VR: UI
    //  Length: 26
    //  Value: "1.2.840.10008.5.1.4.1.1.1\0"
    // --
    //  Tag: (0002,0010) Transfer Syntax UID
    //  VR: UI
    //  Length: 20
    //  Value: "1.2.840.10008.1.2.1\0" == ExplicitVRLittleEndian
    // --
    const RAW: &'static [u8; 62] = &[
        0x02, 0x00, 0x02, 0x00, 0x55, 0x49, 0x1a, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30,
        0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e,
        0x31, 0x2e, 0x31, 0x00, 0x02, 0x00, 0x10, 0x00, 0x55, 0x49, 0x14, 0x00, 0x31, 0x2e, 0x32,
        0x2e, 0x38, 0x34, 0x30, 0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e,
        0x31, 0x00,
    ];

    #[test]
    fn decode_data_elements() {
        let dec = ExplicitVRLittleEndianDecoder::default();
        let mut cursor = Cursor::new(RAW.as_ref());
        {
            // read first element
            let (elem, bytes_read) = dec
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.len(), Length(26));
            assert_eq!(bytes_read, 8);
            // read only half of the value data
            let mut buffer: Vec<u8> = Vec::with_capacity(13);
            buffer.resize(13, 0);
            cursor
                .read_exact(buffer.as_mut_slice())
                .expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008".as_ref());
        }
        // cursor should now be @ #21 (there is no automatic skipping)
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 21);
        // cursor should now be @ #34 after skipping
        assert_eq!(cursor.seek(SeekFrom::Current(13)).unwrap(), 34);
        {
            // read second element
            let (elem, _bytes_read) = dec
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 16));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.len(), Length(20));
            // read all data
            let mut buffer: Vec<u8> = Vec::with_capacity(20);
            buffer.resize(20, 0);
            cursor
                .read_exact(buffer.as_mut_slice())
                .expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008.1.2.1\0".as_ref());
        }
    }

    #[test]
    fn encode_data_elements() {
        let mut buf = [0u8; 62];
        {
            let enc = ExplicitVRLittleEndianEncoder::default();
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
            let enc = ExplicitVRLittleEndianEncoder::default();
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
            assert!(elem.len().is_undefined());
            assert_eq!(bytes_read, 12);
        }
        // cursor should now be @ #12
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 12);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE000));
            assert!(elem.len().is_undefined());
        }
        // cursor should now be @ #20
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 20);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE00D));
            assert_eq!(elem.len(), Length(0));
        }
        // cursor should now be @ #28
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 28);
        {
            let elem = dec
                .decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_sequence_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE0DD));
            assert_eq!(elem.len(), Length(0));
        }
    }
}
