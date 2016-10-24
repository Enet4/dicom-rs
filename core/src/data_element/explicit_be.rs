//! Explicit VR Big Endian syntax transfer implementation.

use std::io::Read;
use std::fmt;
use attribute::ValueRepresentation;
use byteorder::{ByteOrder, BigEndian};
use error::Result;
use super::decode::Decode;
use std::marker::PhantomData;
use data_element::{DataElementHeader, SequenceItemHeader};

#[cfg(test)]
mod tests {
    use super::super::decode::Decode;
    use super::ExplicitVRBigEndianDecoder;
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
        0x00, 0x02, 0x00, 0x02, 0x55, 0x49, 0x00, 0x1a, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e, 0x31, 0x2e,
        0x31, 0x00,

        0x00, 0x02, 0x00, 0x10, 0x55, 0x49, 0x00, 0x14, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
        0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e, 0x31, 0x00
    ];

    #[test]
    fn explicit_vr_be_works() {
        
        let reader = ExplicitVRBigEndianDecoder::default();
        let mut cursor = Cursor::new(RAW.as_ref());
        { // read first element
            let elem = reader.decode(&mut cursor).expect("should find an element");
            assert_eq!(elem.tag(), (2, 2));
            assert_eq!(elem.vr(), ValueRepresentation::UI);
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

/// A data element decoder for the Explicit VR Big Endian transfer syntax.
pub struct ExplicitVRBigEndianDecoder<S: Read + ?Sized> {
    phantom: PhantomData<S>,
}

impl<S: Read + ?Sized> Default for ExplicitVRBigEndianDecoder<S> {
    fn default() -> ExplicitVRBigEndianDecoder<S> {
        ExplicitVRBigEndianDecoder{ phantom: PhantomData::default() }
    }
}

impl<S: Read + ?Sized> fmt::Debug for ExplicitVRBigEndianDecoder<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ImplicitVRLittleEndianDecoder{{phantom}}")
    }
}

impl<'s, S: Read + ?Sized + 's> Decode for ExplicitVRBigEndianDecoder<S> {
    type Source = S;
    
    fn decode(&self, source: &mut Self::Source) -> Result<DataElementHeader> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf));
        // retrieve tag
        let group = BigEndian::read_u16(&buf[0..2]);
        let element = BigEndian::read_u16(&buf[2..4]);

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
                BigEndian::read_u32(&buf)
            },
            _ => {
                // read 2 bytes for the data length
                try!(source.read_exact(&mut buf[0..2]));
                BigEndian::read_u16(&buf[0..2]) as u32
            }
        };

        Ok(DataElementHeader{ tag: (group, element), vr: vr, len: len })
    }

    fn decode_item(&self, source: &mut Self::Source) -> Result<SequenceItemHeader> {
        let mut buf = [0u8; 4];
        try!(source.read_exact(&mut buf));
        // retrieve tag
        let group = BigEndian::read_u16(&buf[0..2]);
        let element = BigEndian::read_u16(&buf[2..4]);

        try!(source.read_exact(&mut buf));
        let len = BigEndian::read_u32(&buf);

        SequenceItemHeader::new((group, element), len)
    }
}
