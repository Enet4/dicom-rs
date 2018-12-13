//! Explicit VR Big Endian syntax transfer implementation.

use byteordered::Endianness;
use byteordered::byteorder::{BigEndian, ByteOrder};
use decode::basic::BigEndianBasicDecoder;
use decode::{BasicDecode, Decode};
use encode::basic::BigEndianBasicEncoder;
use encode::{BasicEncode, Encode};
use dicom_core::{Tag, VR};
use dicom_core::header::{DataElementHeader, Header, Length, SequenceItemHeader};
use error::Result;
use std::fmt;
use std::io::{Read, Write};
use std::marker::PhantomData;

/// A data element decoder for the Explicit VR Big Endian transfer syntax.
pub struct ExplicitVRBigEndianDecoder<S: ?Sized> {
    basic: BigEndianBasicDecoder,
    phantom: PhantomData<S>,
}

impl<S: ?Sized> Default for ExplicitVRBigEndianDecoder<S> {
    fn default() -> ExplicitVRBigEndianDecoder<S> {
        ExplicitVRBigEndianDecoder {
            basic: BigEndianBasicDecoder,
            phantom: PhantomData,
        }
    }
}

impl<S: ?Sized> fmt::Debug for ExplicitVRBigEndianDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ExplicitVRLittleEndianDecoder")
            .field("basic", &self.basic)
            .finish()
    }
}

impl<S: ?Sized> Decode for ExplicitVRBigEndianDecoder<S>
where
    S: Read,
{
    type Source = S;

    fn decode_header(&self, mut source: &mut Self::Source) -> Result<DataElementHeader> {
        // retrieve tag
        let Tag(group, element) = self.basic.decode_tag(&mut source)?;

        let mut buf = [0u8; 4];
        if group == 0xFFFE {
            // item delimiters do not have VR or reserved field
            source.read_exact(&mut buf)?;
            let len = BigEndian::read_u32(&buf);
            return Ok(DataElementHeader::new(
                (group, element),
                VR::UN,
                Length(len),
            ));
        }

        // retrieve explicit VR
        source.read_exact(&mut buf[0..2])?;
        let vr = VR::from_binary([buf[0], buf[1]]).unwrap_or(VR::UN);

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
                BigEndian::read_u32(&buf)
            }
            _ => {
                // read 2 bytes for the data length
                source.read_exact(&mut buf[0..2])?;
                u32::from(BigEndian::read_u16(&buf[0..2]))
            }
        };

        Ok(DataElementHeader::new((group, element), vr, Length(len)))
    }

    fn decode_item_header(&self, source: &mut Self::Source) -> Result<SequenceItemHeader> {
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf)?;
        // retrieve tag
        let group = BigEndian::read_u16(&buf[0..2]);
        let element = BigEndian::read_u16(&buf[2..4]);
        let len = BigEndian::read_u32(&buf[4..8]);

        let header = SequenceItemHeader::new((group, element), Length(len))?;
        Ok(header)
    }

    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf)?;
        Ok(Tag(
            BigEndian::read_u16(&buf[0..2]),
            BigEndian::read_u16(&buf[2..4]),
        ))
    }
}

/// A concrete encoder for the transfer syntax ExplicitVRBigEndian
pub struct ExplicitVRBigEndianEncoder<W: ?Sized> {
    basic: BigEndianBasicEncoder,
    phantom: PhantomData<W>,
}

impl<W: ?Sized> Default for ExplicitVRBigEndianEncoder<W> {
    fn default() -> ExplicitVRBigEndianEncoder<W> {
        ExplicitVRBigEndianEncoder {
            basic: BigEndianBasicEncoder,
            phantom: PhantomData,
        }
    }
}

impl<W: ?Sized + fmt::Debug> fmt::Debug for ExplicitVRBigEndianEncoder<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ExplicitVRLittleEndianDecoder")
            .field("basic", &self.basic)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<W: ?Sized> BasicEncode for ExplicitVRBigEndianEncoder<W> {
    fn endianness(&self) -> Endianness {
        Endianness::Big
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

impl<W: ?Sized> Encode for ExplicitVRBigEndianEncoder<W>
where
    W: Write,
{
    type Writer = W;

    fn encode_tag(&self, to: &mut W, tag: Tag) -> Result<()> {
        let mut buf = [0u8, 4];
        BigEndian::write_u16(&mut buf[..], tag.group());
        BigEndian::write_u16(&mut buf[2..], tag.element());
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_element_header(&self, to: &mut W, de: DataElementHeader) -> Result<usize> {
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
                BigEndian::write_u16(&mut buf[0..], de.tag().group());
                BigEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                // buf[6..8] is kept zero'd
                BigEndian::write_u32(&mut buf[8..], de.len().0);
                try!(to.write_all(&buf));

                Ok(12)
            }
            _ => {
                let mut buf = [0u8; 8];
                BigEndian::write_u16(&mut buf[0..], de.tag().group());
                BigEndian::write_u16(&mut buf[2..], de.tag().element());
                let vr_bytes = de.vr().to_bytes();
                buf[4] = vr_bytes[0];
                buf[5] = vr_bytes[1];
                BigEndian::write_u16(&mut buf[6..], de.len().0 as u16);
                try!(to.write_all(&buf));

                Ok(8)
            }
        }
    }

    fn encode_item_header(&self, to: &mut W, len: u32) -> Result<()> {
        let mut buf = [0u8; 8];
        BigEndian::write_u16(&mut buf, 0xFFFE);
        BigEndian::write_u16(&mut buf, 0xE000);
        BigEndian::write_u32(&mut buf[4..], len);
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_item_delimiter(&self, to: &mut W) -> Result<()> {
        let mut buf = [0u8; 8];
        BigEndian::write_u16(&mut buf, 0xFFFE);
        BigEndian::write_u16(&mut buf, 0xE00D);
        // remaining bytes are already zero, so it's ready to write
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()> {
        let mut buf = [0u8; 8];
        BigEndian::write_u16(&mut buf, 0xFFFE);
        BigEndian::write_u16(&mut buf, 0xE0DD);
        // remaining bytes are already zero, so it's ready to write
        to.write_all(&buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ExplicitVRBigEndianDecoder;
    use super::ExplicitVRBigEndianEncoder;
    use decode::Decode;
    use encode::Encode;
    use dicom_core::{Tag, VR};
    use dicom_core::header::{DataElementHeader, Header, Length};
    use std::io::{Cursor, Read, Seek, SeekFrom, Write};

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
            let elem = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.len(), Length(26));
            // read only half of the data
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
            let elem = reader
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
    fn encode_explicit_vr_be() {
        let mut buf = [0u8; 62];
        {
            let enc = ExplicitVRBigEndianEncoder::default();
            let mut writer = Cursor::new(&mut buf[..]);

            // encode first element
            let de = DataElementHeader::new(Tag(0x0002, 0x0002), VR::UI, Length(26));
            let len = enc.encode_element_header(&mut writer, de)
                .expect("should write it fine");
            assert_eq!(len, 8);
            writer
                .write_all(b"1.2.840.10008.5.1.4.1.1.1\0".as_ref())
                .expect("should write the value fine");
        }
        assert_eq!(&buf[0..8], &RAW[0..8]);
        {
            let enc = ExplicitVRBigEndianEncoder::default();
            let mut writer = Cursor::new(&mut buf[34..]);

            // encode second element
            let de = DataElementHeader::new(Tag(0x0002, 0x0010), VR::UI, Length(20));
            let len = enc.encode_element_header(&mut writer, de)
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
            let elem = dec.decode_header(&mut cursor)
                .expect("should find an element header");
            assert_eq!(elem.tag(), Tag(8, 0x103F));
            assert_eq!(elem.vr(), VR::SQ);
            assert!(elem.len().is_undefined());
        }
        // cursor should now be @ #12
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 12);
        {
            let elem = dec.decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE000));
            assert!(elem.len().is_undefined());
        }
        // cursor should now be @ #20
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 20);
        {
            let elem = dec.decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_item_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE00D));
            assert_eq!(elem.len(), Length(0));
        }
        // cursor should now be @ #28
        assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 28);
        {
            let elem = dec.decode_item_header(&mut cursor)
                .expect("should find an item header");
            assert!(elem.is_sequence_delimiter());
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE0DD));
            assert_eq!(elem.len(), Length(0));
        }
    }
}
