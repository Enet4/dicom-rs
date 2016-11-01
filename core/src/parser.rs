//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use std::io::Read;
use std::iter::{repeat, Iterator};
use error::{Result, Error, InvalidValueReadError};
use data_element::{Header, DataElementHeader, SequenceItemHeader};
use data_element::decode::Decode;
use data_element::text::TextCodec;
use attribute::ValueRepresentation;
use attribute::value::DicomValue;
use std::borrow::Borrow;
use std::borrow::BorrowMut;

/// A data structure for parsing DICOM data.
/// This type encapsulates the necessary decoders in order
/// to be as autonomous as possible in the DICOM content reading
/// process. 
#[derive(Debug)]
pub struct DicomParser<'s, S: Read + ?Sized + 's> {
    source: &'s mut S,
    decoder: Box<Decode<Source = S> + 's>,
    text: Box<TextCodec>,
}

macro_rules! require_known_length {
    ($header: ident) => (if $header.len() == 0xFFFFFFFF {
        return Err(Error::from(InvalidValueReadError::UnresolvedValueLength))
    })
}

impl<'s, S: Read + ?Sized + 's> DicomParser<'s, S> {
    /// Create a new DICOM parser
    pub fn new(source: &'s mut S, decoder: Box<Decode<Source=S> + 's>, text: Box<TextCodec>) -> DicomParser<'s, S> {
        DicomParser {
            source: source,
            decoder: decoder,
            text: text,
        }
    }

    /// Same as `Decode.decode_header` over the internal source.
    pub fn decode_header(&mut self) -> Result<DataElementHeader> {
        self.decoder.as_ref().decode_header(self.source)
    }

    /// Same as `Decode.decode_item_header` over the internal source.
    pub fn decode_item_header(&mut self) -> Result<SequenceItemHeader> {
        self.decoder.as_ref().decode_item_header(self.source)
    }

    /// Eagerly read the following data in the source as a data value.
    pub fn read_value<'a>(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        if header.len() == 0 {
            return Ok(DicomValue::Empty);
        }
        let value = match header.vr() {
            ValueRepresentation::SQ => {
                // sequence objects... should not work
                return Err(Error::from(InvalidValueReadError::NonPrimitiveType));
            }
            ValueRepresentation::AT => {
                require_known_length!(header);

                // tags
                // try!(self.source.read_exact(&mut buf));
                let ntags = {
                    header.len() >> 2
                } as usize;
                let parts: Box<[(u16, u16)]> = try!(repeat(())
                        .take(ntags)
                        .map(|_| self.decoder.decode_tag(self.source))
                        .collect::<Result<Vec<(u16, u16)>>>())
                    .into_boxed_slice();
                DicomValue::Tags(parts)
            }
            ValueRepresentation::AE | ValueRepresentation::AS | ValueRepresentation::PN |
            ValueRepresentation::SH | ValueRepresentation::LO | ValueRepresentation::UI |
            ValueRepresentation::UC | ValueRepresentation::CS => {
                require_known_length!(header);
                // sequence of strings
                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));
                let parts: Box<[String]> = try!(buf[..]
                        .split(|v| *v == '\\' as u8)
                        .map(|slice| self.text.as_ref().decode(slice))
                        .collect::<Result<Vec<String>>>())
                    .into_boxed_slice();

                DicomValue::Strs(parts)
            }
            ValueRepresentation::UT | ValueRepresentation::ST | ValueRepresentation::UR |
            ValueRepresentation::LT => {
                require_known_length!(header);

                // a single string
                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));
                DicomValue::Str(try!(self.text.as_ref().decode(&buf[..])))
            }
            ValueRepresentation::UN | ValueRepresentation::OB => {
                // sequence of 8-bit integers (or just byte data)
                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));
                DicomValue::U8(buf.into_boxed_slice())
            }
            ValueRepresentation::US | ValueRepresentation::OW => {
                // TODO add support for OW value data length resolution
                require_known_length!(header);

                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));
                // sequence of 16-bit unsigned integers
                unimplemented!()
            }
            ValueRepresentation::SS => {
                require_known_length!(header);
                // sequence of 16-bit signed integers
                unimplemented!()
            }
            ValueRepresentation::DA => {
                require_known_length!(header);
                // sequence of dates
                unimplemented!()
            }
            ValueRepresentation::DT => {
                require_known_length!(header);
                // sequence of datetimes
                unimplemented!()
            }
            ValueRepresentation::TM => {
                require_known_length!(header);
                // sequence of time instances
                unimplemented!()
            }
            ValueRepresentation::DS | ValueRepresentation::FD | ValueRepresentation::OD => {
                require_known_length!(header);

                // sequence of 64-bit floats
                unimplemented!()
            }
            ValueRepresentation::FL | ValueRepresentation::OF => {
                require_known_length!(header);

                // sequence of 32-bit floats
                unimplemented!()
            }
            ValueRepresentation::IS | ValueRepresentation::SL => {
                require_known_length!(header);

                // sequence of 32-bit signed integers
                unimplemented!()
            }
            ValueRepresentation::OL | ValueRepresentation::UL => {
                require_known_length!(header);
                // sequence of 32-bit unsigned integers
                unimplemented!()
            }
        };

        Ok(value)
    }

    fn read_value_preserved(&mut self, elem: &mut DataElementHeader) -> Result<()> {
        unimplemented!(); // TODO
    }

    /// Borrow this parser's source.
    pub fn borrow_source(&mut self) -> &mut S {
        self.source
    }
}

impl<'s, S: Read + ?Sized + 's> Borrow<S> for DicomParser<'s, S> {
    fn borrow(&self) -> &S {
        self.source
    }
}

impl<'s, S: Read + ?Sized + 's> BorrowMut<S> for DicomParser<'s, S> {
    fn borrow_mut(&mut self) -> &mut S {
        self.source
    }
}
