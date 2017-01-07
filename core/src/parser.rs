//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use std::ops::DerefMut;
use std::io::{Read, Seek, SeekFrom};
use std::str;
use std::iter::Iterator;
use error::{Result, Error, TextEncodingError, InvalidValueReadError};
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
use util::n_times;

/// A data structure for parsing DICOM data.
/// This type encapsulates the necessary decoders in order
/// to be as autonomous as possible in the DICOM content reading
/// process.
/// `S` is the generic parameter type for the original source's type, whereas
/// `DS` is the parameter type that the decoder interprets as.
pub struct DicomParser<'s, S: Read + ?Sized + 's, DS: Read + ?Sized + 's>
    where S: DerefMut<Target = DS> {
    source: &'s mut S,
    decoder: Box<Decode<Source = DS> + 's>,
    text: Box<TextCodec>,
}

impl<'s, S: Read + ?Sized + 's, DS: Read + ?Sized + 's> fmt::Debug for DicomParser<'s, S, DS>
    where S: DerefMut<Target = DS> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DicomParser{{source, decoder, text:{:?}}}", &self.text)
    }
}

macro_rules! require_known_length {
    ($header: ident) => (if $header.len() == 0xFFFFFFFF {
        return Err(Error::from(InvalidValueReadError::UnresolvedValueLength))
    })
}

impl<'s, S: Read + ?Sized + 's, DS: Read + ?Sized + 's> DicomParser<'s, S, DS>
    where S: DerefMut<Target = DS> {
    /// Create a new DICOM parser.
    pub fn new(source: &'s mut S,
               decoder: Box<Decode<Source = DS> + 's>,
               text: Box<TextCodec>)
               -> DicomParser<'s, S, DS> {
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
    /// When reading values in text form, a conversion to a more maleable
    /// type is attempted. Namely, numbers in text form (IS, DS) are converted
    /// to the correspoding binary number types, and date/time instances are
    /// decoded into binary date/time objects of types defined in the `chrono` crate.
    /// To avoid this conversion, see `read_value_preserved`.
    pub fn read_value(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        if header.len() == 0 {
            return Ok(DicomValue::Empty);
        }

        match header.vr() {
            ValueRepresentation::SQ => {
                // sequence objects... should not work
                return Err(Error::from(InvalidValueReadError::NonPrimitiveType));
            }
            ValueRepresentation::AT => self.read_value_tag(header),
            ValueRepresentation::AE | ValueRepresentation::AS | ValueRepresentation::PN |
            ValueRepresentation::SH | ValueRepresentation::LO | ValueRepresentation::UI |
            ValueRepresentation::UC | ValueRepresentation::CS => self.read_value_strs(header),
            ValueRepresentation::UT | ValueRepresentation::ST | ValueRepresentation::UR |
            ValueRepresentation::LT => self.read_value_str(header),
            ValueRepresentation::UN | ValueRepresentation::OB => self.read_value_ob(header),
            ValueRepresentation::US | ValueRepresentation::OW => self.read_value_us(header),
            ValueRepresentation::SS => self.read_value_ss(header),
            ValueRepresentation::DA => {
                require_known_length!(header);
                // sequence of dates
                let len = header.len() as usize / 8;
                let mut vec = Vec::with_capacity(len);
                for _ in 0..len {
                    // "YYYYMMDD"
                    let mut buf = [0u8; 8];
                    try!(self.source.read_exact(&mut buf));
                    let (y4, y3, y2, y1, m2, m1, d2, d1) =
                        (buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]);
                    const Z: i32 = '0' as i32;
                    let year = (y4 as i32 - Z) * 1000 + (y3 as i32 - Z) * 100 +
                               (y2 as i32 - Z) * 10 + y1 as i32 - Z;

                    let month = ((m2 as i32 - Z) * 10 + m1 as i32) as u32;
                    let day = ((d2 as i32 - Z) * 10 + d1 as i32) as u32;

                    let date = try!(NaiveDate::from_ymd_opt(year, month, day)
                        .ok_or_else(|| Error::from(InvalidValueReadError::InvalidFormat)));
                    vec.push(date);
                }
                Ok(DicomValue::Date(vec.into_boxed_slice()))
            }
            ValueRepresentation::DT => {
                require_known_length!(header);
                // sequence of datetimes
                unimplemented!()
            }
            ValueRepresentation::TM => {
                require_known_length!(header);
                // sequence of time instances
                // "HHMMSS.FFFFFF"

                unimplemented!()
            }
            ValueRepresentation::DS => {
                require_known_length!(header);
                // sequence of doubles in text form
                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));
                let parts: Box<[f64]> = try!(buf[..]
                        .split(|v| *v == '\\' as u8)
                        .map(|slice| {
                            let txt = try!(str::from_utf8(slice)
                                .map_err(|e| Error::from(TextEncodingError::from(e))));
                            txt.parse::<f64>()
                                .map_err(|e| Error::from(InvalidValueReadError::from(e)))
                        })
                        .collect::<Result<Vec<f64>>>())
                    .into_boxed_slice();
                Ok(DicomValue::F64(parts))
            }
            ValueRepresentation::FD | ValueRepresentation::OD => self.read_value_od(header),
            ValueRepresentation::FL | ValueRepresentation::OF => self.read_value_fl(header),
            ValueRepresentation::IS => {
                require_known_length!(header);
                // sequence of signed integers in text form
                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));

                let last = if let Some(c) = buf.last() { *c } else { 0u8 };
                if last == ' ' as u8 {
                    buf.pop();
                }
                let parts: Box<[i32]> = try!(buf[..]
                        .split(|v| *v == '\\' as u8)
                        .map(|slice| {
                            let txt = try!(str::from_utf8(slice)
                                .map_err(|e| Error::from(TextEncodingError::from(e))));
                            txt.parse::<i32>()
                                .map_err(|e| Error::from(InvalidValueReadError::from(e)))
                        })
                        .collect::<Result<Vec<i32>>>())
                    .into_boxed_slice();
                Ok(DicomValue::I32(parts))
            }
            ValueRepresentation::SL => self.read_value_sl(header),
            ValueRepresentation::OL | ValueRepresentation::UL => self.read_value_ul(header),
        }
    }

    /// Eagerly read the following data in the source as a data value.
    /// Unlike `read_value`, this method will preserve the original format of
    /// values in text form, a conversion to a more maleable type is attempted.
    /// Namely, numbers in text form (IS, DS) are converted to the correspoding
    /// binary number types, and date/time instances are decoded into binary
    /// date/time objects of types defined in the `chrono` crate.
    /// To avoid this conversion, see `read_value_preserved`.
    pub fn read_value_preserved(&mut self, header: &mut DataElementHeader) -> Result<(DicomValue)> {
        if header.len() == 0 {
            return Ok(DicomValue::Empty);
        }

        match header.vr() {
            ValueRepresentation::SQ => {
                // sequence objects... should not work
                return Err(Error::from(InvalidValueReadError::NonPrimitiveType));
            }
            ValueRepresentation::AT => self.read_value_tag(header),
            ValueRepresentation::AE | ValueRepresentation::AS | ValueRepresentation::PN |
            ValueRepresentation::SH | ValueRepresentation::LO | ValueRepresentation::UI |
            ValueRepresentation::UC | ValueRepresentation::CS | ValueRepresentation::IS |
            ValueRepresentation::DS | ValueRepresentation::DA | ValueRepresentation::TM |
            ValueRepresentation::DT => self.read_value_strs(header),
            ValueRepresentation::UT | ValueRepresentation::ST | ValueRepresentation::UR |
            ValueRepresentation::LT => self.read_value_str(header),
            ValueRepresentation::UN | ValueRepresentation::OB => self.read_value_ob(header),
            ValueRepresentation::US | ValueRepresentation::OW => self.read_value_us(header),
            ValueRepresentation::SS => self.read_value_ss(header),
            ValueRepresentation::FD | ValueRepresentation::OD => self.read_value_od(header),
            ValueRepresentation::FL | ValueRepresentation::OF => self.read_value_fl(header),
            ValueRepresentation::SL => self.read_value_sl(header),
            ValueRepresentation::OL | ValueRepresentation::UL => self.read_value_ul(header),
        }
    }

    /// Borrow this parser's source mutably.
    pub fn borrow_source_mut(&mut self) -> &mut S {
        self.source
    }

    /// Borrow this parser's source.
    pub fn borrow_source(&self) -> &S {
        self.source
    }

    /// Get the inner source's position in the stream using `seek()`.
    pub fn get_position(&mut self) -> Result<u64> where S: Seek {
        self.source.seek(SeekFrom::Current(0))
            .map_err(Error::from)
    }

    // ---------------- private methods ---------------------

    fn read_value_tag(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);

        // tags
        let ntags = {
            header.len() >> 2
        } as usize;
        let parts: Box<[Tag]> = try!(n_times(ntags)
                .map(|_| self.decoder.decode_tag(self.source))
                .collect::<Result<Vec<_>>>())
            .into_boxed_slice();
        Ok(DicomValue::Tags(parts))
    }

    fn read_value_ob(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        // TODO add support for OB value data length resolution
        require_known_length!(header);

        // sequence of 8-bit integers (or just byte data)
        let mut buf = vec![0u8 ; header.len() as usize];
        try!(self.source.read_exact(&mut buf));
        Ok(DicomValue::U8(buf.into_boxed_slice()))
    }

    fn read_value_strs(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of strings
        let mut buf = vec![0u8 ; header.len() as usize];
        try!(self.source.read_exact(&mut buf));
        let parts: Box<[String]> = try!(buf[..]
                .split(|v| *v == '\\' as u8)
                .map(|slice| self.text.as_ref().decode(slice))
                .collect::<Result<Vec<String>>>())
            .into_boxed_slice();

        Ok(DicomValue::Strs(parts))
    }

    fn read_value_str(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);

        // a single string
        let mut buf = vec![0u8 ; header.len() as usize];
        try!(self.source.read_exact(&mut buf));
        Ok(DicomValue::Str(try!(self.text.as_ref().decode(&buf[..]))))
    }

    fn read_value_ss(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        // sequence of 16-bit signed integers
        require_known_length!(header);

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.decoder.as_ref().decode_ss(self.source)));
        }
        Ok(DicomValue::I16(vec.into_boxed_slice()))
    }

    fn read_value_fl(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit floats
        let l = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(l);
        for _ in n_times(l) {
            vec.push(try!(self.decoder.as_ref().decode_fl(self.source)));
        }
        Ok(DicomValue::F32(vec.into_boxed_slice()))
    }

    fn read_value_od(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 64-bit floats
        let len = header.len() as usize >> 3;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.decoder.as_ref().decode_fd(self.source)));
        }
        Ok(DicomValue::F64(vec.into_boxed_slice()))
    }

    fn read_value_ul(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit unsigned integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.decoder.as_ref().decode_ul(self.source)));
        }
        Ok(DicomValue::U32(vec.into_boxed_slice()))
    }

    fn read_value_us(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 16-bit unsigned integers

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.decoder.as_ref().decode_us(self.source)));
        }
        Ok(DicomValue::U16(vec.into_boxed_slice()))
    }

    fn read_value_sl(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit signed integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.decoder.as_ref().decode_sl(self.source)));
        }
        Ok(DicomValue::I32(vec.into_boxed_slice()))
    }
}

impl<'s, S: Read + ?Sized + 's, DS: Read + ?Sized + 's> Borrow<S> for DicomParser<'s, S, DS>
    where S: DerefMut<Target = DS> {
    fn borrow(&self) -> &S {
        self.source
    }
}

impl<'s, S: Read + ?Sized + 's, DS: Read + ?Sized + 's> BorrowMut<S> for DicomParser<'s, S, DS>
    where S: DerefMut<Target = DS> {
    fn borrow_mut(&mut self) -> &mut S {
        self.source
    }
}
