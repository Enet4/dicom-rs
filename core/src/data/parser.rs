//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use std::fmt;
use std::fmt::Debug;
use std::io::Read;
use std::marker::PhantomData;
use std::iter::Iterator;
use error::{Error, InvalidValueReadError, Result};
use data::{DataElementHeader, Header, SequenceItemHeader, Tag, VR};
use data::decode::{BasicDecode, Decode};
use data::decode::basic::{BasicDecoder, LittleEndianBasicDecoder};
use data::text::{DynamicTextCodec, SpecificCharacterSet, TextCodec};
use data::value::DicomValue;
use transfer_syntax::TransferSyntax;
use transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
use data::text::DefaultCharacterSetCodec;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use util::n_times;

const Z: i32 = b'0' as i32;

/// A trait for DICOM data parsers, which abstracts the necessary parts
/// of a full DICOM content reading process.
pub trait Parse<S: ?Sized>
where
    S: Read,
{
    /// Same as `Decode.decode_header` over the bound source.
    fn decode_header(&self, from: &mut S) -> Result<DataElementHeader>;

    /// Same as `Decode.decode_header` over the bound source.
    fn decode_item_header(&self, from: &mut S) -> Result<SequenceItemHeader>;

    /// Eagerly read the following data in the source as a data value.
    /// When reading values in text form, a conversion to a more maleable
    /// type is attempted. Namely, numbers in text form (IS, DS) are converted
    /// to the correspoding binary number types, and date/time instances are
    /// decoded into binary date/time objects of types defined in the `chrono` crate.
    /// To avoid this conversion, see `read_value_preserved`.
    fn read_value(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue>;

    /// Eagerly read the following data in the source as a data value.
    /// Unlike `read_value`, this method will preserve the DICOM value's
    /// original format: numbers saved as text, as well as dates and times,
    /// are read as strings.
    fn read_value_preserved(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue>;

    /// Define the specific character set of subsequent text elements.
    fn set_character_set(&mut self, charset: SpecificCharacterSet) -> Result<()>;
}

/// Alias for a dynamically resolved DICOM parser. Although the data source may be known
/// in compile time, the required decoder may vary according to an object's transfer syntax.
pub type DynamicDicomParser =
    DicomParser<Box<Decode<Source = Read>>, BasicDecoder, Read, DynamicTextCodec>;

/// A data structure for parsing DICOM data.
/// This type encapsulates the necessary codecs in order
/// to be as autonomous as possible in the DICOM content reading
/// process.
/// `S` is the generic parameter type for the original source's type,
/// `DS` is the parameter type that the decoder interprets as,
/// whereas `DB` is the parameter type for the basic decoder.
/// `TextCodec` defines the text codec used underneath.
pub struct DicomParser<D, BD, S: ?Sized, TC> {
    phantom: PhantomData<S>,
    decoder: D,
    basic: BD,
    text: TC,
}

impl<S: ?Sized, D, BD, TC> Debug for DicomParser<D, BD, S, TC>
where
    D: Debug,
    BD: Debug,
    TC: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DicomParser")
            .field("decoder", &self.decoder)
            .field("basic", &self.basic)
            .field("text", &self.text)
            .finish()
    }
}

macro_rules! require_known_length {
    ($header: ident) => (if $header.len() == 0xFFFFFFFF {
        return Err(Error::from(InvalidValueReadError::UnresolvedValueLength))
    })
}

impl DynamicDicomParser {
    /// Create a new DICOM parser for the given transfer syntax and character set.
    pub fn new_with(ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self> {
        let basic = ts.get_basic_decoder();
        let decoder = ts.get_decoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax)?;
        let text = cs.get_codec()
            .ok_or_else(|| Error::UnsupportedCharacterSet)?;

        Ok(DicomParser {
            phantom: PhantomData,
            basic: basic,
            decoder: decoder,
            text: text,
        })
    }
}

pub type FileHeaderParser<S> = DicomParser<
    ExplicitVRLittleEndianDecoder<S>,
    LittleEndianBasicDecoder,
    S,
    DefaultCharacterSetCodec,
>;

impl<S: ?Sized> FileHeaderParser<S>
where
    S: Read,
{
    /// Create a new DICOM parser from its parts.
    pub fn file_header_parser() -> Self {
        DicomParser {
            phantom: PhantomData,
            basic: LittleEndianBasicDecoder::default(),
            decoder: ExplicitVRLittleEndianDecoder::default(),
            text: DefaultCharacterSetCodec,
        }
    }
}

impl<D, BD, S: ?Sized, TC> DicomParser<D, BD, S, TC>
where
    D: Decode<Source = S>,
    BD: BasicDecode,
    S: Read,
    TC: TextCodec,
{
    /// Create a new DICOM parser from its parts.
    pub fn new(decoder: D, basic: BD, text: TC) -> DicomParser<D, BD, S, TC> {
        DicomParser {
            phantom: PhantomData,
            basic: basic,
            decoder: decoder,
            text: text,
        }
    }

    // ---------------- private methods ---------------------

    fn read_value_tag(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);

        // tags
        let ntags = { header.len() >> 2 } as usize;
        let parts: Result<Vec<Tag>> = n_times(ntags)
            .map(|_| {
                let g = self.basic.decode_us(&mut *from)?;
                let e = self.basic.decode_us(&mut *from)?;
                Ok(Tag(g, e))
            })
            .collect();
        Ok(DicomValue::Tags(parts?))
    }

    fn read_value_ob(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        // TODO add support for OB value data length resolution
        require_known_length!(header);

        // sequence of 8-bit integers (or just byte data)
        let mut buf = vec![0u8; header.len() as usize];
        from.read_exact(&mut buf)?;
        Ok(DicomValue::U8(buf))
    }

    fn read_value_strs(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of strings
        let mut buf = vec![0u8; header.len() as usize];
        from.read_exact(&mut buf)?;

        let parts: Result<Vec<_>> = match header.vr() {
            VR::AE | VR::CS | VR::AS => buf[..]
                    .split(|v| *v == '\\' as u8)
                    .map(|slice| DefaultCharacterSetCodec.decode(slice))
                    .collect(),
            _ => buf[..]
                    .split(|v| *v == '\\' as u8)
                    .map(|slice| self.text.decode(slice))
                    .collect()
        };

        Ok(DicomValue::Strs(parts?))
    }

    fn read_value_str(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);

        // a single string
        let mut buf = vec![0u8; header.len() as usize];
        try!(from.read_exact(&mut buf));
        Ok(DicomValue::Str(self.text.decode(&buf[..])?))
    }

    fn read_value_ss(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        // sequence of 16-bit signed integers
        require_known_length!(header);

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_ss(&mut *from)?);
        }
        Ok(DicomValue::I16(vec))
    }

    fn read_value_fl(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit floats
        let l = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(l);
        for _ in n_times(l) {
            vec.push(self.basic.decode_fl(&mut *from)?);
        }
        Ok(DicomValue::F32(vec))
    }

    fn read_value_od(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 64-bit floats
        let len = header.len() as usize >> 3;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_fd(&mut *from)?);
        }
        Ok(DicomValue::F64(vec))
    }

    fn read_value_ul(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit unsigned integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_ul(&mut *from)?);
        }
        Ok(DicomValue::U32(vec))
    }

    fn read_value_us(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 16-bit unsigned integers

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_us(&mut *from)?);
        }
        Ok(DicomValue::U16(vec))
    }

    fn read_value_sl(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        require_known_length!(header);
        // sequence of 32-bit signed integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_sl(&mut *from)?);
        }
        Ok(DicomValue::I32(vec))
    }
}

impl<S: ?Sized, D, BD> Parse<S> for DicomParser<D, BD, S, Box<TextCodec>>
where
    D: Decode<Source = S>,
    BD: BasicDecode,
    S: Read,
{
    fn decode_header(&self, from: &mut S) -> Result<DataElementHeader> {
        self.decoder.decode_header(from)
    }

    fn decode_item_header(&self, from: &mut S) -> Result<SequenceItemHeader> {
        self.decoder.decode_item_header(from)
    }

    fn read_value(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        if header.len() == 0 {
            return Ok(DicomValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                return Err(Error::from(InvalidValueReadError::NonPrimitiveType));
            }
            VR::AT => self.read_value_tag(from, header),
            VR::AE | VR::AS | VR::PN | VR::SH | VR::LO | VR::UI | VR::UC | VR::CS => {
                self.read_value_strs(from, header)
            }
            VR::UT | VR::ST | VR::UR | VR::LT => self.read_value_str(from, header),
            VR::UN | VR::OB => self.read_value_ob(from, header),
            VR::US | VR::OW => self.read_value_us(from, header),
            VR::SS => self.read_value_ss(from, header),
            VR::DA => {
                require_known_length!(header);
                // sequence of dates

                // maybe one day I should find a way to get rid of this dynamic allocation
                let mut buf = vec![0u8; header.len() as usize];
                from.read_exact(&mut buf)?;
                let vec: Result<Vec<_>> = buf.split(|b| *b == b'\\')
                    .map(|part| Ok(parse_date(part)?.0))
                    .collect();
                Ok(DicomValue::Date(vec?))
            }
            VR::DT => {
                require_known_length!(header);
                // sequence of datetimes

                // dynamic allocation
                let mut buf = vec![0u8; header.len() as usize];
                from.read_exact(&mut buf)?;
                let vec: Result<Vec<_>> = buf.split(|b| *b == b'\\')
                    .map(|part| Ok(parse_datetime(part)?))
                    .collect();

                Ok(DicomValue::DateTime(vec?))
            }
            VR::TM => {
                require_known_length!(header);
                // sequence of time instances

                // dynamic allocation
                let mut buf = vec![0u8; header.len() as usize];
                from.read_exact(&mut buf)?;
                let vec: Result<Vec<_>> = buf.split(|b| *b == b'\\')
                    .map(|part| Ok(parse_time(part)?.0))
                    .collect();
                Ok(DicomValue::Time(vec?))
            }
            VR::DS => {
                require_known_length!(header);
                // sequence of doubles in text form
                let mut buf = vec![0u8; header.len() as usize];
                from.read_exact(&mut buf)?;
                let parts: Result<Vec<f64>> = buf[..]
                    .split(|v| *v == '\\' as u8)
                    .map(|slice| {
                        let codec = SpecificCharacterSet::Default.get_codec().unwrap();
                        let txt = codec.decode(slice)?;
                        txt.parse::<f64>()
                            .map_err(|e| Error::from(InvalidValueReadError::from(e)))
                    })
                    .collect();
                Ok(DicomValue::F64(parts?))
            }
            VR::FD | VR::OD => self.read_value_od(from, header),
            VR::FL | VR::OF => self.read_value_fl(from, header),
            VR::IS => {
                require_known_length!(header);
                // sequence of signed integers in text form
                let mut buf = vec![0u8; header.len() as usize];
                from.read_exact(&mut buf)?;

                let last = if let Some(c) = buf.last() { *c } else { 0u8 };
                if last == ' ' as u8 {
                    buf.pop();
                }
                let parts: Result<Vec<i32>> =
                    buf[..]
                        .split(|v| *v == '\\' as u8)
                        .map(|slice| {
                            let codec = SpecificCharacterSet::Default.get_codec().unwrap();
                            let txt = codec.decode(slice)?;
                            txt.parse::<i32>()
                                .map_err(|e| Error::from(InvalidValueReadError::from(e)))
                        })
                        .collect();
                Ok(DicomValue::I32(parts?))
            }
            VR::SL => self.read_value_sl(from, header),
            VR::OL | VR::UL => self.read_value_ul(from, header),
        }
    }

    fn read_value_preserved(&self, from: &mut S, header: &DataElementHeader) -> Result<DicomValue> {
        if header.len() == 0 {
            return Ok(DicomValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                return Err(Error::from(InvalidValueReadError::NonPrimitiveType));
            }
            VR::AT => self.read_value_tag(from, header),
            VR::AE
            | VR::AS
            | VR::PN
            | VR::SH
            | VR::LO
            | VR::UI
            | VR::UC
            | VR::CS
            | VR::IS
            | VR::DS
            | VR::DA
            | VR::TM
            | VR::DT => self.read_value_strs(from, header),
            VR::UT | VR::ST | VR::UR | VR::LT => self.read_value_str(from, header),
            VR::UN | VR::OB => self.read_value_ob(from, header),
            VR::US | VR::OW => self.read_value_us(from, header),
            VR::SS => self.read_value_ss(from, header),
            VR::FD | VR::OD => self.read_value_od(from, header),
            VR::FL | VR::OF => self.read_value_fl(from, header),
            VR::SL => self.read_value_sl(from, header),
            VR::OL | VR::UL => self.read_value_ul(from, header),
        }
    }

    fn set_character_set(&mut self, charset: SpecificCharacterSet) -> Result<()> {
        self.text = charset
            .get_codec()
            .ok_or_else(|| Error::UnsupportedCharacterSet)?;
        Ok(())
    }
}

fn parse_date(buf: &[u8]) -> Result<(NaiveDate, usize)> {
    // YYYY(MM(DD)?)?
    match buf.len() {
        0 | 1 | 2 | 3 | 5 | 7 => Err(InvalidValueReadError::UnexpectedEndOfElement.into()),
        4 => {
            let year = read_number_naive(buf);
            Ok((NaiveDate::from_ymd(year, 0, 0), 4))
        }
        6 => {
            let year = read_number_naive(&buf[0..4]);
            let month = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            Ok((NaiveDate::from_ymd(year, month as u32, 0), 6))
        }
        len => {
            debug_assert!(len >= 8);
            let year = read_number_naive(&buf[0..4]);
            let month = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            let day = (buf[6] as i32 - Z) * 10 + buf[7] as i32 - Z;
            Ok((NaiveDate::from_ymd(year, month as u32, day as u32), 8))
        }
    }
}

fn parse_time(buf: &[u8]) -> Result<(NaiveTime, usize)> {
    const Z: i32 = b'0' as i32;
    // HH(MM(SS(.F{1,6})?)?)?

    match buf.len() {
        0 | 1 | 3 | 5 | 7 => Err(InvalidValueReadError::UnexpectedEndOfElement.into()),
        2 => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            Ok((NaiveTime::from_hms(hour as u32, 0, 0), 2))
        }
        4 => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            let minute = (buf[2] as i32 - Z) * 10 + buf[3] as i32 - Z;
            Ok((NaiveTime::from_hms(hour as u32, minute as u32, 0), 4))
        }
        6 => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            let minute = (buf[2] as i32 - Z) * 10 + buf[3] as i32 - Z;
            let second = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            Ok((
                NaiveTime::from_hms(hour as u32, minute as u32, second as u32),
                6,
            ))
        }
        _ => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            let minute = (buf[2] as i32 - Z) * 10 + buf[3] as i32 - Z;
            let second = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            if buf[6] != b'.' {
                return Err(InvalidValueReadError::InvalidFormat.into());
            }
            // read at most 6 bytes
            let n = usize::min(6, buf.len() - 7);
            let mut fract = read_number_naive(&buf[7..7 + n]);
            let mut acc = n;
            while acc < 6 {
                fract *= 10;
                acc += 1;
            }
            Ok((
                NaiveTime::from_hms_micro(hour as u32, minute as u32, second as u32, fract as u32),
                7 + n,
            ))
        }
    }
}

#[inline]
fn read_number_naive(buf: &[u8]) -> i32 {
    (&buf[1..])
        .into_iter()
        .fold(buf[0] as i32 - Z, |acc, v| acc * 10 + *v as i32 - Z)
}

fn parse_datetime(buf: &[u8]) -> Result<DateTime<FixedOffset>> {
    let (date, bytes_read) = parse_date(buf)?;
    if buf.len() <= 8 {
        return Ok(FixedOffset::east(0).from_utc_date(&date).and_hms(0, 0, 0));
    }

    let (time, bytes_read) = parse_time(&buf[bytes_read..])?;
    if buf.len() == bytes_read + 8 {
        return FixedOffset::east(0)
            .from_utc_date(&date)
            .and_time(time)
            .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
    }

    let buf = &buf[bytes_read..];
    let len = buf.len();
    let offset = match len {
        0 => FixedOffset::east(0),
        1 => return Err(InvalidValueReadError::UnexpectedEndOfElement.into()),
        _ => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let (tz_h, tz_m) = match buf.len() {
                2 => (buf[0] as i32 - Z, 0),
                3 => (read_number_naive(&buf[0..2]), 0),
                _ => {
                    let tz_h = read_number_naive(&buf[0..2]);
                    let tz_m = read_number_naive(&buf[2..usize::min(4, buf.len())]);
                    (tz_h, tz_m)
                }
            };
            let s = (tz_h * 60 + tz_m) * 60;
            match tz_sign {
                b'+' => FixedOffset::east(s),
                b'-' => FixedOffset::west(s),
                _ => return Err(InvalidValueReadError::InvalidFormat.into()),
            }
        }
    };

    offset
        .from_utc_date(&date)
        .and_time(time)
        .ok_or_else(|| InvalidValueReadError::InvalidFormat.into())
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, NaiveDate, NaiveTime, TimeZone};
    use super::{parse_date, parse_datetime, parse_time};

    #[test]
    fn test_parse_date() {
        assert_eq!(
            parse_date(b"20180101").unwrap(),
            (NaiveDate::from_ymd(2018, 1, 1), 8)
        );
        assert_eq!(
            parse_date(b"19711231").unwrap(),
            (NaiveDate::from_ymd(1971, 12, 31), 8)
        );
        assert_eq!(
            parse_date(b"20140426").unwrap(),
            (NaiveDate::from_ymd(2014, 4, 26), 8)
        );
        assert_eq!(
            parse_date(b"20180101xxxx").unwrap(),
            (NaiveDate::from_ymd(2018, 1, 1), 8)
        );
    }

    #[test]
    fn test_time() {
        assert_eq!(
            parse_time(b"10").unwrap(),
            (NaiveTime::from_hms(10, 0, 0), 2)
        );
        assert_eq!(
            parse_time(b"0755").unwrap(),
            (NaiveTime::from_hms(7, 55, 0), 4)
        );
        assert_eq!(
            parse_time(b"075500").unwrap(),
            (NaiveTime::from_hms(7, 55, 0), 6)
        );
        assert_eq!(
            parse_time(b"075501.5").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 500_000), 8)
        );
        assert_eq!(
            parse_time(b"075501.58").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 580_000), 9)
        );
        assert_eq!(
            parse_time(b"075501.123456").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 123_456), 13)
        );
        assert_eq!(
            parse_time(b"075501.123456...").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 123_456), 13)
        );
    }

    #[test]
    fn test_datetime() {
        assert_eq!(
            parse_datetime(b"20180101").unwrap(),
            FixedOffset::east(0).ymd(2018, 1, 1).and_hms(0, 0, 0)
        );
        assert_eq!(
            parse_datetime(b"19711231").unwrap(),
            FixedOffset::east(0).ymd(1971, 12, 31).and_hms(0, 0, 0)
        );
        assert_eq!(
            parse_datetime(b"20140426").unwrap(),
            FixedOffset::east(0).ymd(2014, 4, 26).and_hms(0, 0, 0)
        );
    }
}
