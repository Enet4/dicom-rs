//! Implicit VR Big Endian syntax transfer implementation

use crate::decode::basic::LittleEndianBasicDecoder;
use crate::decode::{BasicDecode, Decode};
use crate::encode::basic::LittleEndianBasicEncoder;
use crate::encode::{BasicEncode, Encode, EncoderFor};
use crate::error::Result;
use byteordered::byteorder::{ByteOrder, LittleEndian};
use byteordered::Endianness;
use dicom_core::dictionary::{DataDictionary, DictionaryEntry};
use dicom_core::header::{DataElementHeader, Header, Length, SequenceItemHeader};
use dicom_core::{PrimitiveValue, Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use std::fmt;
use std::io::{Read, Write};
use std::marker::PhantomData;

/// An ImplicitVRLittleEndianDecoder which uses the standard data dictionary.
pub type StandardImplicitVRLittleEndianDecoder<S> =
    ImplicitVRLittleEndianDecoder<S, StandardDataDictionary>;

/// A data element decoder for the Explicit VR Little Endian transfer syntax.
/// This type contains a reference to an attribute dictionary for resolving
/// value representations.
pub struct ImplicitVRLittleEndianDecoder<S: ?Sized, D> {
    dict: D,
    basic: LittleEndianBasicDecoder,
    phantom: PhantomData<S>,
}

impl<S: ?Sized, D> fmt::Debug for ImplicitVRLittleEndianDecoder<S, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ImplicitVRLittleEndianDecoder")
            .field("dict", &"«omitted»")
            .field("basic", &self.basic)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<S: ?Sized> ImplicitVRLittleEndianDecoder<S, StandardDataDictionary> {
    /// Retrieve this decoder using the standard data dictionary.
    pub fn with_std_dict() -> Self {
        ImplicitVRLittleEndianDecoder {
            dict: StandardDataDictionary,
            basic: LittleEndianBasicDecoder,
            phantom: PhantomData,
        }
    }
}

impl<S: ?Sized> Default for ImplicitVRLittleEndianDecoder<S, StandardDataDictionary> {
    fn default() -> Self {
        ImplicitVRLittleEndianDecoder::with_std_dict()
    }
}

impl<S: ?Sized, D> ImplicitVRLittleEndianDecoder<S, D>
where
    S: Read,
    D: DataDictionary,
{
    /// Retrieve this decoder using a custom data dictionary.
    pub fn with_dict(dictionary: D) -> Self {
        ImplicitVRLittleEndianDecoder {
            dict: dictionary,
            basic: LittleEndianBasicDecoder,
            phantom: PhantomData,
        }
    }
}

impl<'d, S: ?Sized, D> Decode for ImplicitVRLittleEndianDecoder<S, D>
where
    S: Read,
    D: DataDictionary,
{
    type Source = S;

    fn decode_header(&self, mut source: &mut S) -> Result<(DataElementHeader, usize)> {
        // retrieve tag
        let tag = self.basic.decode_tag(&mut source)?;

        let mut buf = [0u8; 4];
        source.read_exact(&mut buf)?;
        let len = LittleEndian::read_u32(&buf);

        // VR resolution is done with the help of the data dictionary.
        // In Implicit VR Little Endian, the VR of OB may not be used for Pixel
        // Data (7FE0,0010). This edge case is addressed manually.
        let vr = if tag == Tag(0x7FE0, 0x0010) {
            VR::OW
        } else {
            self.dict
                .by_tag(tag)
                .map(|entry| entry.vr())
                .unwrap_or(VR::UN)
        };
        Ok((DataElementHeader::new(tag, vr, Length(len)), 8))
    }

    fn decode_item_header(&self, mut source: &mut S) -> Result<SequenceItemHeader> {
        let mut buf = [0u8; 4];

        // retrieve tag
        let tag = self.basic.decode_tag(&mut source)?;

        source.read_exact(&mut buf)?;
        let len = LittleEndian::read_u32(&buf);
        let header = SequenceItemHeader::new(tag, Length(len))?;
        Ok(header)
    }

    #[inline]
    fn decode_tag(&self, source: &mut S) -> Result<Tag> {
        self.basic.decode_tag(source)
    }
}

/// A concrete encoder for the transfer syntax ImplicitVRLittleEndian
#[derive(Debug, Default, Clone)]
pub struct ImplicitVRLittleEndianEncoder {
    basic: LittleEndianBasicEncoder,
}

pub type ImplicitVRLittleEndianEncoderTo<W> = EncoderFor<ImplicitVRLittleEndianEncoder, W>;

impl BasicEncode for ImplicitVRLittleEndianEncoder {
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

impl Encode for ImplicitVRLittleEndianEncoder {
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
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf[0..], de.tag().group());
        LittleEndian::write_u16(&mut buf[2..], de.tag().element());
        LittleEndian::write_u32(&mut buf[4..], de.len().0);
        to.write_all(&buf)?;
        Ok(8)
    }

    fn encode_item_header<W>(&self, mut to: W, len: u32) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf, 0xE000);
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
        LittleEndian::write_u16(&mut buf, 0xE00D);
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_sequence_delimiter<W>(&self, mut to: W) -> Result<()>
    where
        W: Write,
    {
        let mut buf = [0u8; 8];
        LittleEndian::write_u16(&mut buf, 0xFFFE);
        LittleEndian::write_u16(&mut buf, 0xE0DD);
        to.write_all(&buf)?;
        Ok(())
    }

    fn encode_primitive<W>(&self, to: W, value: &PrimitiveValue) -> Result<()>
    where
        W: Write,
    {
        self.basic.encode_primitive(to, value)
    }
}

#[cfg(test)]
mod tests {
    use super::ImplicitVRLittleEndianDecoder;
    use super::ImplicitVRLittleEndianEncoder;
    use crate::decode::Decode;
    use crate::encode::Encode;
    use dicom_core::dictionary::stub::StubDataDictionary;
    use dicom_core::header::{DataElementHeader, Header, Length, Tag, VR};
    use std::io::{Cursor, Read, Seek, SeekFrom, Write};

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

    const DICT: &'static StubDataDictionary = &StubDataDictionary;

    #[test]
    fn implicit_vr_le() {
        let reader = ImplicitVRLittleEndianDecoder::with_dict(DICT);
        let mut cursor = Cursor::new(RAW.as_ref());
        {
            // read first element
            let (elem, bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), (0x0002, 0x0002));
            assert_eq!(elem.vr(), VR::UN);
            assert_eq!(elem.len(), Length(26));
            assert_eq!(bytes_read, 8);
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
            let (elem, _bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), (0x0002, 0x0010));
            assert_eq!(elem.vr(), VR::UN);
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
    fn implicit_vr_le_with_standard_dictionary() {
        let reader = ImplicitVRLittleEndianDecoder::with_std_dict();
        let mut cursor = Cursor::new(RAW.as_ref());
        {
            // read first element
            let (elem, _bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), (2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.len(), Length(26));
            // cursor should be @ #8
            assert_eq!(cursor.seek(SeekFrom::Current(0)).unwrap(), 8);
            // don't read any data, just skip
            // cursor should be @ #34 after skipping
            assert_eq!(
                cursor.seek(SeekFrom::Current(elem.len().0 as i64)).unwrap(),
                34
            );
        }
        {
            // read second element
            let (elem, _bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), (2, 16));
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
}
