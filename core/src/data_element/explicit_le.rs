//! Explicit VR Little Endian syntax transfer implementation

use std::io::{Read, Write};
use std::fmt;
use std::marker::PhantomData;
use attribute::ValueRepresentation;
use byteorder::{ByteOrder, LittleEndian};
use error::Result;
use super::decode::Decode;
use super::encode::Encode;
use data_element::{DataElementHeader, SequenceItemHeader};
use util::Endianness;

#[cfg(test)]
mod tests {
    use super::super::decode::Decode;
    use super::ExplicitVRLittleEndianDecoder;
    use super::super::encode::Encode;
    use super::ExplicitVRLittleEndianEncoder;
    use data_element::{Header, DataElement, DataElementHeader};
    use attribute::tag::Tag;
    use attribute::ValueRepresentation;
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
            assert_eq!(elem.vr(), ValueRepresentation::UI);
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
            assert_eq!(elem.vr(), ValueRepresentation::UI);
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
            let de = DataElementHeader {
                tag: Tag(0x0002,0x0002),
                vr: ValueRepresentation::UI,
                len: 26,
            };
            enc.encode_element_header(de, &mut writer).expect("should write it fine");
            writer.write_all(b"1.2.840.10008.5.1.4.1.1.1\0".as_ref()).expect("should write the value fine");
        }
        assert_eq!(&buf[0..8], &RAW[0..8]);
        {
            let enc = ExplicitVRLittleEndianEncoder::default();
            let mut writer = Cursor::new(&mut buf[34..]);

            // encode second element
            let de = DataElementHeader {
                tag: Tag(0x0002,0x0010),
                vr: ValueRepresentation::UI,
                len: 20,
            };
            enc.encode_element_header(de, &mut writer).expect("should write it fine");
            writer.write_all(b"1.2.840.10008.1.2.1\0".as_ref()).expect("should write the value fine");
        }
        assert_eq!(&buf[34..42], &RAW[34..42]);

        assert_eq!(&buf[..], &RAW[..]);
    }
}

/// A data element decoder for the Explicit VR Little Endian transfer syntax.
pub struct ExplicitVRLittleEndianDecoder<S: Read + ?Sized> {
    phantom: PhantomData<S>
}

impl<S: Read + ?Sized> Default for ExplicitVRLittleEndianDecoder<S> {
    fn default() -> ExplicitVRLittleEndianDecoder<S> {
        ExplicitVRLittleEndianDecoder{ phantom: PhantomData::default() }
    }
}

impl<S: Read + ?Sized> fmt::Debug for ExplicitVRLittleEndianDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExplicitVRLittleEndianDecoder")
    }
}

impl<S: Read + ?Sized> Decode for ExplicitVRLittleEndianDecoder<S> {
    type Source = S;

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn decode_header(&self, source: &mut S) -> Result<DataElementHeader> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf));
        // retrieve tag
        let group = LittleEndian::read_u16(&buf[0..2]);
        let element = LittleEndian::read_u16(&buf[2..4]);

        // retrieve explicit VR
        try!(source.read_exact(&mut buf[0..2]));
        let vr = ValueRepresentation::from_binary([buf[0], buf[1]]).unwrap_or(ValueRepresentation::UN);

        // retrieve data length
        let len = match vr {
            ValueRepresentation::OB | ValueRepresentation::OD |
            ValueRepresentation::OF | ValueRepresentation::OL |
            ValueRepresentation::OW | ValueRepresentation::SQ |
            ValueRepresentation::UC | ValueRepresentation::UR |
            ValueRepresentation::UT | ValueRepresentation::UN => {
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

    fn decode_us(&self, source: &mut Self::Source) -> Result<u16> {
        let mut buf = [0u8; 2];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_u16(&buf[..]))
    }

    fn decode_ul(&self, source: &mut Self::Source) -> Result<u32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_u32(&buf[..]))
    }

    fn decode_ss(&self, source: &mut Self::Source) -> Result<i16> {
        let mut buf = [0u8; 2];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_i16(&buf[..]))
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_i32(&buf[..]))
    }
}

pub struct ExplicitVRLittleEndianEncoder<W: Write + ?Sized> {
    phantom: PhantomData<W>
}

impl<W: Write + ?Sized> Default for ExplicitVRLittleEndianEncoder<W> {
    fn default() -> ExplicitVRLittleEndianEncoder<W> {
        ExplicitVRLittleEndianEncoder{ phantom: PhantomData::default() }
    }
}

impl<W: Write + ?Sized> fmt::Debug for ExplicitVRLittleEndianEncoder<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExplicitVRLittleEndianEncoder")
    }
}

impl<W: Write + ?Sized> Encode for ExplicitVRLittleEndianEncoder<W> {
    type Writer = W;

    fn encode_element_header(&self, de: DataElementHeader, to: &mut W) -> Result<()> {
        match de.vr {
            ValueRepresentation::OB | ValueRepresentation::OD |
            ValueRepresentation::OF | ValueRepresentation::OL |
            ValueRepresentation::OW | ValueRepresentation::SQ |
            ValueRepresentation::UC | ValueRepresentation::UR |
            ValueRepresentation::UT | ValueRepresentation::UN => {

                let mut buf = [0u8 ; 12];
                LittleEndian::write_u16(&mut buf[0..], de.tag.group());
                LittleEndian::write_u16(&mut buf[2..], de.tag.element());
                let vr_bytes = de.vr.to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                // buf[6..8] is kept zero'd
                LittleEndian::write_u32(&mut buf[8..], de.len);
                try!(to.write_all(&buf));

                Ok(())
            },
            _ => {
                let mut buf = [0u8; 8];
                LittleEndian::write_u16(&mut buf[0..], de.tag.group());
                LittleEndian::write_u16(&mut buf[2..], de.tag.element());
                let vr_bytes = de.vr.to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                LittleEndian::write_u16(&mut buf[6..], de.len as u16);
                try!(to.write_all(&buf));

                Ok(())
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
