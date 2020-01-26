//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use crate::error::{Error, Result};
use crate::util::n_times;
use chrono::FixedOffset;
use dicom_core::header::{DataElementHeader, Header, Length, SequenceItemHeader, Tag, VR};
use dicom_core::value::{PrimitiveValue, C};
use dicom_encoding::decode::basic::{BasicDecoder, LittleEndianBasicDecoder};
use dicom_encoding::decode::primitive_value::*;
use dicom_encoding::decode::{BasicDecode, DecodeFrom};
use dicom_encoding::error::{InvalidValueReadError, Result as EncodingResult, TextEncodingError};
use dicom_encoding::text::{
    validate_da, validate_dt, validate_tm, DefaultCharacterSetCodec, DynamicTextCodec,
    SpecificCharacterSet, TextCodec, TextValidationOutcome,
};
use dicom_encoding::transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
use dicom_encoding::transfer_syntax::{DynDecoder, TransferSyntax};
use smallvec::{smallvec, SmallVec};
use std::fmt;
use std::fmt::Debug;
use std::io::Read;
use std::iter::Iterator;
use std::marker::PhantomData;

/// A trait for abstracting the necessary parts
/// of a full DICOM content reading process.
pub trait Parse<S: ?Sized>
where
    S: Read,
{
    /// Same as `Decode::decode_header` over the bound source.
    fn decode_header(&mut self, from: &mut S) -> Result<DataElementHeader>;

    /// Same as `Decode::decode_item_header` over the bound source.
    fn decode_item_header(&mut self, from: &mut S) -> Result<SequenceItemHeader>;

    /// Eagerly read the following data in the source as a primitive data
    /// value. When reading values in text form, a conversion to a more
    /// maleable type is attempted. Namely, numbers in text form (IS, DS) are
    /// converted to the corresponding binary number types, and date/time
    /// instances are decoded into binary date/time objects of types defined in
    /// the `chrono` crate. To avoid this conversion, see
    /// `read_value_preserved`.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O problems, or if the header VR describes a
    /// sequence, which in that case this method should not be used.
    fn read_value(&mut self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue>;

    /// Eagerly read the following data in the source as a primitive data
    /// value. Unlike `read_value`, this method will preserve the DICOM value's
    /// original format: numbers saved as text, as well as dates and times, are
    /// read as strings.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O problems, or if the header VR describes a
    /// sequence, which in that case this method should not be used.
    fn read_value_preserved(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue>;

    /// Retrieve the exact number of bytes read by the parser.
    fn bytes_read(&self) -> u64;
}

/// Alias for a dynamically resolved DICOM parser. Although the data source may be known
/// in compile time, the required decoder may vary according to an object's transfer syntax.
pub type DynamicDicomParser<'s> =
    DicomParser<DynDecoder<dyn Read + 's>, BasicDecoder, dyn Read + 's, DynamicTextCodec>;

/// The initial capacity of the `DicomParser` buffer.
const PARSER_BUFFER_CAPACITY: usize = 2048;

/// A data structure for parsing DICOM data.
/// This type encapsulates the necessary codecs in order
/// to be as autonomous as possible in the DICOM content reading
/// process.
/// `S` is the generic parameter type for the original source's type,
/// `D` is the parameter type that the decoder interprets as,
/// whereas `DB` is the parameter type for the basic decoder.
/// `TextCodec` defines the text codec used underneath.
pub struct DicomParser<D, BD, S: ?Sized, TC> {
    phantom: PhantomData<S>,
    decoder: D,
    basic: BD,
    text: TC,
    dt_utc_offset: FixedOffset,
    buffer: Vec<u8>,
    bytes_read: u64,
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
    ($header:ident) => {
        match $header.len().get() {
            None => {
                return Err(Error::from(InvalidValueReadError::UnresolvedValueLength));
            }
            Some(len) => len as usize,
        }
    };
}

impl<'s> DynamicDicomParser<'s> {
    /// Create a new DICOM parser for the given transfer syntax and character set.
    pub fn new_with(ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self> {
        let basic = ts.basic_decoder();
        let decoder = ts
            .decoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax)?;
        let text = cs.codec().ok_or_else(|| Error::UnsupportedCharacterSet)?;

        Ok(DynamicDicomParser::new(decoder, basic, text))
    }
}

/// Type alias for the DICOM parser of a file's Meta group.
pub type FileHeaderParser<S> = DicomParser<
    ExplicitVRLittleEndianDecoder,
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
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            bytes_read: 0,
        }
    }
}

impl<D, BD, S: ?Sized, TC> DicomParser<D, BD, S, TC>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
    TC: TextCodec,
{
    /// Create a new DICOM parser from its parts.
    pub fn new(decoder: D, basic: BD, text: TC) -> DicomParser<D, BD, S, TC> {
        DicomParser {
            phantom: PhantomData,
            basic,
            decoder,
            text,
            dt_utc_offset: FixedOffset::east(0),
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            bytes_read: 0,
        }
    }

    // ---------------- private methods ---------------------

    fn read_value_tag(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);

        // tags
        let ntags = len >> 2;
        let parts: Result<C<Tag>> = n_times(ntags)
            .map(|_| {
                let g = self.basic.decode_us(&mut *from)?;
                let e = self.basic.decode_us(&mut *from)?;
                Ok(Tag(g, e))
            })
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::Tags(parts?))
    }

    fn read_value_ob(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        // TODO add support for OB value data length resolution
        // (might need to delegate pixel data reading to a separate trait)
        let len = require_known_length!(header);

        // sequence of 8-bit integers (or arbitrary byte data)
        let mut buf = smallvec![0u8; len];
        from.read_exact(&mut buf)?;
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U8(buf))
    }

    fn read_value_strs(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of strings
        self.buffer.resize_with(len, Default::default);
        from.read_exact(&mut self.buffer)?;

        let parts: EncodingResult<C<_>> = match header.vr() {
            VR::AE | VR::CS | VR::AS => self
                .buffer
                .split(|v| *v == b'\\')
                .map(|slice| DefaultCharacterSetCodec.decode(slice))
                .collect(),
            _ => self
                .buffer
                .split(|v| *v == b'\\')
                .map(|slice| self.text.decode(slice))
                .collect(),
        };

        self.bytes_read += len as u64;
        Ok(PrimitiveValue::Strs(parts?))
    }

    fn read_value_str(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);

        // a single string
        let mut buf: SmallVec<[u8; 16]> = smallvec![0u8; len];
        from.read_exact(&mut buf)?;
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::Str(self.text.decode(&buf[..])?))
    }

    fn read_value_ss(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        // sequence of 16-bit signed integers
        let len = require_known_length!(header);

        let n = len >> 1;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_ss(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::I16(vec?))
    }

    fn read_value_fl(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of 32-bit floats
        let n = len >> 2;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_fl(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::F32(vec?))
    }

    fn read_value_da(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of dates

        self.buffer.resize_with(len, Default::default);
        from.read_exact(&mut self.buffer)?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        if validate_da(buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return Err(TextEncodingError::new(format!(
                "Invalid date value element \"{}\"",
                lossy_str
            ))
            .into());
        }
        let vec: Result<C<_>> = buf
            .split(|b| *b == b'\\')
            .map(|part| Ok(parse_date(part)?.0))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::Date(vec?))
    }

    fn read_value_ds(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of doubles in text form

        self.buffer.resize_with(len, Default::default);
        from.read_exact(&mut self.buffer)?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        let parts: Result<C<f64>> = buf
            .split(|b| *b == b'\\')
            .map(|slice| {
                let codec = SpecificCharacterSet::Default.codec().unwrap();
                let txt = codec.decode(slice)?;
                let txt = txt.trim();
                txt.parse::<f64>()
                    .map_err(|e| Error::from(InvalidValueReadError::from(e)))
            })
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::F64(parts?))
    }

    fn read_value_dt(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of datetimes

        self.buffer.resize_with(len, Default::default);
        from.read_exact(&mut self.buffer)?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        if validate_dt(buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return Err(TextEncodingError::new(format!(
                "Invalid date-time value element \"{}\"",
                lossy_str
            ))
            .into());
        }
        let vec: Result<C<_>> = buf
            .split(|b| *b == b'\\')
            .map(|part| Ok(parse_datetime(part, self.dt_utc_offset)?))
            .collect();

        self.bytes_read += len as u64;
        Ok(PrimitiveValue::DateTime(vec?))
    }

    fn read_value_is(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of signed integers in text form
        self.buffer.resize_with(len, Default::default);
        from.read_exact(&mut self.buffer)?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        let parts: Result<C<_>> = buf
            .split(|v| *v == b'\\')
            .map(|slice| {
                let codec = SpecificCharacterSet::Default.codec().unwrap();
                let txt = codec.decode(slice)?;
                let txt = txt.trim();
                txt.parse::<i32>()
                    .map_err(|e| Error::from(InvalidValueReadError::from(e)))
            })
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::I32(parts?))
    }

    fn read_value_tm(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of time instances

        self.buffer.resize_with(len, Default::default);
        from.read_exact(&mut self.buffer)?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        if validate_tm(buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return Err(TextEncodingError::new(format!(
                "Invalid time value element \"{}\"",
                lossy_str
            ))
            .into());
        }
        let vec: std::result::Result<C<_>, _> = buf
            .split(|b| *b == b'\\')
            .map(|part| parse_time(part).map(|t| t.0))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::Time(vec?))
    }

    fn read_value_od(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of 64-bit floats
        let n = len >> 3;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_fd(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::F64(vec?))
    }

    fn read_value_ul(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of 32-bit unsigned integers

        let n = len >> 2;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_ul(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U32(vec?))
    }

    fn read_value_us(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of 16-bit unsigned integers

        let n = len >> 1;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_us(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U16(vec?))
    }

    fn read_value_uv(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of 64-bit unsigned integers

        let n = len >> 3;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_uv(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U64(vec?))
    }

    fn read_value_sl(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of 32-bit signed integers

        let n = len >> 2;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_sl(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::I32(vec?))
    }

    fn read_value_sv(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of 64-bit signed integers

        let n = len >> 3;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_sv(&mut *from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::I64(vec?))
    }
}

impl<S: ?Sized, D, BD> DicomParser<D, BD, S, Box<dyn TextCodec>>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
{
    fn set_character_set(&mut self, charset: SpecificCharacterSet) -> Result<()> {
        self.text = charset
            .codec()
            .ok_or_else(|| Error::UnsupportedCharacterSet)?;
        Ok(())
    }

    fn read_value_ui(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        let len = require_known_length!(header);
        // sequence of UID's
        self.buffer.resize_with(len, Default::default);
        from.read_exact(&mut self.buffer)?;

        let parts: EncodingResult<C<_>> = match header.vr() {
            VR::AE | VR::CS | VR::AS => self
                .buffer
                .split(|v| *v == 0)
                .map(|slice| DefaultCharacterSetCodec.decode(slice))
                .collect(),
            _ => self
                .buffer
                .split(|v| *v == 0)
                .map(|slice| self.text.decode(slice))
                .collect(),
        };

        let parts = parts?;
        self.bytes_read += len as u64;

        // if it's a Specific Character Set, update the parser immediately.
        if header.tag == Tag(0x0008, 0x0005) {
            // TODO trigger an error or warning on unsupported specific character sets.
            // Edge case handling strategies should be considered in the future.
            if let Some(charset) = parts
                .first()
                .map(|x| x.as_ref())
                .and_then(SpecificCharacterSet::from_code)
            {
                self.set_character_set(charset)?;
            }
        }

        Ok(PrimitiveValue::Strs(parts))
    }
}

impl<S: ?Sized, D, BD> Parse<S> for DicomParser<D, BD, S, Box<dyn TextCodec>>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
{
    fn decode_header(&mut self, from: &mut S) -> Result<DataElementHeader> {
        self.decoder
            .decode_header(from)
            .map(|(header, bytes_read)| {
                self.bytes_read += bytes_read as u64;
                header
            })
            .map_err(From::from)
    }

    fn decode_item_header(&mut self, from: &mut S) -> Result<SequenceItemHeader> {
        self.decoder
            .decode_item_header(from)
            .map(|header| {
                self.bytes_read += 8;
                header
            })
            .map_err(From::from)
    }

    fn read_value(&mut self, from: &mut S, header: &DataElementHeader) -> Result<PrimitiveValue> {
        if header.len() == Length(0) {
            return Ok(PrimitiveValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects should not head over here, they are
                // handled at a higher level
                Err(Error::from(InvalidValueReadError::NonPrimitiveType))
            }
            VR::AT => self.read_value_tag(from, header),
            VR::AE | VR::AS | VR::PN | VR::SH | VR::LO | VR::UC | VR::CS => {
                self.read_value_strs(from, header)
            }
            VR::UI => self.read_value_ui(from, header),
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
            VR::SV => self.read_value_sv(from, header),
            VR::OL | VR::UL => self.read_value_ul(from, header),
            VR::OV | VR::UV => self.read_value_uv(from, header),
        }
    }

    fn read_value_preserved(
        &mut self,
        from: &mut S,
        header: &DataElementHeader,
    ) -> Result<PrimitiveValue> {
        if header.len() == Length(0) {
            return Ok(PrimitiveValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                Err(Error::from(InvalidValueReadError::NonPrimitiveType))
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
            VR::SV => self.read_value_sv(from, header),
            VR::OV | VR::UV => self.read_value_uv(from, header),
        }
    }

    fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

/// Remove trailing spaces and null characters.
fn trim_trail_empty_bytes(mut x: &[u8]) -> &[u8] {
    while x.last() == Some(&b' ') || x.last() == Some(&b'\0') {
        x = &x[..x.len() - 1];
    }
    x
}
