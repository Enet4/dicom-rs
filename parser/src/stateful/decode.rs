//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use crate::error::{Error, Result};
use crate::util::n_times;
use byteordered::{ByteOrdered, Endianness};
use chrono::FixedOffset;
use dicom_core::header::{
    DataElementHeader, HasLength, Header, Length, SequenceItemHeader, Tag, VR,
};
use dicom_core::value::{PrimitiveValue, C};
use dicom_core::ReadSeek;
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
use std::fmt::Debug;
use std::io::{self, Read, Seek, SeekFrom};
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

    /// Read the exact amount of bytes to fill the buffer.
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()>;

    /// Retrieve the exact number of bytes read so far by the stateful decoder.
    fn bytes_read(&self) -> u64;
}

/// Alias for a dynamically resolved DICOM stateful decoder. Although the data
/// source may be known at compile time, the required decoder may vary
/// according to an object's transfer syntax.
pub type DynStatefulDecoder<'s> = StatefulDecoder<
    DynDecoder<dyn ReadSeek + 's>,
    BasicDecoder,
    Box<dyn ReadSeek + 's>,
    DynamicTextCodec,
>;

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
        S: Read + Seek,
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
    S: std::ops::DerefMut<Target = T> + Read + Seek,
    T: ?Sized + Read + Seek,
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
        // (might need to delegate pixel data reading to a separate trait)
        if let Some(len) = header.length().get() {
            // sequence of 8-bit integers (or arbitrary byte data)
            let mut buf = smallvec![0u8; len as usize];
            self.from.read_exact(&mut buf)?;
            self.bytes_read += len as u64;
            Ok(PrimitiveValue::U8(buf))
        } else {
            let bytes_to_find = tag_as_bytes(
                SequenceItemHeader::SequenceDelimiter.tag(),
                self.basic.endianness(),
            );
            let out = read_until_marker(&mut self.from, &bytes_to_find)?;

            Ok(PrimitiveValue::U8(SmallVec::from_vec(out)))
        }
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

impl<S, T, D, BD> StatefulDecoder<D, BD, S, DynamicTextCodec>
where
    D: DecodeFrom<T>,
    BD: BasicDecode,
    S: std::ops::DerefMut<Target = T> + Read + Seek,
    T: ?Sized + Read + Seek,
{
    fn set_character_set(&mut self, charset: SpecificCharacterSet) -> Result<()> {
        self.text = charset
            .codec()
            .ok_or_else(|| Error::UnsupportedCharacterSet)?;
        Ok(())
    }

    /// Read a sequence of UID values. Similar to `read_value_strs`, but also
    /// triggers a character set change when it finds the _SpecificCharacterSet_
    /// attribute.
    fn read_value_ui(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let out = self.read_value_strs(header)?;

        let parts = match &out {
            PrimitiveValue::Strs(parts) => parts,
            _ => unreachable!(),
        };

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

        Ok(out)
    }
}

impl<S, T, D, BD> StatefulDecode for StatefulDecoder<D, BD, S, DynamicTextCodec>
where
    D: DecodeFrom<T>,
    BD: BasicDecode,
    S: std::ops::DerefMut<Target = T> + Read + Seek,
    T: ?Sized + Read + Seek,
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
        if header.length() == Length(0) {
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
        if header.length() == Length(0) {
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
            | VR::UC
            | VR::CS
            | VR::IS
            | VR::DS
            | VR::DA
            | VR::TM
            | VR::DT => self.read_value_strs(header),
            VR::UI => self.read_value_ui(header),
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
        if header.length() == Length(0) {
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
            _ => Ok(self.from.by_ref().take(
                header
                    .length()
                    .get()
                    .map(u64::from)
                    .unwrap_or(std::u64::MAX),
            )),
        }
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()> {
        self.from.read_exact(buf)?;
        self.bytes_read += buf.len() as u64;
        Ok(())
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
        .length()
        .get()
        .map(|len| len as usize)
        .ok_or_else(|| InvalidValueReadError::UnresolvedValueLength)
}

fn read_until_marker<S, T>(from: &mut S, bytes_to_find: &[u8; 4]) -> Result<Vec<u8>>
where
    S: std::ops::DerefMut<Target = T> + Read + Seek,
    T: ?Sized + Read + Seek,
{
    const READ_SIZE: usize = 1024 * 8;
    let mut buf = [0u8; READ_SIZE];
    let mut out = Vec::new();
    let mut found = false;
    let mut eof = false;

    while !found {
        let mut bytes_read = from.read(&mut buf)?;

        // try to fill the buffer
        while bytes_read < READ_SIZE {
            let bytes_read_here = from.read(&mut buf[bytes_read..])?;
            if bytes_read_here == 0 {
                eof = true;
                break;
            }
            bytes_read += bytes_read_here;
        }

        match find_in_buffer(&buf, &bytes_to_find) {
            Some(i) => {
                found = true;
                // extend up to before delimiter
                out.extend_from_slice(&buf[0..i]);
                // seek reader back to after delimiter
                from.seek(SeekFrom::Current(0 - (bytes_read as i64 - i as i64 - 4)))
                    .unwrap();
                // must have zero bytes after
                check_zero_bytes(from)?;
            }
            None => {
                // check if delimiter crossed READ_SIZE boundary
                // TODO: add a test file that fails without this
                if out.len() > 3 {
                    let mut overlap = out[out.len() - 3..].iter().cloned().collect::<Vec<u8>>();
                    overlap.extend_from_slice(&buf[0..3]);

                    match find_in_buffer(&overlap, &bytes_to_find) {
                        Some(i) => {
                            found = true;
                            // extend up to before delimiter
                            out.extend_from_slice(&buf[0..(3 - i)]);
                            // seek reader back to after delimiter
                            from.seek(SeekFrom::Current(0 - (bytes_read as i64 - (3 - i as i64))))
                                .unwrap();
                            // must have zero bytes after
                            check_zero_bytes(from)?;
                        }
                        None => {}
                    }
                }
                if !found {
                    if eof {
                        return Err(Error::Io(io::Error::from(io::ErrorKind::UnexpectedEof)));
                    }

                    out.extend_from_slice(&buf[0..bytes_read]);
                }
            }
        }
    }

    Ok(out)
}

fn find_in_buffer(buf: &[u8], bytes_to_find: &[u8; 4]) -> Option<usize> {
    buf.windows(4)
        .enumerate()
        .find(|(_, window)| **window == *bytes_to_find)
        .map(|(i, _)| i as usize)
}

fn check_zero_bytes<S, T>(from: &mut S) -> Result<()>
where
    S: std::ops::DerefMut<Target = T> + Read + Seek,
    T: ?Sized + Read + Seek,
{
    let mut len_buf: [u8; 4] = [0; 4];
    from.read_exact(&mut len_buf)?;
    assert_eq!(len_buf, [0, 0, 0, 0]);
    Ok(())
}

fn tag_as_bytes(tag: Tag, endianness: Endianness) -> [u8; 4] {
    let mut writer = ByteOrdered::new(vec![], endianness);
    writer.write_u16(tag.group()).unwrap();
    writer.write_u16(tag.element()).unwrap();
    let mut bytes: [u8; 4] = [0; 4];
    bytes.copy_from_slice(&writer.into_inner()[0..4]);
    bytes
}

#[cfg(test)]
mod tests {
    use super::{StatefulDecode, StatefulDecoder};
    use dicom_core::header::{HasLength, Header, Length};
    use dicom_core::{Tag, VR};
    use dicom_encoding::decode::basic::LittleEndianBasicDecoder;
    use dicom_encoding::text::{DefaultCharacterSetCodec, DynamicTextCodec};
    use dicom_encoding::transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
    use std::io::Cursor;

    // manually crafting some DICOM data elements
    //  Tag: (0002,0002) Media Storage SOP Class UID
    //  VR: UI
    //  Length: 26
    //  Value: "1.2.840.10008.5.1.4.1.1.1\0"
    // --
    //  Tag: (0002,0010) Transfer Syntax UID
    //  VR: UI
    //  Length: 20
    //  Value: "1.2.840.10008.1.2.1\0" == ExplicitVRLittleEndian
    // --
    const RAW: &'static [u8; 62] = &[
        0x02, 0x00, 0x02, 0x00, 0x55, 0x49, 0x1a, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30,
        0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e,
        0x31, 0x2e, 0x31, 0x00, 0x02, 0x00, 0x10, 0x00, 0x55, 0x49, 0x14, 0x00, 0x31, 0x2e, 0x32,
        0x2e, 0x38, 0x34, 0x30, 0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e,
        0x31, 0x00,
    ];

    fn is_stateful_decoder<T>(_: &T)
    where
        T: StatefulDecode,
    {
    }

    #[test]
    fn decode_data_elements() {
        let mut cursor = Cursor::new(&RAW[..]);
        let mut decoder = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            Box::new(DefaultCharacterSetCodec) as DynamicTextCodec,
        );

        is_stateful_decoder(&decoder);

        {
            // read first element
            let elem = decoder.decode_header().expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(26));

            assert_eq!(decoder.bytes_read(), 8);

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Some("1.2.840.10008.5.1.4.1.1.1\0"));

            assert_eq!(decoder.bytes_read(), 8 + 26);
        }
        {
            // read second element
            let elem = decoder.decode_header().expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 16));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(20));

            assert_eq!(decoder.bytes_read(), 8 + 26 + 8);

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Some("1.2.840.10008.1.2.1\0"));

            assert_eq!(decoder.bytes_read(), 8 + 26 + 8 + 20);
        }
    }
}
