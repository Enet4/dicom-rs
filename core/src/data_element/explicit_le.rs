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

#[cfg(test)]
mod tests {
    use super::super::decode::Decode;
    use super::ExplicitVRLittleEndianDecoder;
    use data_element::DataElement;
    use attribute::ValueRepresentation;
    use std::io::{Read, Cursor, Seek, SeekFrom};

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
        0x02, 0x00, 0x02, 0x00, 0x55, 0x49, 0x1a, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e, 0x31, 0x2e,
        0x31, 0x00,

        0x02, 0x00, 0x10, 0x00, 0x55, 0x49, 0x14, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e, 0x31, 0x00
    ];

    #[test]
    fn explicit_vr_le_works() {
        
        let reader = ExplicitVRLittleEndianDecoder::default();
        let mut cursor = Cursor::new(RAW.as_ref());
        { // read first element
            let elem = reader.decode(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), (2, 2));
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
            let elem = reader.decode(&mut cursor).expect("should find an element");
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
        write!(f, "ImplicitVRLittleEndianDecoder{{phantom}}")
    }
}

impl<S: Read + ?Sized> Decode for ExplicitVRLittleEndianDecoder<S> {
    type Source = S;

    fn decode(&self, source: &mut S) -> Result<DataElementHeader> {
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

    fn decode_item(&self, source: &mut S) -> Result<SequenceItemHeader> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf));
        // retrieve tag
        let group = LittleEndian::read_u16(&buf[0..2]);
        let element = LittleEndian::read_u16(&buf[2..4]);

        try!(source.read_exact(&mut buf));
        let len = LittleEndian::read_u32(&buf);

        SequenceItemHeader::new((group, element), len)
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
        write!(f, "ImplicitVRLittleEndianEncoder{{phantom}}")
    }
}

impl<W: Write + ?Sized> Encode for ExplicitVRLittleEndianEncoder<W> {
    type Writer = W;

    fn encode_element_header(&self, de: DataElementHeader, to: &mut W) -> Result<()> {
        unimplemented!();
    }

    /// Encode and write a DICOM sequence item header to the given destination.
    fn encode_item(&self, len: u32, to: &mut W) -> Result<()> {
        unimplemented!();
    }

    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter(&self, to: &mut W) -> Result<()> {
        unimplemented!();
    }

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter(&self, to: &mut W) -> Result<()> {
        unimplemented!();
    }

}