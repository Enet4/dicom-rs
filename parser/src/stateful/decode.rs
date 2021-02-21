//! Module holding a stateful DICOM data decoding abstraction,
//! which also supports text decoding.

use crate::util::n_times;
use chrono::FixedOffset;
use dicom_core::header::{DataElementHeader, HasLength, Length, SequenceItemHeader, Tag, VR};
use dicom_core::value::PrimitiveValue;
use dicom_encoding::decode::basic::{BasicDecoder, LittleEndianBasicDecoder};
use dicom_encoding::decode::primitive_value::*;
use dicom_encoding::decode::{BasicDecode, DecodeFrom};
use dicom_encoding::text::{
    validate_da, validate_dt, validate_tm, DefaultCharacterSetCodec, DynamicTextCodec,
    SpecificCharacterSet, TextCodec, TextValidationOutcome,
};
use dicom_encoding::transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
use dicom_encoding::transfer_syntax::{DynDecoder, TransferSyntax};
use smallvec::smallvec;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::io::Read;
use std::iter::Iterator;
use std::{fmt::Debug, io::Seek, io::SeekFrom};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Decoding in transfer syntax {} is unsupported", ts))]
    UnsupportedTransferSyntax {
        ts: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Unsupported character set {:?}", charset))]
    UnsupportedCharacterSet {
        charset: SpecificCharacterSet,
        backtrace: Backtrace,
    },

    #[snafu(display("Attempted to read non-primitive value at position {}", position))]
    NonPrimitiveType { position: u64, backtrace: Backtrace },

    #[snafu(display(
        "Undefined value length of element tagged {} at position {}",
        tag,
        position
    ))]
    UndefinedValueLength {
        tag: Tag,
        position: u64,
        backtrace: Backtrace,
    },

    #[snafu(display("Could not decode element header at position {}", position))]
    DecodeElementHeader {
        position: u64,
        #[snafu(backtrace)]
        source: dicom_encoding::decode::Error,
    },

    #[snafu(display("Could not decode element header at position {}", position))]
    DecodeItemHeader {
        position: u64,
        #[snafu(backtrace)]
        source: dicom_encoding::decode::Error,
    },

    #[snafu(display("Could not decode text at position {}", position))]
    DecodeText {
        position: u64,
        #[snafu(backtrace)]
        source: dicom_encoding::text::DecodeTextError,
    },

    #[snafu(display("Could not read value from source at position {}", position))]
    ReadValueData {
        position: u64,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Could not move source cursor from position {} to {}",
        position,
        new_position
    ))]
    SeekReader {
        position: u64,
        new_position: u64,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed value deserialization at position {}", position))]
    DeserializeValue {
        position: u64,
        source: dicom_core::value::deserialize::Error,
    },

    #[snafu(display("Invalid integer value at position {}", position))]
    ReadInt {
        position: u64,
        source: std::num::ParseIntError,
    },

    #[snafu(display("Invalid float value at position {}", position))]
    ReadFloat {
        position: u64,
        source: std::num::ParseFloatError,
    },

    #[snafu(display("Invalid Date value element `{}` at position {}", string, position))]
    InvalidDateValue {
        position: u64,
        string: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid Time value element `{}` at position {}", string, position))]
    InvalidTimeValue {
        position: u64,
        string: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid DateTime value element `{}` at position {}", string, position))]
    InvalidDateTimeValue {
        position: u64,
        string: String,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

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

    /// Read the following bytes into a vector.
    fn read_to_vec(&mut self, length: u32, vec: &mut Vec<u8>) -> Result<()>;

    /// Skip the following bytes into a vector,
    /// counting them as if they were read.
    fn skip_bytes(&mut self, length: u32) -> Result<()>;

    /// Reposition the reader so that it starts reading
    /// at the reader's given position.
    ///
    /// The number of bytes read is not expected to be modified.
    fn seek(&mut self, position: u64) -> Result<()>
    where
        Self::Reader: Seek;

    /// Retrieve the known position of the inner reader source.
    /// If the stateful decoder was constructed at the beginning of the reader,
    /// this equals to the number of bytes read so far.
    fn position(&self) -> u64;
}

/// Alias for a dynamically resolved DICOM stateful decoder. Although the data
/// source may be known at compile time, the required decoder may vary
/// according to an object's transfer syntax.
pub type DynStatefulDecoder<S> = StatefulDecoder<DynDecoder<S>, S>;

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
pub struct StatefulDecoder<D, S, BD = BasicDecoder, TC = DynamicTextCodec> {
    from: S,
    decoder: D,
    basic: BD,
    text: TC,
    dt_utc_offset: FixedOffset,
    buffer: Vec<u8>,
    /// the assumed position of the reader source
    position: u64,
}

impl<S> StatefulDecoder<DynDecoder<S>, S> {
    /// Create a new DICOM parser for the given transfer syntax, character set,
    /// and assumed position of the reader source.
    pub fn new_with(
        from: S,
        ts: &TransferSyntax,
        charset: SpecificCharacterSet,
        position: u64,
    ) -> Result<Self>
    where
        S: Read,
    {
        let basic = ts.basic_decoder();
        let decoder = ts
            .decoder_for::<S>()
            .context(UnsupportedTransferSyntax { ts: ts.name() })?;
        let text = charset
            .codec()
            .context(UnsupportedCharacterSet { charset })?;

        Ok(StatefulDecoder::new_with_position(
            from, decoder, basic, text, position,
        ))
    }
}

/// Type alias for the DICOM parser of a file's Meta group.
pub type FileHeaderParser<S> = StatefulDecoder<
    ExplicitVRLittleEndianDecoder,
    S,
    LittleEndianBasicDecoder,
    DefaultCharacterSetCodec,
>;

impl<S> FileHeaderParser<S>
where
    S: Read,
{
    /// Create a new DICOM stateful decoder for reading the file meta header,
    /// which is always in _Explicit VR Little Endian_.
    pub fn file_header_parser(from: S) -> Self {
        Self {
            from,
            basic: LittleEndianBasicDecoder::default(),
            decoder: ExplicitVRLittleEndianDecoder::default(),
            text: DefaultCharacterSetCodec,
            dt_utc_offset: FixedOffset::east(0),
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            position: 0,
        }
    }
}

impl<D, S, BD, TC> StatefulDecoder<D, S, BD, TC>
where
    BD: BasicDecode,
    TC: TextCodec,
{
    /// Create a new DICOM stateful decoder from its parts.
    #[inline]
    pub fn new(from: S, decoder: D, basic: BD, text: TC) -> StatefulDecoder<D, S, BD, TC> {
        Self::new_with_position(from, decoder, basic, text, 0)
    }

    /// Create a new DICOM stateful decoder from its parts,
    /// while assuming a base reading position.
    ///
    /// `position` should be calculatd with care.
    /// Decoding or parsing errors may occur
    /// if this position does not match the real position of the reader.
    #[inline]
    pub fn new_with_position(
        from: S,
        decoder: D,
        basic: BD,
        text: TC,
        position: u64,
    ) -> Self {
        Self {
            from,
            basic,
            decoder,
            text,
            dt_utc_offset: FixedOffset::east(0),
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            position,
        }
    }
}

impl<D, S, BD, TC> StatefulDecoder<D, S, BD, TC>
where
    S: Seek,
    BD: BasicDecode,
    TC: TextCodec,
{
    /// Create a new DICOM stateful decoder from its parts,
    /// while determining the data source's current position via `seek`.
    pub fn new_positioned(
        mut from: S,
        decoder: D,
        basic: BD,
        text: TC,
    ) -> Result<Self, std::io::Error> {
        let position = from.seek(SeekFrom::Current(0))?;
        Ok(Self::new_with_position(
            from, decoder, basic, text, position,
        ))
    }
}

impl<D, S, BD, TC> StatefulDecoder<D, S, BD, TC>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
    TC: TextCodec,
{
    // ---------------- private methods ---------------------

    fn require_known_length(&self, header: &DataElementHeader) -> Result<usize> {
        header
            .length()
            .get()
            .map(|len| len as usize)
            .context(UndefinedValueLength {
                position: self.position,
                tag: header.tag,
            })
    }

    fn read_value_tag(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;

        // tags
        let ntags = len >> 2;
        let parts: Result<_> = n_times(ntags)
            .map(|_| {
                self.basic
                    .decode_tag(&mut self.from)
                    .context(ReadValueData {
                        position: self.position,
                    })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::Tags(parts?))
    }

    fn read_value_ob(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        // Note: this function always expects a defined length OB value
        // (pixel sequence detection needs to be done by the caller)
        let len = self.require_known_length(header)?;

        // sequence of 8-bit integers (or arbitrary byte data)
        let mut buf = smallvec![0u8; len];
        self.from.read_exact(&mut buf).context(ReadValueData {
            position: self.position,
        })?;
        self.position += len as u64;
        Ok(PrimitiveValue::U8(buf))
    }

    fn read_value_strs(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of strings
        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueData {
                position: self.position,
            })?;

        let parts: Result<_> = match header.vr() {
            VR::AE | VR::CS | VR::AS => self
                .buffer
                .split(|v| *v == b'\\')
                .map(|slice| {
                    DefaultCharacterSetCodec.decode(slice).context(DecodeText {
                        position: self.position,
                    })
                })
                .collect(),
            _ => self
                .buffer
                .split(|v| *v == b'\\')
                .map(|slice| {
                    self.text.decode(slice).context(DecodeText {
                        position: self.position,
                    })
                })
                .collect(),
        };

        self.position += len as u64;
        Ok(PrimitiveValue::Strs(parts?))
    }

    fn read_value_str(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;

        // a single string
        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueData {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::Str(
            self.text.decode(&self.buffer[..]).context(DecodeText {
                position: self.position,
            })?,
        ))
    }

    fn read_value_ss(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        // sequence of 16-bit signed integers
        let len = self.require_known_length(header)?;

        let n = len >> 1;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_ss(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::I16(vec?))
    }

    fn read_value_fl(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 32-bit floats
        let n = len >> 2;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_fl(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::F32(vec?))
    }

    fn read_value_da(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of dates

        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueData {
                position: self.position,
            })?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        if validate_da(buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return InvalidDateValue {
                position: self.position,
                string: lossy_str,
            }
            .fail();
        }
        let vec: Result<_> = buf
            .split(|b| *b == b'\\')
            .map(|part| {
                Ok(parse_date(part)
                    .context(DeserializeValue {
                        position: self.position,
                    })?
                    .0)
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::Date(vec?))
    }

    fn read_value_ds(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of doubles in text form

        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueData {
                position: self.position,
            })?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        let parts: Result<_> = buf
            .split(|b| *b == b'\\')
            .map(|slice| {
                let codec = SpecificCharacterSet::Default.codec().unwrap();
                let txt = codec.decode(slice).context(DecodeText {
                    position: self.position,
                })?;
                let txt = txt.trim();
                txt.parse::<f64>().context(ReadFloat {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::F64(parts?))
    }

    fn read_value_dt(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of datetimes

        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueData {
                position: self.position,
            })?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        if validate_dt(buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return InvalidDateTimeValue {
                position: self.position,
                string: lossy_str,
            }
            .fail();
        }
        let vec: Result<_> = buf
            .split(|b| *b == b'\\')
            .map(|part| {
                Ok(
                    parse_datetime(part, self.dt_utc_offset).context(DeserializeValue {
                        position: self.position,
                    })?,
                )
            })
            .collect();

        self.position += len as u64;
        Ok(PrimitiveValue::DateTime(vec?))
    }

    fn read_value_is(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of signed integers in text form
        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueData {
                position: self.position,
            })?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        let parts: Result<_> = buf
            .split(|v| *v == b'\\')
            .map(|slice| {
                let codec = SpecificCharacterSet::Default.codec().unwrap();
                let txt = codec.decode(slice).context(DecodeText {
                    position: self.position,
                })?;
                let txt = txt.trim();
                txt.parse::<i32>().context(ReadInt {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::I32(parts?))
    }

    fn read_value_tm(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of time instances

        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueData {
                position: self.position,
            })?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        if validate_tm(buf) != TextValidationOutcome::Ok {
            let lossy_str = DefaultCharacterSetCodec
                .decode(buf)
                .unwrap_or_else(|_| "[byte stream]".to_string());
            return InvalidTimeValue {
                position: self.position,
                string: lossy_str,
            }
            .fail();
        }
        let vec: std::result::Result<_, _> = buf
            .split(|b| *b == b'\\')
            .map(|part| {
                parse_time(part).map(|t| t.0).context(DeserializeValue {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::Time(vec?))
    }

    fn read_value_od(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 64-bit floats
        let n = len >> 3;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_fd(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::F64(vec?))
    }

    fn read_value_ul(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 32-bit unsigned integers

        let n = len >> 2;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_ul(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::U32(vec?))
    }

    fn read_value_us(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 16-bit unsigned integers

        let n = len >> 1;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_us(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::U16(vec?))
    }

    fn read_value_uv(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 64-bit unsigned integers

        let n = len >> 3;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_uv(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::U64(vec?))
    }

    fn read_value_sl(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 32-bit signed integers

        let n = len >> 2;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_sl(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::I32(vec?))
    }

    fn read_value_sv(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 64-bit signed integers

        let n = len >> 3;
        let vec: Result<_> = n_times(n)
            .map(|_| {
                self.basic.decode_sv(&mut self.from).context(ReadValueData {
                    position: self.position,
                })
            })
            .collect();
        self.position += len as u64;
        Ok(PrimitiveValue::I64(vec?))
    }
}

impl<S, D, BD> StatefulDecoder<D, S, BD, DynamicTextCodec>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
{
    fn set_character_set(&mut self, charset: SpecificCharacterSet) -> Result<()> {
        self.text = charset
            .codec()
            .context(UnsupportedCharacterSet { charset })?;
        Ok(())
    }

    /// Read a sequence of Code String values. Similar to `read_value_strs`, but also
    /// triggers a character set change when it finds the _SpecificCharacterSet_
    /// attribute.
    fn read_value_cs(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let out = self.read_value_strs(header)?;

        let parts = match &out {
            PrimitiveValue::Strs(parts) => parts,
            _ => unreachable!(),
        };

        // if it's a Specific Character Set, update the decoder immediately.
        if header.tag == Tag(0x0008, 0x0005) {
            // Edge case handling strategies for
            // unsupported specific character sets should probably be considered
            // in the future. See #40 for discussion.
            if let Some(charset) = parts.first().map(|x| x.as_ref()).and_then(|name| {
                SpecificCharacterSet::from_code(name).or_else(|| {
                    // TODO(#49) log this as a warning
                    eprintln!("Unsupported character set `{}`, ignoring", name);
                    None
                })
            }) {
                self.set_character_set(charset)?;
            }
        }

        Ok(out)
    }
}

impl<'a, D> StatefulDecode for &'a mut D
where
    D: StatefulDecode,
{
    type Reader = D::Reader;

    fn decode_header(&mut self) -> Result<DataElementHeader> {
        (**self).decode_header()
    }

    fn decode_item_header(&mut self) -> Result<SequenceItemHeader> {
        (**self).decode_item_header()
    }

    fn read_value(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        (**self).read_value(header)
    }

    fn read_value_preserved(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        (**self).read_value_preserved(header)
    }

    fn read_value_bytes(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        (**self).read_value_bytes(header)
    }

    fn read_to_vec(&mut self, length: u32, vec: &mut Vec<u8>) -> Result<()> {
        (**self).read_to_vec(length, vec)
    }

    fn skip_bytes(&mut self, length: u32) -> Result<()> {
        (**self).skip_bytes(length)
    }

    fn position(&self) -> u64 {
        (**self).position()
    }

    fn seek(&mut self, position: u64) -> Result<()>
    where
        Self::Reader: Seek,
    {
        (**self).seek(position)
    }
}

impl<D, S, BD> StatefulDecode for StatefulDecoder<D, S, BD, DynamicTextCodec>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
{
    type Reader = S;

    fn decode_header(&mut self) -> Result<DataElementHeader> {
        self.decoder
            .decode_header(&mut self.from)
            .context(DecodeElementHeader {
                position: self.position,
            })
            .map(|(header, bytes_read)| {
                self.position += bytes_read as u64;
                header
            })
            .map_err(From::from)
    }

    fn decode_item_header(&mut self) -> Result<SequenceItemHeader> {
        self.decoder
            .decode_item_header(&mut self.from)
            .context(DecodeItemHeader {
                position: self.position,
            })
            .map(|header| {
                self.position += 8;
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
                NonPrimitiveType {
                    position: self.position,
                }
                .fail()
            }
            VR::AT => self.read_value_tag(header),
            VR::AE | VR::AS | VR::PN | VR::SH | VR::LO | VR::UC | VR::UI => {
                self.read_value_strs(header)
            }
            VR::CS => self.read_value_cs(header),
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
                NonPrimitiveType {
                    position: self.position,
                }
                .fail()
            }
            VR::AT => self.read_value_tag(header),
            VR::AE
            | VR::AS
            | VR::PN
            | VR::SH
            | VR::LO
            | VR::UC
            | VR::UI
            | VR::IS
            | VR::DS
            | VR::DA
            | VR::TM
            | VR::DT => self.read_value_strs(header),
            VR::CS => self.read_value_cs(header),
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
                NonPrimitiveType {
                    position: self.position,
                }
                .fail()
            }
            _ => self.read_value_ob(header),
        }
    }

    fn position(&self) -> u64 {
        self.position
    }

    fn read_to_vec(&mut self, length: u32, vec: &mut Vec<u8>) -> Result<()> {
        let length = u64::from(length);
        self.from
            .by_ref()
            .take(u64::from(length))
            .read_to_end(vec)
            .context(ReadValueData {
                position: self.position,
            })?;
        self.position += length;
        Ok(())
    }

    fn skip_bytes(&mut self, length: u32) -> Result<()> {
        std::io::copy(
            &mut self.from.by_ref().take(u64::from(length)),
            &mut std::io::sink(),
        )
        .context(ReadValueData {
            position: self.position,
        })?;

        self.position += u64::from(length);
        Ok(())
    }

    fn seek(&mut self, position: u64) -> Result<()>
    where
        Self::Reader: Seek,
    {
        self.from
            .seek(SeekFrom::Start(position))
            .context(SeekReader {
                position: self.position,
                new_position: position,
            })
            .map(|_| ())
    }
}

/// Remove trailing spaces and null characters.
fn trim_trail_empty_bytes(mut x: &[u8]) -> &[u8] {
    while x.last() == Some(&b' ') || x.last() == Some(&b'\0') {
        x = &x[..x.len() - 1];
    }
    x
}

#[cfg(test)]
mod tests {
    use super::{StatefulDecode, StatefulDecoder};
    use dicom_core::header::{DataElementHeader, HasLength, Header, Length};
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

            assert_eq!(decoder.position(), 8);

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Ok("1.2.840.10008.5.1.4.1.1.1\0"));

            assert_eq!(decoder.position(), 8 + 26);
        }
        {
            // read second element
            let elem = decoder.decode_header().expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 16));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(20));

            assert_eq!(decoder.position(), 8 + 26 + 8);

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Ok("1.2.840.10008.1.2.1\0"));

            assert_eq!(decoder.position(), 8 + 26 + 8 + 20);

            // rolling back to read the last value again
            decoder.seek(8 + 26 + 8).unwrap();

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Ok("1.2.840.10008.1.2.1\0"));

            assert_eq!(decoder.position(), 8 + 26 + 8 + 20 + 20);
        }
    }

    /// Test that the stateful decoder updates
    /// the active character set after reaching a Specific Character Set element
    /// with a supported text encoding.
    #[test]
    fn update_character_set() {
        const RAW: &'static [u8; 18] = &[
            // Tag: (0008,0005) Specific Character Set
            0x08, 0x00, 0x05, 0x00, // VR: CS
            b'C', b'S', // Length: 10
            0x0a, 0x00, // Value: "ISO_IR 192"
            b'I', b'S', b'O', b'_', b'I', b'R', b' ', b'1', b'9', b'2',
        ];

        let mut cursor = &RAW[..];
        let mut decoder = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            Box::new(DefaultCharacterSetCodec) as DynamicTextCodec,
        );

        is_stateful_decoder(&decoder);

        let header = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(
            header,
            DataElementHeader {
                tag: Tag(0x0008, 0x0005),
                vr: VR::CS,
                len: Length(10),
            }
        );

        let value = decoder
            .read_value_preserved(&header)
            .expect("should read a value");

        assert_eq!(value.string(), Ok("ISO_IR 192"));
        assert_eq!(decoder.text.name(), "ISO_IR 192",);
    }
}
