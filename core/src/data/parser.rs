//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use std::fmt;
use std::fmt::Debug;
use std::io::Read;
use std::marker::PhantomData;
use std::iter::Iterator;
use std::ops::{Add, Mul, Sub};
use error::{Error, InvalidValueReadError, Result, TextEncodingError};
use data::{DataElementHeader, Header, SequenceItemHeader, Tag, VR};
use data::decode::{BasicDecode, Decode};
use data::decode::basic::{BasicDecoder, LittleEndianBasicDecoder};
use data::text::{DynamicTextCodec, SpecificCharacterSet, TextCodec};
use data::value::PrimitiveValue;
use transfer_syntax::TransferSyntax;
use transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
use data::text::{validate_da, validate_dt, validate_tm, DefaultCharacterSetCodec,
                 TextValidationOutcome};
use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveTime, TimeZone};
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
    fn read_value(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue>;

    /// Eagerly read the following data in the source as a data value.
    /// Unlike `read_value`, this method will preserve the DICOM value's
    /// original format: numbers saved as text, as well as dates and times,
    /// are read as strings.
    fn read_value_preserved(
        &self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue>;

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
    dt_utc_offset: FixedOffset,
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
            .field("dt_utc_offset", &self.dt_utc_offset)
            .finish()
    }
}

macro_rules! require_known_length {
    ($header: ident) => {
        if $header.len() == 0xFFFFFFFF {
            return Err(Error::from(InvalidValueReadError::UnresolvedValueLength));
        }
    };
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
            dt_utc_offset: FixedOffset::east(0),
        })
    }
}

/// Type alias for the DICOM parser of a file's Meta group.
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
            dt_utc_offset: FixedOffset::east(0),
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
            dt_utc_offset: FixedOffset::east(0),
        }
    }

    // ---------------- private methods ---------------------

    fn read_value_tag(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
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
        Ok(PrimitiveValue::Tags(parts?))
    }

    fn read_value_ob(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        // TODO add support for OB value data length resolution
        require_known_length!(header);

        // sequence of 8-bit integers (or just byte data)
        let mut buf = vec![0u8; header.len() as usize];
        from.read_exact(&mut buf)?;
        Ok(PrimitiveValue::U8(buf))
    }

    fn read_value_strs(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
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
                .collect(),
        };

        Ok(PrimitiveValue::Strs(parts?))
    }

    fn read_value_str(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);

        // a single string
        let mut buf = vec![0u8; header.len() as usize];
        try!(from.read_exact(&mut buf));
        Ok(PrimitiveValue::Str(self.text.decode(&buf[..])?))
    }

    fn read_value_ss(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        // sequence of 16-bit signed integers
        require_known_length!(header);

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_ss(&mut *from)?);
        }
        Ok(PrimitiveValue::I16(vec))
    }

    fn read_value_fl(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of 32-bit floats
        let l = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(l);
        for _ in n_times(l) {
            vec.push(self.basic.decode_fl(&mut *from)?);
        }
        Ok(PrimitiveValue::F32(vec))
    }

    fn read_value_da(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of dates

        // maybe one day I should find a way to get rid of this dynamic allocation
        let mut buf = vec![0u8; header.len() as usize];
        from.read_exact(&mut buf)?;
        if validate_da(&buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(&buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return Err(TextEncodingError::new(format!(
                "Invalid time value element \"{}\"",
                lossy_str
            )).into());
        }
        let vec: Result<Vec<_>> = buf.split(|b| *b == b'\\')
            .map(|part| Ok(parse_date(part)?.0))
            .collect();
        Ok(PrimitiveValue::Date(vec?))
    }

    fn read_value_ds(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
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
        Ok(PrimitiveValue::F64(parts?))
    }

    fn read_value_dt(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of datetimes

        // dynamic allocation
        let mut buf = vec![0u8; header.len() as usize];
        from.read_exact(&mut buf)?;
        if validate_dt(&buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(&buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return Err(TextEncodingError::new(format!(
                "Invalid time value element \"{}\"",
                lossy_str
            )).into());
        }
        let vec: Result<Vec<_>> = buf.split(|b| *b == b'\\')
            .map(|part| Ok(parse_datetime(part, &self.dt_utc_offset)?))
            .collect();

        Ok(PrimitiveValue::DateTime(vec?))
    }

    fn read_value_is(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of signed integers in text form
        let mut buf = vec![0u8; header.len() as usize];
        from.read_exact(&mut buf)?;

        let last = if let Some(c) = buf.last() { *c } else { 0u8 };
        if last == ' ' as u8 {
            buf.pop();
        }
        let parts: Result<Vec<_>> = buf[..]
            .split(|v| *v == '\\' as u8)
            .map(|slice| {
                let codec = SpecificCharacterSet::Default.get_codec().unwrap();
                let txt = codec.decode(slice)?;
                txt.parse::<i32>()
                    .map_err(|e| Error::from(InvalidValueReadError::from(e)))
            })
            .collect();
        Ok(PrimitiveValue::I32(parts?))
    }

    fn read_value_tm(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of time instances

        // dynamic allocation
        let mut buf = vec![0u8; header.len() as usize];
        from.read_exact(&mut buf)?;
        if validate_tm(&buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(&buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return Err(TextEncodingError::new(format!(
                "Invalid time value element \"{}\"",
                lossy_str
            )).into());
        }
        let vec: Result<Vec<_>> = buf.split(|b| *b == b'\\')
            .map(|part| Ok(parse_time(part)?.0))
            .collect();
        Ok(PrimitiveValue::Time(vec?))
    }

    fn read_value_od(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of 64-bit floats
        let len = header.len() as usize >> 3;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_fd(&mut *from)?);
        }
        Ok(PrimitiveValue::F64(vec))
    }

    fn read_value_ul(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of 32-bit unsigned integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_ul(&mut *from)?);
        }
        Ok(PrimitiveValue::U32(vec))
    }

    fn read_value_us(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of 16-bit unsigned integers

        let len = header.len() as usize >> 1;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_us(&mut *from)?);
        }
        Ok(PrimitiveValue::U16(vec))
    }

    fn read_value_sl(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        require_known_length!(header);
        // sequence of 32-bit signed integers

        let len = header.len() as usize >> 2;
        let mut vec = Vec::with_capacity(len);
        for _ in n_times(len) {
            vec.push(self.basic.decode_sl(&mut *from)?);
        }
        Ok(PrimitiveValue::I32(vec))
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

    fn read_value(&self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        if header.len() == 0 {
            return Ok(PrimitiveValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects should not head over here, they are
                // handled at a higher level
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
            VR::DA => self.read_value_da(from, header),
            VR::DT => self.read_value_dt(from, header),
            VR::TM => self.read_value_tm(from, header),
            VR::DS => self.read_value_ds(from, header),
            VR::FD | VR::OD => self.read_value_od(from, header),
            VR::FL | VR::OF => self.read_value_fl(from, header),
            VR::IS => self.read_value_is(from, header),
            VR::SL => self.read_value_sl(from, header),
            VR::OL | VR::UL => self.read_value_ul(from, header),
        }
    }

    fn read_value_preserved(
        &self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        if header.len() == 0 {
            return Ok(PrimitiveValue::Empty);
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
            let year = read_number(buf)?;
            let date: Result<_> = NaiveDate::from_ymd_opt(year, 0, 0)
                .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
            Ok((date?, 4))
        }
        6 => {
            let year = read_number(&buf[0..4])?;
            let month = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            let date: Result<_> = NaiveDate::from_ymd_opt(year, month as u32, 0)
                .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
            Ok((date?, 6))
        }
        len => {
            debug_assert!(len >= 8);
            let year = read_number(&buf[0..4])?;
            let month = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            let day = (buf[6] as i32 - Z) * 10 + buf[7] as i32 - Z;
            let date: Result<_> = NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
            Ok((date?, 8))
        }
    }
}

fn parse_time(buf: &[u8]) -> Result<(NaiveTime, usize)> {
    parse_time_impl(buf, false)
}

fn parse_time_impl(mut buf: &[u8], for_datetime: bool) -> Result<(NaiveTime, usize)> {
    const Z: i32 = b'0' as i32;
    // HH(MM(SS(.F{1,6})?)?)?

    let mut suffix_offset = 0;
    if !for_datetime {
        // perform a single trailing space trim
        if let Some(b' ') = buf.last().map(|x| *x) {
            buf = &buf[..buf.len() - 1];
            suffix_offset = 1;
        }
    }

    match buf.len() {
        0 | 1 | 3 | 5 | 7 => Err(InvalidValueReadError::UnexpectedEndOfElement.into()),
        2 => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            let time: Result<_> = NaiveTime::from_hms_opt(hour as u32, 0, 0)
                .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
            Ok((time?, 2 + suffix_offset))
        }
        4 => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            let minute = (buf[2] as i32 - Z) * 10 + buf[3] as i32 - Z;
            let time: Result<_> = NaiveTime::from_hms_opt(hour as u32, minute as u32, 0)
                .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
            Ok((time?, 4 + suffix_offset))
        }
        6 => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            let minute = (buf[2] as i32 - Z) * 10 + buf[3] as i32 - Z;
            let second = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            let time: Result<_> =
                NaiveTime::from_hms_opt(hour as u32, minute as u32, second as u32)
                    .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
            Ok((time?, 6 + suffix_offset))
        }
        _ => {
            let hour = (buf[0] as i32 - Z) * 10 + buf[1] as i32 - Z;
            let minute = (buf[2] as i32 - Z) * 10 + buf[3] as i32 - Z;
            let second = (buf[4] as i32 - Z) * 10 + buf[5] as i32 - Z;
            match buf[6] {
                b'.' => { /* do nothing */ }
                b'+' | b'-' if for_datetime => { /* do nothing */ }
                _ => return Err(InvalidValueReadError::InvalidFormat.into()),
            }
            let buf = &buf[7..];
            // read at most 6 bytes
            let mut n = usize::min(6, buf.len());
            if for_datetime {
                // check for time zone suffix, restrict fraction size accordingly
                if let Some(i) = buf.into_iter().position(|v| *v == b'+' || *v == b'-') {
                    n = i;
                }
            }
            let mut fract: u32 = read_number(&buf[0..n])?;
            let mut acc = n;
            while acc < 6 {
                fract *= 10;
                acc += 1;
            }
            let time: Result<_> = NaiveTime::from_hms_micro_opt(
                hour as u32,
                minute as u32,
                second as u32,
                fract,
            ).ok_or_else(|| InvalidValueReadError::InvalidFormat.into());

            Ok((time?, 7 + n + suffix_offset))
        }
    }
}

trait Ten {
    fn ten() -> Self;
}

macro_rules! impl_integral_ten {
    ($t: ty) => {
        impl Ten for $t {
            fn ten() -> Self {
                10
            }
        }
    };
}

macro_rules! impl_floating_ten {
    ($t: ty) => {
        impl Ten for $t {
            fn ten() -> Self {
                10.
            }
        }
    };
}

impl_integral_ten!(i16);
impl_integral_ten!(u16);
impl_integral_ten!(i32);
impl_integral_ten!(u32);
impl_integral_ten!(i64);
impl_integral_ten!(u64);
impl_integral_ten!(isize);
impl_integral_ten!(usize);
impl_floating_ten!(f32);
impl_floating_ten!(f64);

fn read_number<T>(text: &[u8]) -> Result<T>
where
    T: Ten,
    T: From<u8>,
    T: Add<T, Output = T>,
    T: Mul<T, Output = T>,
    T: Sub<T, Output = T>,
{
    if text.len() == 0 || text.len() > 9 {
        return Err(InvalidValueReadError::InvalidFormat.into());
    }
    if text.into_iter().any(|b| *b < b'0' || *b > b'9') {
        return Err(InvalidValueReadError::InvalidFormat.into());
    }

    Ok(read_number_unchecked(text))
}

#[inline]
fn read_number_unchecked<T>(buf: &[u8]) -> T
where
    T: Ten,
    T: From<u8>,
    T: Add<T, Output = T>,
    T: Mul<T, Output = T>,
{
    debug_assert!(buf.len() > 0);
    debug_assert!(buf.len() < 10);
    (&buf[1..])
        .into_iter()
        .fold((buf[0] - b'0').into(), |acc, v| {
            acc * T::ten() + (*v - b'0').into()
        })
}

fn parse_datetime(mut buf: &[u8], dt_utc_offset: &FixedOffset) -> Result<DateTime<FixedOffset>> {
    // perform a single trailing space trim
    if let Some(b' ') = buf.last().map(|x| *x) {
        buf = &buf[..buf.len() - 1];
    }
    let (date, bytes_read) = parse_date(buf)?;
    if buf.len() <= 8 {
        return Ok(FixedOffset::east(0).from_utc_date(&date).and_hms(0, 0, 0));
    }
    let buf = &buf[bytes_read..];
    let (time, bytes_read) = parse_time_impl(buf, true)?;
    if buf.len() == bytes_read {
        return FixedOffset::east(0)
            .from_utc_date(&date)
            .and_time(time)
            .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
    }

    let buf = &buf[bytes_read..];
    let len = buf.len();
    let offset = match len {
        0 => {
            // A Date Time value without the optional suffix should be interpreted to be
            // the local time zone of the application creating the Data Element, and can
            // be overridden by the _Timezone Offset from UTC_ attribute.
            let local_dt: Result<_> = Local
                .from_local_date(&date)
                .and_time(time)
                .single()
                .ok_or_else(|| InvalidValueReadError::InvalidFormat.into());
            let dt = local_dt?.with_timezone(dt_utc_offset);
            return Ok(dt);
        }
        1 | 2 => return Err(InvalidValueReadError::UnexpectedEndOfElement.into()),
        _ => {
            let tz_sign = buf[0];
            let buf = &buf[1..];
            let (tz_h, tz_m) = match buf.len() {
                1 => (buf[0] as i32 - Z, 0),
                2 => return Err(InvalidValueReadError::UnexpectedEndOfElement.into()),
                _ => {
                    let (h_buf, m_buf) = buf.split_at(2);
                    let tz_h = read_number(h_buf)?;
                    let tz_m = read_number(&m_buf[0..usize::min(2, m_buf.len())])?;
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
        assert!(parse_date(b"").is_err());
        assert!(parse_date(b"        ").is_err());
        assert!(parse_date(b"--------").is_err());
        assert!(parse_date(&[0x00_u8; 8]).is_err());
        assert!(parse_date(&[0xFF_u8; 8]).is_err());
        assert!(parse_date(&[b'0'; 8]).is_err());
        assert!(parse_date(b"19991313").is_err());
        assert!(parse_date(b"20180229").is_err());
        assert!(parse_date(b"nothing!").is_err());
        assert!(parse_date(b"2012dec").is_err());
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
            parse_time(b"075501.58 ").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 580_000), 10)
        );
        assert_eq!(
            parse_time(b"101010.204").unwrap(),
            (NaiveTime::from_hms_micro(10, 10, 10, 204_000), 10)
        );
        assert_eq!(
            parse_time(b"075501.123456").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 123_456), 13)
        );
        assert_eq!(
            parse_time(b"075501.123456 ").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 123_456), 14)
        );
        assert_eq!(
            parse_time(b"235959.99999").unwrap(),
            (NaiveTime::from_hms_micro(23, 59, 59, 999_990), 12)
        );
        assert_eq!(
            parse_time(b"235959.999999").unwrap(),
            (NaiveTime::from_hms_micro(23, 59, 59, 999_999), 13)
        );
        assert_eq!(
            parse_time(b"235959.999999 ").unwrap(),
            (NaiveTime::from_hms_micro(23, 59, 59, 999_999), 14)
        );
        assert_eq!(
            parse_time(b"075501.123456...").unwrap(),
            (NaiveTime::from_hms_micro(7, 55, 1, 123_456), 13)
        );
        assert!(parse_date(b"").is_err());
        assert!(parse_date(&[0x00_u8; 6]).is_err());
        assert!(parse_date(&[0xFF_u8; 6]).is_err());
        assert!(parse_date(b"      ").is_err());
        assert!(parse_date(b"------").is_err());
        assert!(parse_date(b"------.----").is_err());
        assert!(parse_date(b"235959.9999").is_err());
        assert!(parse_date(b"075501.").is_err());
        assert!(parse_date(b"075501.----").is_err());
        assert!(parse_date(b"nope").is_err());
        assert!(parse_date(b"235800.0a").is_err());
    }

    #[test]
    fn test_datetime() {
        let default_offset = FixedOffset::east(0);
        assert_eq!(
            parse_datetime(b"201801010930", &default_offset).unwrap(),
            FixedOffset::east(0).ymd(2018, 1, 1).and_hms(9, 30, 0)
        );
        assert_eq!(
            parse_datetime(b"19711231065003", &default_offset).unwrap(),
            FixedOffset::east(0).ymd(1971, 12, 31).and_hms(6, 50, 3)
        );
        assert_eq!(
            parse_datetime(b"20171130101010.204", &default_offset).unwrap(),
            FixedOffset::east(0)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            parse_datetime(b"20180314000000.25 ", &default_offset).unwrap(),
            FixedOffset::east(0)
                .ymd(2018, 03, 14)
                .and_hms_micro(0, 0, 0, 250_000)
        );
        let dt = parse_datetime(b"20171130101010.204+0100", &default_offset).unwrap();
        assert_eq!(
            dt,
            FixedOffset::east(3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            format!("{:?}", dt),
            "2017-11-30T10:10:10.204+01:00".to_string()
        );
        let dt = parse_datetime(b"20171130101010.204+0100 ", &default_offset).unwrap();
        assert_eq!(
            dt,
            FixedOffset::east(3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            format!("{:?}", dt),
            "2017-11-30T10:10:10.204+01:00".to_string()
        );

        assert_eq!(
            parse_datetime(b"20171130101010.204-1000", &default_offset).unwrap(),
            FixedOffset::west(10 * 3600)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        let dt = parse_datetime(b"20171130101010.204+0535", &default_offset).unwrap();
        assert_eq!(
            dt,
            FixedOffset::east(5 * 3600 + 35 * 60)
                .ymd(2017, 11, 30)
                .and_hms_micro(10, 10, 10, 204_000)
        );
        assert_eq!(
            format!("{:?}", dt),
            "2017-11-30T10:10:10.204+05:35".to_string()
        );
        assert_eq!(
            parse_datetime(b"20140426", &default_offset).unwrap(),
            FixedOffset::east(0).ymd(2014, 4, 26).and_hms(0, 0, 0)
        );

        assert!(parse_datetime(b"", &default_offset).is_err());
        assert!(parse_datetime(&[0x00_u8; 8], &default_offset).is_err());
        assert!(parse_datetime(&[0xFF_u8; 8], &default_offset).is_err());
        assert!(parse_datetime(&[b'0'; 8], &default_offset).is_err());
        assert!(parse_datetime(&[b' '; 8], &default_offset).is_err());
        assert!(parse_datetime(b"nope", &default_offset).is_err());
        assert!(parse_datetime(b"2015dec", &default_offset).is_err());
        assert!(parse_datetime(b"20151231162945.", &default_offset).is_err());
        assert!(parse_datetime(b"20151130161445+", &default_offset).is_err());
        assert!(parse_datetime(b"20151130161445+----", &default_offset).is_err());
        assert!(parse_datetime(b"20151130161445. ", &default_offset).is_err());
        assert!(parse_datetime(b"20151130161445. +0000", &default_offset).is_err());
        assert!(parse_datetime(b"20100423164000.001+3", &default_offset).is_err());
        assert!(parse_datetime(b"200809112945*1000", &default_offset).is_err());
    }
}
