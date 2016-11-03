//! Implicit VR Big Endian syntax transfer implementation

use byteorder::{ByteOrder, LittleEndian};
use std::io::{Read, Write};
use std::marker::PhantomData;
use attribute::dictionary::{AttributeDictionary, get_standard_dictionary};
use attribute::ValueRepresentation;
use attribute::tag::Tag;
use std::fmt;
use util::Endianness;
use error::Result;
use super::decode::Decode;
use super::encode::Encode;
use data_element::{DataElementHeader, SequenceItemHeader};

#[cfg(test)]
mod tests {
    use super::super::decode::Decode;
    use super::super::encode::Encode;
    use super::ImplicitVRLittleEndianDecoder;
    use super::ImplicitVRLittleEndianEncoder;
    use attribute::dictionary::AttributeDictionary;
    use attribute::dictionary::stub::StubAttributeDictionary;
    use attribute::ValueRepresentation;
    use attribute::tag::Tag;
    use data_element::{Header, DataElement, DataElementHeader};
    use std::io::{Read, Cursor, Seek, SeekFrom, Write};

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
        0x02, 0x00, 0x02, 0x00, 0x1a, 0x00, 0x00, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e, 0x31, 0x2e,
        0x31, 0x00,

        0x02, 0x00, 0x10, 0x00, 0x14, 0x00, 0x00, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e, 0x31, 0x00
    ];

    const DICT: &'static AttributeDictionary<'static> = &StubAttributeDictionary;

    #[test]
    fn implicit_vr_le_works() {
        
        let reader = ImplicitVRLittleEndianDecoder::with_dict(DICT);
        let mut cursor = Cursor::new(RAW.as_ref());
        { // read first element
            let elem = reader.decode_header(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), (0x0002, 0x0002));
            assert_eq!(elem.vr(), ValueRepresentation::UN);
            assert_eq!(elem.len(), 26);
            // read only half of the data
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
            let elem = reader.decode_header(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), (0x0002, 0x0010));
            assert_eq!(elem.vr(), ValueRepresentation::UN);
            assert_eq!(elem.len(), 20);
            // read all data
            let mut buffer: Vec<u8> = Vec::with_capacity(20);
            buffer.resize(20, 0);
            cursor.read_exact(buffer.as_mut_slice()).expect("should read it fine");
            assert_eq!(buffer.as_slice(), b"1.2.840.10008.1.2.1\0".as_ref());
        }
    }

    #[test]
    fn implicit_vr_le_works_with_standard_dictionary() {
        
        let reader = ImplicitVRLittleEndianDecoder::with_default_dict();
        let mut cursor = Cursor::new(RAW.as_ref());
        { // read first element
            let elem = reader.decode_header(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), (2, 2));
            assert_eq!(elem.vr(), ValueRepresentation::UI);
            assert_eq!(elem.len(), 26);
            // cursor should be @ #8
            assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 8);
            // don't read any data, just skip
            // cursor should be @ #34 after skipping
            assert_eq!(cursor.seek(SeekFrom::Current(elem.len() as i64)).unwrap(), 34);
        }
        { // read second element
            let elem = reader.decode_header(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), (2, 16));
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
    fn encode_implicit_vr_le_works() {
        let mut buf = [0u8; 62];
        {
            let enc = ImplicitVRLittleEndianEncoder::default();
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
            let enc = ImplicitVRLittleEndianEncoder::default();
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
/// This type contains a reference to an attribute dictionary for resolving
/// value representations.
pub struct ImplicitVRLittleEndianDecoder<'d, S: Read + ?Sized> {
    dict: &'d AttributeDictionary<'d>,
    phantom: PhantomData<S>,
}

impl<'d, 's, S: Read + ?Sized + 's> fmt::Debug for ImplicitVRLittleEndianDecoder<'d, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ImplicitVRLittleEndianDecoder{{dict={:?},phantom}}", self.dict)
    }
}

impl<'d, 's, S: Read + ?Sized + 's> ImplicitVRLittleEndianDecoder<'d, S> {
    /// Retrieve this decoder using the standard data dictionary.
    pub fn with_default_dict() -> ImplicitVRLittleEndianDecoder<'static, S> {
        ImplicitVRLittleEndianDecoder::<'static, S> {
            dict: get_standard_dictionary(),
            phantom: PhantomData::default()
        }
    }

    /// Retrieve this decoder using a custom data dictionary.
    pub fn with_dict(dictionary: &'d AttributeDictionary<'d>) -> ImplicitVRLittleEndianDecoder<'d, S> {
        ImplicitVRLittleEndianDecoder::<'d, S> {
            dict: dictionary,
            phantom: PhantomData::default()
        }
    }
}

impl<'d, 's, S: Read + ?Sized + 's> Decode for ImplicitVRLittleEndianDecoder<'d, S>  {
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
        let tag = Tag(group, element);
        try!(source.read_exact(&mut buf));
        let len = LittleEndian::read_u32(&buf);
        let vr = self.dict.get_by_tag(tag).map(|entry| entry.vr).unwrap_or(ValueRepresentation::UN);
        Ok(DataElementHeader::new(tag, vr, len))
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
        Ok(LittleEndian::read_i16(&buf[0..2]))
    }

    fn decode_sl(&self, source: &mut Self::Source) -> Result<i32> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf[..]));
        Ok(LittleEndian::read_i32(&buf[..]))
    }
}

pub struct ImplicitVRLittleEndianEncoder<W: Write + ?Sized> {
    phantom: PhantomData<W>
}

impl<W: Write + ?Sized> Default for ImplicitVRLittleEndianEncoder<W> {
    fn default() -> ImplicitVRLittleEndianEncoder<W> {
        ImplicitVRLittleEndianEncoder{ phantom: PhantomData::default() }
    }
}

impl<W: Write + ?Sized> fmt::Debug for ImplicitVRLittleEndianEncoder<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ImplicitVRLittleEndianEncoder")
    }
}

impl<W: Write + ?Sized> Encode for ImplicitVRLittleEndianEncoder<W> {
    type Writer = W;

    fn encode_element_header(&self, de: DataElementHeader, to: &mut W) -> Result<()> {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf[0..], de.tag.group());
        LittleEndian::write_u16(&mut buf[2..], de.tag.element());
        LittleEndian::write_u32(&mut buf[4..], de.len);
        try!(to.write_all(&buf));

        Ok(())
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
