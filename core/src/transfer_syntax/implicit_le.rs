//! Implicit VR Big Endian syntax transfer implementation

use byteorder::{ByteOrder, LittleEndian};
use std::io::{Read, Write};
use dictionary::{DataDictionary, DictionaryEntry, get_standard_dictionary};
use dictionary::standard::StandardDataDictionary;
use data::VR;
use data::Tag;
use std::fmt;
use util::Endianness;
use error::Result;
use data::decode::basic::LittleEndianBasicDecoder;
use data::decode::{BasicDecode, Decode};
use data::encode::basic::LittleEndianBasicEncoder;
use data::encode::{BasicEncode, Encode};
use data::{DataElementHeader, SequenceItemHeader, Header};

pub type StandardImplicitVRLittleEndianDecoder<S>
    = ImplicitVRLittleEndianDecoder<'static, S, StandardDataDictionary>;

/// A data element decoder for the Explicit VR Little Endian transfer syntax.
/// This type contains a reference to an attribute dictionary for resolving
/// value representations.
pub struct ImplicitVRLittleEndianDecoder<'d, S: ?Sized, D: 'd> {
    dict: &'d D,
    basic: LittleEndianBasicDecoder<S>,
}

impl<'d, 's, S: ?Sized + 's, D: 'd> fmt::Debug for ImplicitVRLittleEndianDecoder<'d, S, D> 
    where D: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ImplicitVRLittleEndianDecoder{{dict={:?}, {:?}}}", self.dict, self.basic)
    }
}

impl<'s , S: ?Sized + 's> ImplicitVRLittleEndianDecoder<'static, S, StandardDataDictionary>
    where S: Read {

    /// Retrieve this decoder using the standard data dictionary.
    pub fn with_default_dict() -> Self {
        ImplicitVRLittleEndianDecoder {
            dict: get_standard_dictionary(),
            basic: LittleEndianBasicDecoder::default()
        }
    }
}

impl<'s , S: ?Sized + 's> Default for ImplicitVRLittleEndianDecoder<'static, S, StandardDataDictionary>
    where S: Read
{
    fn default() -> Self {
        ImplicitVRLittleEndianDecoder::with_default_dict()
    }
}

impl<'d, 's, S: ?Sized + 's, D: 'd> ImplicitVRLittleEndianDecoder<'d, S, D>
    where S: Read, D: DataDictionary
{
    /// Retrieve this decoder using a custom data dictionary.
    pub fn with_dict(dictionary: &'d D) -> Self {
        ImplicitVRLittleEndianDecoder {
            dict: dictionary,
            basic: LittleEndianBasicDecoder::default()
        }
    }
}

impl<'d, S: ?Sized, D> Decode for ImplicitVRLittleEndianDecoder<'d, S, D>
    where S: Read, D: DataDictionary
{
    type Source = S;

    fn decode_header(&self, source: &mut S) -> Result<DataElementHeader> {
        // retrieve tag
        let Tag(group, element) = try!(self.basic.decode_tag(source));
        
        let mut buf = [0u8; 4];
        let tag = Tag(group, element);
        try!(source.read_exact(&mut buf));
        let len = LittleEndian::read_u32(&buf);
        let vr = self.dict.get_by_tag(tag).map(|entry| entry.vr()).unwrap_or(VR::UN);
        Ok(DataElementHeader::new(tag, vr, len))
    }

    fn decode_item_header(&self, source: &mut S) -> Result<SequenceItemHeader> {
        let mut buf = [0u8; 4];

        // retrieve tag
        let Tag(group, element) = try!(self.basic.decode_tag(source));

        try!(source.read_exact(&mut buf));
        let len = LittleEndian::read_u32(&buf);
        SequenceItemHeader::new((group, element), len)
    }

    #[inline]
    fn decode_tag(&self, source: &mut Self::Source) -> Result<Tag> {
        self.basic.decode_tag(source)
    }
}

/// A concrete encoder for the transfer syntax ImplicitVRLittleEndian
pub struct ImplicitVRLittleEndianEncoder<W: Write + ?Sized> {
    basic: LittleEndianBasicEncoder<W>,
}

impl<W: Write + ?Sized> fmt::Debug for ImplicitVRLittleEndianEncoder<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ImplicitVRLittleEndianEncoder")
    }
}

impl<W: Write + ?Sized> Default for ImplicitVRLittleEndianEncoder<W> {
    fn default() -> ImplicitVRLittleEndianEncoder<W> {
        ImplicitVRLittleEndianEncoder{
            basic: LittleEndianBasicEncoder::default()
        }
    }
}

impl<W: Write + ?Sized> BasicEncode for ImplicitVRLittleEndianEncoder<W> {
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

impl<W: Write + ?Sized> Encode for ImplicitVRLittleEndianEncoder<W> {

    fn encode_element_header(&self, de: DataElementHeader, to: &mut W) -> Result<usize> {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf[0..], de.tag().group());
        LittleEndian::write_u16(&mut buf[2..], de.tag().element());
        LittleEndian::write_u32(&mut buf[4..], de.len());
        try!(to.write_all(&buf));
        Ok(8)
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
    use super::ImplicitVRLittleEndianDecoder;
    use super::ImplicitVRLittleEndianEncoder;
    use data::{Header, DataElementHeader};
    use data::decode::Decode;
    use data::encode::Encode;
    use dictionary::DataDictionary;
    use dictionary::stub::StubDataDictionary;
    use data::VR;
    use data::Tag;
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

    const DICT: &'static StubDataDictionary = &StubDataDictionary;

    #[test]
    fn implicit_vr_le_works() {
        
        let reader = ImplicitVRLittleEndianDecoder::with_dict(DICT);
        let mut cursor = Cursor::new(RAW.as_ref());
        { // read first element
            let elem = reader.decode_header(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), (0x0002, 0x0002));
            assert_eq!(elem.vr(), VR::UN);
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
            assert_eq!(elem.vr(), VR::UN);
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
            assert_eq!(elem.vr(), VR::UI);
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
    fn encode_implicit_vr_le_works() {
        let mut buf = [0u8; 62];
        {
            let enc = ImplicitVRLittleEndianEncoder::default();
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
            let enc = ImplicitVRLittleEndianEncoder::default();
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
