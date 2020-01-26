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
use smallvec::smallvec;
use std::fmt::Debug;
use std::io::Read;
use std::iter::Iterator;

pub trait StatefulDecode {
    type Reader: Read;

    /// Same as `Decode::decode_header` over the bound source.
    fn decode_header(&mut self) -> Result<DataElementHeader>;

    /// Same as `Decode::decode_item_header` over the bound source.
    fn decode_item_header(&mut self) -> Result<SequenceItemHeader>;

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
    fn read_value(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue>;

    /// Eagerly read the following data in the source as a primitive data
    /// value. Unlike `read_value`, this method will preserve the DICOM value's
    /// original format: numbers saved as text, as well as dates and times, are
    /// read as strings.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O problems, or if the header VR describes a
    /// sequence, which in that case this method should not be used.
    fn read_value_preserved(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue>;

    /// Eagerly read the following data in the source as a primitive data
    /// value as bytes, regardless of its value representation.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O problems, or if the header VR describes a
    /// sequence, which in that case this method should not be used.
    fn read_value_bytes(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue>;

    /// Obtain a reader which outlines the primitive value data from the
    /// given source.
    fn value_reader(
        &mut self,
        header: &DataElementHeader,
    ) -> Result<std::io::Take<&mut Self::Reader>>;

    /// Retrieve the exact number of bytes read so far by the stateful decoder.
    fn bytes_read(&self) -> u64;
}

/// Alias for a dynamically resolved DICOM stateful decoder. Although the data
/// source may be known at compile time, the required decoder may vary
/// according to an object's transfer syntax.
pub type DynStatefulDecoder<'s> =
    StatefulDecoder<DynDecoder<dyn Read + 's>, BasicDecoder, Box<dyn Read + 's>, DynamicTextCodec>;

/// The initial capacity of the `DicomParser` buffer.
const PARSER_BUFFER_CAPACITY: usize = 2048;

/// A stateful abstraction for the full DICOM content reading process.
/// This type encapsulates the necessary codecs in order
/// to be as autonomous as possible in the DICOM content reading
/// process.
/// `S` is the generic parameter type for the original source,
/// `D` is the parameter type that the decoder interprets as,
/// whereas `DB` is the parameter type for the basic decoder.
/// `TC` defines the text codec used underneath.
#[derive(Debug)]
pub struct StatefulDecoder<D, BD, S, TC> {
    from: S,
    decoder: D,
    basic: BD,
    text: TC,
    dt_utc_offset: FixedOffset,
    buffer: Vec<u8>,
    bytes_read: u64,
}

pub type DicomParser<D, BD, S, TC> = StatefulDecoder<D, BD, S, TC>;

impl<'s> DynStatefulDecoder<'s> {
    /// Create a new DICOM parser for the given transfer syntax and character set.
    pub fn new_with<S: 's>(from: S, ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self>
    where
        S: Read,
    {
        let basic = ts.basic_decoder();
        let decoder = ts
            .decoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax)?;
        let text = cs.codec().ok_or_else(|| Error::UnsupportedCharacterSet)?;

        Ok(DynStatefulDecoder::new(
            Box::from(from),
            decoder,
            basic,
            text,
        ))
    }
}

/// Type alias for the DICOM parser of a file's Meta group.
pub type FileHeaderParser<S> = StatefulDecoder<
    ExplicitVRLittleEndianDecoder,
    LittleEndianBasicDecoder,
    S,
    DefaultCharacterSetCodec,
>;

impl<S> FileHeaderParser<S>
where
    S: Read,
{
    /// Create a new DICOM stateful decoder for reading the file meta header,
    /// which is always in _Explicit VR Little Endian_.
    pub fn file_header_parser(from: S) -> Self {
        DicomParser {
            from,
            basic: LittleEndianBasicDecoder::default(),
            decoder: ExplicitVRLittleEndianDecoder::default(),
            text: DefaultCharacterSetCodec,
            dt_utc_offset: FixedOffset::east(0),
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            bytes_read: 0,
        }
    }
}

impl<D, BD, S, TC> StatefulDecoder<D, BD, S, TC> {
    /// Create a new DICOM stateful decoder from its parts.
    pub fn new(from: S, decoder: D, basic: BD, text: TC) -> StatefulDecoder<D, BD, S, TC> {
        DicomParser {
            from,
            basic,
            decoder,
            text,
            dt_utc_offset: FixedOffset::east(0),
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            bytes_read: 0,
        }
    }
}

impl<D, T, BD, S, TC> StatefulDecoder<D, BD, S, TC>
where
    D: DecodeFrom<T>,
    BD: BasicDecode,
    S: std::ops::DerefMut<Target = T> + Read,
    T: ?Sized + Read,
    TC: TextCodec,
{
    // ---------------- private methods ---------------------

    fn read_value_tag(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;

        // tags
        let ntags = len >> 2;
        let parts: Result<C<Tag>> = n_times(ntags)
            .map(|_| {
                let g = self.basic.decode_us(&mut self.from)?;
                let e = self.basic.decode_us(&mut self.from)?;
                Ok(Tag(g, e))
            })
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::Tags(parts?))
    }

    fn read_value_ob(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        // TODO add support for OB value data length resolution
        // (might need to delegate pixel data reading to a separate trait)
        let len = require_known_length(header)?;

        // sequence of 8-bit integers (or arbitrary byte data)
        let mut buf = smallvec![0u8; len];
        self.from.read_exact(&mut buf)?;
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U8(buf))
    }

    fn read_value_strs(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of strings
        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;

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

    fn read_value_str(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;

        // a single string
        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::Str(self.text.decode(&self.buffer[..])?))
    }

    fn read_value_ss(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        // sequence of 16-bit signed integers
        let len = require_known_length(header)?;

        let n = len >> 1;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_ss(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::I16(vec?))
    }

    fn read_value_fl(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of 32-bit floats
        let n = len >> 2;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_fl(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::F32(vec?))
    }

    fn read_value_da(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of dates

        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;
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

    fn read_value_ds(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of doubles in text form

        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;
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

    fn read_value_dt(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of datetimes

        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;
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

    fn read_value_is(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of signed integers in text form
        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;
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

    fn read_value_tm(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of time instances

        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;
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

    fn read_value_od(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of 64-bit floats
        let n = len >> 3;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_fd(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::F64(vec?))
    }

    fn read_value_ul(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of 32-bit unsigned integers

        let n = len >> 2;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_ul(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U32(vec?))
    }

    fn read_value_us(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of 16-bit unsigned integers

        let n = len >> 1;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_us(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U16(vec?))
    }

    fn read_value_uv(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of 64-bit unsigned integers

        let n = len >> 3;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_uv(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::U64(vec?))
    }

    fn read_value_sl(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of 32-bit signed integers

        let n = len >> 2;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_sl(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::I32(vec?))
    }

    fn read_value_sv(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of 64-bit signed integers

        let n = len >> 3;
        let vec: EncodingResult<C<_>> = n_times(n)
            .map(|_| self.basic.decode_sv(&mut self.from))
            .collect();
        self.bytes_read += len as u64;
        Ok(PrimitiveValue::I64(vec?))
    }
}

impl<S, T, D, BD> StatefulDecoder<D, BD, S, Box<dyn TextCodec>>
where
    D: DecodeFrom<T>,
    BD: BasicDecode,
    S: std::ops::DerefMut<Target = T> + Read,
    T: ?Sized + Read,
{
    fn set_character_set(&mut self, charset: SpecificCharacterSet) -> Result<()> {
        self.text = charset
            .codec()
            .ok_or_else(|| Error::UnsupportedCharacterSet)?;
        Ok(())
    }

    fn read_value_ui(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = require_known_length(header)?;
        // sequence of UID's
        self.buffer.resize_with(len, Default::default);
        self.from.read_exact(&mut self.buffer)?;

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

        // if it's a Specific Character Set, update the decoder immediately.
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

impl<S, T, D, BD> StatefulDecode for StatefulDecoder<D, BD, S, Box<dyn TextCodec>>
where
    D: DecodeFrom<T>,
    BD: BasicDecode,
    S: std::ops::DerefMut<Target = T> + Read,
    T: ?Sized + Read,
{
    type Reader = S;

    fn decode_header(&mut self) -> Result<DataElementHeader> {
        self.decoder
            .decode_header(&mut self.from)
            .map(|(header, bytes_read)| {
                self.bytes_read += bytes_read as u64;
                header
            })
            .map_err(From::from)
    }

    fn decode_item_header(&mut self) -> Result<SequenceItemHeader> {
        self.decoder
            .decode_item_header(&mut self.from)
            .map(|header| {
                self.bytes_read += 8;
                header
            })
            .map_err(From::from)
    }

    fn read_value(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        if header.len() == Length(0) {
            return Ok(PrimitiveValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects should not head over here, they are
                // handled at a higher level
                Err(Error::from(InvalidValueReadError::NonPrimitiveType))
            }
            VR::AT => self.read_value_tag(header),
            VR::AE | VR::AS | VR::PN | VR::SH | VR::LO | VR::UC | VR::CS => {
                self.read_value_strs(header)
            }
            VR::UI => self.read_value_ui(header),
            VR::UT | VR::ST | VR::UR | VR::LT => self.read_value_str(header),
            VR::UN | VR::OB => self.read_value_ob(header),
            VR::US | VR::OW => self.read_value_us(header),
            VR::SS => self.read_value_ss(header),
            VR::DA => self.read_value_da(header),
            VR::DT => self.read_value_dt(header),
            VR::TM => self.read_value_tm(header),
            VR::DS => self.read_value_ds(header),
            VR::FD | VR::OD => self.read_value_od(header),
            VR::FL | VR::OF => self.read_value_fl(header),
            VR::IS => self.read_value_is(header),
            VR::SL => self.read_value_sl(header),
            VR::SV => self.read_value_sv(header),
            VR::OL | VR::UL => self.read_value_ul(header),
            VR::OV | VR::UV => self.read_value_uv(header),
        }
    }

    fn read_value_preserved(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        if header.len() == Length(0) {
            return Ok(PrimitiveValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                Err(Error::from(InvalidValueReadError::NonPrimitiveType))
            }
            VR::AT => self.read_value_tag(header),
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
            | VR::DT => self.read_value_strs(header),
            VR::UT | VR::ST | VR::UR | VR::LT => self.read_value_str(header),
            VR::UN | VR::OB => self.read_value_ob(header),
            VR::US | VR::OW => self.read_value_us(header),
            VR::SS => self.read_value_ss(header),
            VR::FD | VR::OD => self.read_value_od(header),
            VR::FL | VR::OF => self.read_value_fl(header),
            VR::SL => self.read_value_sl(header),
            VR::OL | VR::UL => self.read_value_ul(header),
            VR::SV => self.read_value_sv(header),
            VR::OV | VR::UV => self.read_value_uv(header),
        }
    }

    fn read_value_bytes(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        if header.len() == Length(0) {
            return Ok(PrimitiveValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                Err(Error::from(InvalidValueReadError::NonPrimitiveType))
            }
            _ => self.read_value_ob(header),
        }
    }

    /// Obtain a reader which outlines the primitive value data from the
    /// given source.
    fn value_reader(
        &mut self,
        header: &DataElementHeader,
    ) -> Result<std::io::Take<&mut Self::Reader>> {
        match header.vr() {
            VR::SQ => {
                // sequence objects... should not work
                Err(Error::from(InvalidValueReadError::NonPrimitiveType))
            }
            _ => Ok(self
                .from
                .by_ref()
                .take(u64::from(header.len().get().unwrap_or(100_000)))),
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

fn require_known_length(
    header: &DataElementHeader,
) -> std::result::Result<usize, InvalidValueReadError> {
    header
        .len()
        .get()
        .map(|len| len as usize)
        .ok_or_else(|| InvalidValueReadError::UnresolvedValueLength)
}
