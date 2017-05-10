//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use std::borrow::{Borrow, BorrowMut};
use std::fmt;
use std::io::Read;
use std::ops::DerefMut;
use std::str;
use std::iter::Iterator;
use error::{Result, Error, TextEncodingError, InvalidValueReadError};
use data::{VR, Tag, Header, DataElementHeader, SequenceItemHeader};
use data::decode::{BasicDecode, Decode};
use data::text::{SpecificCharacterSet, TextCodec, DynamicTextCodec};
use data::value::DicomValue;
use transfer_syntax::{DynamicBasicDecoder, DynamicDecoder, TransferSyntax};
use chrono::naive::date::NaiveDate;
use util::n_times;

/// A trait for DICOM data parsers.
/// This type assumes ownership of a specific source and
/// abstracts the necessary parts of a full DICOM content
/// reading process.
pub trait Parse<S: ?Sized>: BorrowMut<S>
    where S: Read
{
    /// Same as `Decode.decode_header` over the bound source.
    fn decode_header(&mut self) -> Result<DataElementHeader>;

    /// Same as `Decode.decode_header` over the bound source.
    fn decode_item_header(&mut self) -> Result<SequenceItemHeader>;

    /// Eagerly read the following data in the source as a data value.
    /// When reading values in text form, a conversion to a more maleable
    /// type is attempted. Namely, numbers in text form (IS, DS) are converted
    /// to the correspoding binary number types, and date/time instances are
    /// decoded into binary date/time objects of types defined in the `chrono` crate.
    /// To avoid this conversion, see `read_value_preserved`.
    fn read_value(&mut self, header: &DataElementHeader) -> Result<DicomValue>;

    /// Eagerly read the following data in the source as a data value.
    /// Unlike `read_value`, this method will preserve the DICOM value'see
    /// original format: numbers saved as text, as well as dates and times,
    /// are read as strings.
    fn read_value_preserved(&mut self, header: &mut DataElementHeader) -> Result<DicomValue>;
}

/// Alias for a dynamically resolved DICOM parser. Although the data source may be known
/// in compile time, the required decoder may vary according to an object's transfer syntax.
pub type DynamicDicomParser<'s, S> = DicomParser<'s,
                                                 DynamicDecoder<'s>,
                                                 DynamicBasicDecoder<'s>,
                                                 S,
                                                 (Read + 's),
                                                 DynamicTextCodec>;

/// A data structure for parsing DICOM data.
/// This type encapsulates the necessary codecs in order
/// to be as autonomous as possible in the DICOM content reading
/// process.
/// `S` is the generic parameter type for the original source's type,
/// `DS` is the parameter type that the decoder interprets as,
/// whereas `DB` is the parameter type for the basic decoder.
/// `TextCodec` defines the text codec used underneath.
pub struct DicomParser<'s, D, BD, S: ?Sized + 's, DS: ?Sized + 's, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + Read,
          DS: Read,
          TC: TextCodec
{
    source: &'s mut S,
    decoder: D,
    basic: BD,
    text: TC,
}

impl<'s, S: ?Sized + 's, D, BD, DS: ?Sized + 's, TC> fmt::Debug
    for DicomParser<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + Read,
          DS: Read,
          TC: TextCodec
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DicomParser{{source, decoder, text:{:?}}}", &self.text)
    }
}


macro_rules! require_known_length {
    ($header: ident) => (if $header.len() == 0xFFFFFFFF {
        return Err(Error::from(InvalidValueReadError::UnresolvedValueLength))
    })
}

impl<'s, S: ?Sized + 's> DicomParser<'s,
                                     DynamicDecoder<'s>,
                                     DynamicBasicDecoder<'s>,
                                     S,
                                     (Read + 's),
                                     Box<TextCodec>>
    where S: DerefMut<Target = (Read + 's)> + Read
{
    /// Create a new DICOM parser for the given transfer syntax and character set.
    pub fn new_with(mut source: &'s mut S,
                    ts: &TransferSyntax,
                    cs: SpecificCharacterSet)
                    -> Result<DicomParser<'s,
                                          DynamicDecoder<'s>,
                                          DynamicBasicDecoder<'s>,
                                          S,
                                          (Read + 's),
                                          Box<TextCodec>>> {

        let basic = ts.get_basic_decoder();
        let decoder = try!(ts.get_decoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax));
        let text = cs.get_codec().ok_or_else(|| Error::UnsupportedCharacterSet)?;

        Ok(DicomParser {
            source: source,
            basic: basic,
            decoder: decoder,
            text: text,
        })
    }
}

impl<'s, D, BD, S: ?Sized + 's, DS: ?Sized + 's, TC> DicomParser<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + Read,
          DS: Read,
          TC: TextCodec
{
    /// Create a new DICOM parser from its parts.
    pub fn new(source: &'s mut S,
               decoder: D,
               basic: BD,
               text: TC)
               -> DicomParser<'s, D, BD, S, DS, TC> {
        DicomParser {
            source: source,
            basic: basic,
            decoder: decoder,
            text: text,
        }
    }

    // ---------------- private methods ---------------------

    fn read_value_tag(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);

        // tags
        let ntags = {
            header.len() >> 2
        } as usize;
        let parts: Vec<Tag> = try!(n_times(ntags)
            .map(|_| self.decoder.decode_tag(self.source))
            .collect());
        Ok(DicomValue::Tags(parts))
    }

    fn read_value_ob(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        // TODO add support for OB value data length resolution
        require_known_length!(header);

        // sequence of 8-bit integers (or just byte data)
        let mut buf = vec![0u8 ; header.len() as usize];
        try!(self.source.read_exact(&mut buf));
        Ok(DicomValue::U8(buf))
    }

    fn read_value_strs(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of strings
        let mut buf = vec![0u8 ; header.len() as usize];
        try!(self.source.read_exact(&mut buf));
        let parts: Vec<String> = try!(buf[..]
            .split(|v| *v == '\\' as u8)
            .map(|slice| self.text.decode(slice))
            .collect());

        Ok(DicomValue::Strs(parts))
    }

    fn read_value_str(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);

        // a single string
        let mut buf = vec![0u8 ; header.len() as usize];
        try!(self.source.read_exact(&mut buf));
        Ok(DicomValue::Str(try!(self.text.decode(&buf[..]))))
    }

    fn read_value_ss(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        // sequence of 16-bit signed integers
        require_known_length!(header);

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.basic.decode_ss(self.source)));
        }
        Ok(DicomValue::I16(vec))
    }

    fn read_value_fl(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit floats
        let l = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(l);
        for _ in n_times(l) {
            vec.push(try!(self.basic.decode_fl(self.source)));
        }
        Ok(DicomValue::F32(vec))
    }

    fn read_value_od(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 64-bit floats
        let len = header.len() as usize >> 3;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.basic.decode_fd(self.source)));
        }
        Ok(DicomValue::F64(vec))
    }

    fn read_value_ul(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit unsigned integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.basic.decode_ul(self.source)));
        }
        Ok(DicomValue::U32(vec))
    }

    fn read_value_us(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 16-bit unsigned integers

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.basic.decode_us(self.source)));
        }
        Ok(DicomValue::U16(vec))
    }

    fn read_value_sl(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit signed integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(try!(self.basic.decode_sl(self.source)));
        }
        Ok(DicomValue::I32(vec))
    }
}

impl<'s, S: ?Sized + 's, D, BD, DS: ?Sized + 's, TC> Parse<S> for DicomParser<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + Read,
          DS: Read,
          TC: TextCodec
{
    fn decode_header(&mut self) -> Result<DataElementHeader> {
        self.decoder.decode_header(self.source)
    }

    fn decode_item_header(&mut self) -> Result<SequenceItemHeader> {
        self.decoder.decode_item_header(self.source)
    }

    fn read_value(&mut self, header: &DataElementHeader) -> Result<DicomValue> {
        if header.len() == 0 {
            return Ok(DicomValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                return Err(Error::from(InvalidValueReadError::NonPrimitiveType));
            }
            VR::AT => self.read_value_tag(header),
            VR::AE | VR::AS | VR::PN | VR::SH | VR::LO | VR::UI | VR::UC | VR::CS => {
                self.read_value_strs(header)
            }
            VR::UT | VR::ST | VR::UR | VR::LT => self.read_value_str(header),
            VR::UN | VR::OB => self.read_value_ob(header),
            VR::US | VR::OW => self.read_value_us(header),
            VR::SS => self.read_value_ss(header),
            VR::DA => {
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
                    const Z: i32 = b'0' as i32;
                    let year = (y4 as i32 - Z) * 1000 + (y3 as i32 - Z) * 100 +
                               (y2 as i32 - Z) * 10 + y1 as i32 - Z;

                    let month = ((m2 as i32 - Z) * 10 + m1 as i32) as u32;
                    let day = ((d2 as i32 - Z) * 10 + d1 as i32) as u32;

                    let date = try!(NaiveDate::from_ymd_opt(year, month, day)
                        .ok_or_else(|| Error::from(InvalidValueReadError::InvalidFormat)));
                    vec.push(date);
                }
                Ok(DicomValue::Date(vec))
            }
            VR::DT => {
                require_known_length!(header);
                // sequence of datetimes
                unimplemented!()
            }
            VR::TM => {
                require_known_length!(header);
                // sequence of time instances
                // "HHMMSS.FFFFFF"

                unimplemented!()
            }
            VR::DS => {
                require_known_length!(header);
                // sequence of doubles in text form
                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));
                let parts: Vec<f64> = try!(buf[..]
                    .split(|v| *v == '\\' as u8)
                    .map(|slice| {
                        let txt = try!(str::from_utf8(slice)
                            .map_err(|e| Error::from(TextEncodingError::from(e))));
                        txt.parse::<f64>()
                            .map_err(|e| Error::from(InvalidValueReadError::from(e)))
                    })
                    .collect());
                Ok(DicomValue::F64(parts))
            }
            VR::FD | VR::OD => self.read_value_od(header),
            VR::FL | VR::OF => self.read_value_fl(header),
            VR::IS => {
                require_known_length!(header);
                // sequence of signed integers in text form
                let mut buf = vec![0u8 ; header.len() as usize];
                try!(self.source.read_exact(&mut buf));

                let last = if let Some(c) = buf.last() { *c } else { 0u8 };
                if last == ' ' as u8 {
                    buf.pop();
                }
                let parts: Vec<i32> = try!(buf[..]
                    .split(|v| *v == '\\' as u8)
                    .map(|slice| {
                        let txt = try!(str::from_utf8(slice)
                            .map_err(|e| Error::from(TextEncodingError::from(e))));
                        txt.parse::<i32>()
                            .map_err(|e| Error::from(InvalidValueReadError::from(e)))
                    })
                    .collect());
                Ok(DicomValue::I32(parts))
            }
            VR::SL => self.read_value_sl(header),
            VR::OL | VR::UL => self.read_value_ul(header),
        }
    }

    fn read_value_preserved(&mut self, header: &mut DataElementHeader) -> Result<DicomValue> {
        if header.len() == 0 {
            return Ok(DicomValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                return Err(Error::from(InvalidValueReadError::NonPrimitiveType));
            }
            VR::AT => self.read_value_tag(header),
            VR::AE | VR::AS | VR::PN | VR::SH | VR::LO | VR::UI | VR::UC | VR::CS | VR::IS |
            VR::DS | VR::DA | VR::TM | VR::DT => self.read_value_strs(header),
            VR::UT | VR::ST | VR::UR | VR::LT => self.read_value_str(header),
            VR::UN | VR::OB => self.read_value_ob(header),
            VR::US | VR::OW => self.read_value_us(header),
            VR::SS => self.read_value_ss(header),
            VR::FD | VR::OD => self.read_value_od(header),
            VR::FL | VR::OF => self.read_value_fl(header),
            VR::SL => self.read_value_sl(header),
            VR::OL | VR::UL => self.read_value_ul(header),
        }
    }
}

impl<'s, S: ?Sized + 's, D, BD, DS: ?Sized + 's, TC> Borrow<S> for DicomParser<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + Read,
          DS: Read,
          TC: TextCodec
{
    fn borrow(&self) -> &S {
        self.source
    }
}

impl<'s, S: ?Sized + 's, D, BD, DS: ?Sized + 's, TC> BorrowMut<S>
    for DicomParser<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + Read,
          DS: Read,
          TC: TextCodec
{
    fn borrow_mut(&mut self) -> &mut S {
        self.source
    }
}
