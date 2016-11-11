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
use attribute::tag::Tag;
use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::fmt;
use chrono::naive::date::NaiveDate;

/// A data structure for parsing DICOM data.
/// This type encapsulates the necessary decoders in order
/// to be as autonomous as possible in the DICOM content reading
/// process. 
pub struct DicomParser<'s, S: Read + ?Sized + 's> {
    source: &'s mut S,
    decoder: Box<Decode<Source = S> + 's>,
    text: Box<TextCodec>,
}

impl<'s, S: Read + ?Sized + 's> fmt::Debug for DicomParser<'s, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DicomParser{{source, decoder, text:{:?}}}", &self.text)
    }
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
                let parts: Box<[Tag]> = try!(repeat(())
                        .take(ntags)
                        .map(|_| self.decoder.decode_tag(self.source))
                        .collect::<Result<Vec<_>>>())
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

                let mut vec = Vec::with_capacity(header.len() as usize / 2);
                for _ in 0..header.len() / 2 {
                    vec.push(try!(self.decoder.as_ref().decode_us(self.source)));
                }
                DicomValue::U16(vec.into_boxed_slice())
            }
            ValueRepresentation::SS => {
                // sequence of 16-bit signed integers
                require_known_length!(header);

                let len = header.len() as usize / 2;
                let mut vec = Vec::with_capacity(len);
                for _ in 0..len{
                    vec.push(try!(self.decoder.as_ref().decode_ss(self.source)));
                }
                DicomValue::I16(vec.into_boxed_slice())
            }
            ValueRepresentation::DA => {
                require_known_length!(header);
                // sequence of dates
                let len = header.len() as usize / 8;
                let mut vec = Vec::with_capacity(len);
                for _ in 0..len {
                    // YYYYMMDD
                    let mut buf = [0u8; 8];
                    try!(self.source.read_exact(&mut buf));
                    let (y4, y3, y2, y1, m2, m1, d2, d1) =
                    (buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]);
                    const Z: i32 = '0' as i32; 
                    let year = (y4 as i32 - Z) * 1000
                            + (y3 as i32 - Z) * 100
                            + (y2 as i32 - Z) * 10
                            + y1 as i32 - Z;

                    const Z2: u32 = '0' as u32;
                    let month = m2 as u32 - Z2 * 10 + m1 as u32;
                    let day = d2 as u32 - Z2 * 10 + d1 as u32;
                    
                    let date = try!(NaiveDate::from_ymd_opt(year, month, day)
                            .ok_or_else(||Error::from(InvalidValueReadError::InvalidFormat)));
                    vec.push(date);
                }
                DicomValue::Date(vec.into_boxed_slice())
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
                let l = header.len() as usize / 4;
                let mut vec = Vec::with_capacity(l);
                for _ in 0..l {
                    vec.push(try!(self.decoder.as_ref().decode_fl(self.source)));
                }
                DicomValue::F32(vec.into_boxed_slice())
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
