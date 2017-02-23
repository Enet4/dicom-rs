//! Explicit VR Little Endian syntax transfer implementation

use std::io::{Read, Write};
use std::fmt;
use attribute::VR;
use attribute::tag::Tag;
use byteorder::{ByteOrder, LittleEndian};
use error::Result;
use data::decode::{BasicDecode, Decode};
use data::decode::basic::LittleEndianBasicDecoder;
use data::encode::{BasicEncode, Encode};
use data::encode::basic::LittleEndianBasicEncoder;
use data::{DataElementHeader, SequenceItemHeader, Header};
use util::Endianness;

/// A data element decoder for the Explicit VR Little Endian transfer syntax.
pub struct ExplicitVRLittleEndianDecoder<S: Read + ?Sized> {
    basic: LittleEndianBasicDecoder<S>,
}

impl<S: Read + ?Sized> Default for ExplicitVRLittleEndianDecoder<S> {
    fn default() -> ExplicitVRLittleEndianDecoder<S> {
        ExplicitVRLittleEndianDecoder{ basic: LittleEndianBasicDecoder::default() }
    }
}

impl<S: Read + ?Sized> fmt::Debug for ExplicitVRLittleEndianDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExplicitVRLittleEndianDecoder")
    }
}

impl<S: Read + ?Sized> Decode for ExplicitVRLittleEndianDecoder<S> {
    type Source = S;

    fn decode_header(&self, source: &mut S) -> Result<DataElementHeader> {
        // retrieve tag
        let Tag(group, element) = try!(self.basic.decode_tag(source));

        let mut buf = [0u8; 4];
        // retrieve explicit VR
        try!(source.read_exact(&mut buf[0..2]));
        let vr = VR::from_binary([buf[0], buf[1]]).unwrap_or(VR::UN);

        // retrieve data length
        let len = match vr {
            VR::OB | VR::OD |
            VR::OF | VR::OL |
            VR::OW | VR::SQ |
            VR::UC | VR::UR |
            VR::UT | VR::UN => {
                // read 2 reserved bytes, then 4 bytes for data length
                try!(source.read_exact(&mut buf[0..2]));
                try!(source.read_exact(&mut buf));
                LittleEndian::read_u32(&buf)
            },
            _ => {
                // read 2 bytes for the data length
                try!(source.read_exact(&mut buf[0..2]));
                LittleEndian::read_u16(&buf[0..2]) as u32
            }
        };

        Ok(DataElementHeader::new((group, element), vr, len))
    }

    fn decode_item_header(&self, source: &mut S) -> Result<SequenceItemHeader> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf));
        // retrieve tag
        let group = LittleEndian::read_u16(&buf[0..2]);
        let element = LittleEndian::read_u16(&buf[2..4]);

        try!(source.read_exact(&mut buf));
        let len = LittleEndian::read_u32(&buf);

        SequenceItemHeader::new((group, element), len)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf));
        Ok(Tag(
            LittleEndian::read_u16(&buf[0..2]),
            LittleEndian::read_u16(&buf[2..4])
        ))
    }
}

/// A concrete encoder for the transfer syntax ExplicitVRLittleEndian
pub struct ExplicitVRLittleEndianEncoder<W: Write + ?Sized> {
    basic: LittleEndianBasicEncoder<W>
}

impl<W: Write + ?Sized> Default for ExplicitVRLittleEndianEncoder<W> {
    fn default() -> ExplicitVRLittleEndianEncoder<W> {
        ExplicitVRLittleEndianEncoder{ basic: LittleEndianBasicEncoder::default() }
    }
}

impl<W: Write + ?Sized> fmt::Debug for ExplicitVRLittleEndianEncoder<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExplicitVRLittleEndianEncoder")
    }
}

impl<W: Write + ?Sized> BasicEncode for ExplicitVRLittleEndianEncoder<W> {
    type Writer = W;

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn encode_us(&self, value: u16, to: &mut Self::Writer) -> Result<()> {
        self.basic.encode_us(value, to)
    }

    fn encode_ul(&self, value: u32, to: &mut Self::Writer) -> Result<()> {
        self.basic.encode_ul(value, to)
    }

    fn encode_ss(&self, value: i16, to: &mut Self::Writer) -> Result<()> {
        self.basic.encode_ss(value, to)
    }

    fn encode_sl(&self, value: i32, to: &mut Self::Writer) -> Result<()> {
        self.basic.encode_sl(value, to)
    }

    fn encode_fl(&self, value: f32, to: &mut Self::Writer) -> Result<()> {
        self.basic.encode_fl(value, to)
    }
    
    fn encode_fd(&self, value: f64, to: &mut Self::Writer) -> Result<()> {
        self.basic.encode_fd(value, to)
    }
}

impl<W: Write + ?Sized> Encode for ExplicitVRLittleEndianEncoder<W> {

    fn encode_element_header(&self, de: DataElementHeader, to: &mut W) -> Result<usize> {
        match de.vr() {
            VR::OB | VR::OD |
            VR::OF | VR::OL |
            VR::OW | VR::SQ |
            VR::UC | VR::UR |
            VR::UT | VR::UN => {
                let mut buf = [0u8 ; 12];
                LittleEndian::write_u16(&mut buf[0..], de.tag().group());
                LittleEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                // buf[6..8] is kept zero'd
                LittleEndian::write_u32(&mut buf[8..], de.len());
                try!(to.write_all(&buf));
                Ok(12)
            },
            _ => {
                let mut buf = [0u8; 8];
                LittleEndian::write_u16(&mut buf[0..], de.tag().group());
                LittleEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                LittleEndian::write_u16(&mut buf[6..], de.len() as u16);
                try!(to.write_all(&buf));
                Ok(8)
            }
        }
    }

    fn encode_item_header(&self, len: u32, to: &mut W) -> Result<()> {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf, 0xE000);
        LittleEndian::write_u32(&mut buf[4..], len);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_item_delimiter(&self, to: &mut W) -> Result<()> {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf, 0xE00D);
        LittleEndian::write_u32(&mut buf[4..], 0);
        try!(to.write_all(&buf));
        Ok(())
    }

    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()> {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf, 0xE0DD);
        LittleEndian::write_u32(&mut buf[4..], 0);
        try!(to.write_all(&buf));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ExplicitVRLittleEndianDecoder;
    use super::ExplicitVRLittleEndianEncoder;
    use data::{Header, DataElementHeader};
    use data::decode::Decode;
    use data::encode::Encode;
    use attribute::tag::Tag;
    use attribute::VR;
    use std::io::{Read, Cursor, Seek, SeekFrom, Write};

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
        0x02, 0x00, 0x02, 0x00, 0x55, 0x49, 0x1a, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e, 0x31, 0x2e,
        0x31, 0x00,

        0x02, 0x00, 0x10, 0x00, 0x55, 0x49, 0x14, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e, 0x31, 0x00
    ];

    #[test]
    fn decode_explicit_vr_le_works() {
        
        let dec = ExplicitVRLittleEndianDecoder::default();
        let mut cursor = Cursor::new(RAW.as_ref());
        { // read first element
            let elem = dec.decode_header(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.len(), 26);
            // read only half of the value data
            let mut buffer: Vec<u8> = Vec::with_capacity(13);
            buffer.resize(13, 0);
            cursor.read_exact(buffer.as_mut_slice()).expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008".as_ref());
        }
        // cursor should now be @ #21 (there is no automatic skipping)
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 21);
        // cursor should now be @ #34 after skipping
        assert_eq!(cursor.seek(SeekFrom::Current(13)).unwrap(), 34);
        { // read second element
            let elem = dec.decode_header(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 16));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.len(), 20);
            // read all data
            let mut buffer: Vec<u8> = Vec::with_capacity(20);
            buffer.resize(20, 0);
            cursor.read_exact(buffer.as_mut_slice()).expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008.1.2.1\0".as_ref());
        }
    }

    #[test]
    fn encode_explicit_vr_le_works() {
        let mut buf = [0u8; 62];
        {
            let enc = ExplicitVRLittleEndianEncoder::default();
            let mut writer = Cursor::new(&mut buf[..]);

            // encode first element
            let de = DataElementHeader::new(
                Tag(0x0002,0x0002),
                VR::UI,
                26
            );
            let len = enc.encode_element_header(de, &mut writer).expect("should write it fine");
            assert_eq!(len, 8);
            writer.write_all(b"1.2.840.10008.5.1.4.1.1.1\0".as_ref()).expect("should write the value fine");
        }
        assert_eq!(&buf[0..8], &RAW[0..8]);
        {
            let enc = ExplicitVRLittleEndianEncoder::default();
            let mut writer = Cursor::new(&mut buf[34..]);

            // encode second element
            let de = DataElementHeader::new(
                Tag(0x0002,0x0010),
                VR::UI,
                20
            );
            let len = enc.encode_element_header(de, &mut writer).expect("should write it fine");
            assert_eq!(len, 8);
            writer.write_all(b"1.2.840.10008.1.2.1\0".as_ref()).expect("should write the value fine");
        }
        assert_eq!(&buf[34..42], &RAW[34..42]);

        assert_eq!(&buf[..], &RAW[..]);
    }
}
