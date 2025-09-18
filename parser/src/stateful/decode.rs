//! Module holding a stateful DICOM data decoding abstraction,
//! which also supports text decoding.

use crate::util::n_times;
use dicom_core::dictionary::VirtualVr;
use dicom_core::header::{DataElementHeader, HasLength, Length, SequenceItemHeader, Tag, VR};
use dicom_core::value::deserialize::{
    parse_date_partial, parse_datetime_partial, parse_time_partial,
};
use dicom_core::value::PrimitiveValue;
use dicom_dictionary_std::StandardDataDictionary;
use dicom_encoding::decode::basic::{BasicDecoder, LittleEndianBasicDecoder};
use dicom_encoding::decode::explicit_le::ExplicitVRLittleEndianDecoder;
use dicom_encoding::decode::{BasicDecode, DecodeFrom};
use dicom_encoding::text::{
    validate_da, validate_dt, validate_tm, DefaultCharacterSetCodec, SpecificCharacterSet,
    TextCodec, TextValidationOutcome,
};
use dicom_encoding::transfer_syntax::{DynDecoder, TransferSyntax};
use smallvec::smallvec;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::io::Read;
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

    /// Read the following number of bytes into a vector.
    fn read_to_vec(&mut self, length: u32, vec: &mut Vec<u8>) -> Result<()>;

    /// Read the following number of bytes
    /// as a sequence of unsigned 32 bit integers
    /// into a vector.
    fn read_u32_to_vec(&mut self, length: u32, vec: &mut Vec<u32>) -> Result<()>;

    /// Read the following number of bytes into a generic writer.
    fn read_to<W>(&mut self, length: u32, out: W) -> Result<()>
    where
        Self: Sized,
        W: std::io::Write;

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

/// Defines a special override for
/// how text of certain value representations is decoded.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum CharacterSetOverride {
    /// The standard behavior.
    /// Use the declared character set for
    /// LO, LT, PN, SH, ST, UC, UT.
    /// Use the default character repertoire
    /// for AE, AS, CS.
    #[default]
    None,

    /// Use the declared character set to decode text
    /// for more value representations,
    /// including CS and UR.
    ///
    /// DA, TM, DT, IS, DS, and FD will not be affected by this change.
    AnyVr,
}

/// A stateful abstraction for the full DICOM content reading process.
/// This type encapsulates the necessary codecs in order
/// to be as autonomous as possible in the DICOM content reading
/// process.
/// `S` is the generic parameter type for the original source,
/// `D` is the parameter type that the decoder interprets as,
/// whereas `DB` is the parameter type for the basic decoder.
/// `TC` defines the text codec used underneath.
#[derive(Debug)]
pub struct StatefulDecoder<D, S, BD = BasicDecoder, TC = SpecificCharacterSet> {
    from: S,
    decoder: D,
    basic: BD,
    text: TC,
    charset_override: CharacterSetOverride,
    buffer: Vec<u8>,
    /// the assumed position of the reader source
    position: u64,
    signed_pixeldata: Option<bool>,
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
            .context(UnsupportedTransferSyntaxSnafu { ts: ts.name() })?;

        Ok(StatefulDecoder::new_with_position(
            from, decoder, basic, charset, position,
        ))
    }

    /// Create a new DICOM parser for the given transfer syntax,
    /// character set override, and assumed position of the reader source.
    pub fn new_with_override(
        from: S,
        ts: &TransferSyntax,
        charset: SpecificCharacterSet,
        charset_override: CharacterSetOverride,
        position: u64,
    ) -> Result<Self>
    where
        S: Read,
    {
        let basic = ts.basic_decoder();
        let decoder = ts
            .decoder_for::<S>()
            .context(UnsupportedTransferSyntaxSnafu { ts: ts.name() })?;

        Ok(StatefulDecoder::new_with_all_options(
            from, decoder, basic, charset, charset_override, position,
        ))
    }

    /// Create a new DICOM parser for the given transfer syntax
    /// and assumed position of the reader source.
    ///
    /// The default character set is assumed
    /// until a _Specific Character Set_ attribute is found.
    pub fn new_with_ts(from: S, ts: &TransferSyntax, position: u64) -> Result<Self>
    where
        S: Read,
    {
        Self::new_with(from, ts, SpecificCharacterSet::default(), position)
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
            basic: LittleEndianBasicDecoder,
            decoder: ExplicitVRLittleEndianDecoder::default(),
            text: DefaultCharacterSetCodec,
            charset_override: Default::default(),
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            position: 0,
            signed_pixeldata: None,
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
    /// `position` should be calculated with care.
    /// Decoding or parsing errors may occur
    /// if this position does not match the real position of the reader.
    #[inline]
    pub fn new_with_position(from: S, decoder: D, basic: BD, text: TC, position: u64) -> Self {
        Self::new_with_all_options(from, decoder, basic, text, Default::default(), position)
    }

    #[inline]
    pub(crate) fn new_with_all_options(from: S, decoder: D, basic: BD, text: TC, charset_override: CharacterSetOverride, position: u64) -> Self {
        Self {
            from,
            basic,
            decoder,
            text,
            charset_override,
            buffer: Vec::with_capacity(PARSER_BUFFER_CAPACITY),
            position,
            signed_pixeldata: None,
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
        let position = from.stream_position()?;
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
            .context(UndefinedValueLengthSnafu {
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
                    .context(ReadValueDataSnafu {
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
        self.from.read_exact(&mut buf).context(ReadValueDataSnafu {
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
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;

        let use_charset_declared = match (self.charset_override, header.vr()) {
            (CharacterSetOverride::AnyVr, _) => true,
            (_, VR::AE) | (_, VR::CS) | (_, VR::AS) => false,
            _ => true,
        };

        let parts: Result<_> = if use_charset_declared {
            self
                .buffer
                .split(|v| *v == b'\\')
                .map(|slice| {
                    self.text.decode(slice).context(DecodeTextSnafu {
                        position: self.position,
                    })
                })
                .collect()
        } else {
            self
                .buffer
                .split(|v| *v == b'\\')
                .map(|slice| {
                    DefaultCharacterSetCodec
                        .decode(slice)
                        .context(DecodeTextSnafu {
                            position: self.position,
                        })
                })
                .collect()
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
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::Str(
            self.text
                .decode(&self.buffer[..])
                .context(DecodeTextSnafu {
                    position: self.position,
                })?,
        ))
    }

    fn read_value_ss(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        // sequence of 16-bit signed integers
        let len = self.require_known_length(header)?;

        let n = len >> 1;
        let mut vec = smallvec![0; n];
        self.basic
            .decode_ss_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;

        self.position += len as u64;
        Ok(PrimitiveValue::I16(vec))
    }

    fn read_value_fl(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 32-bit floats
        let n = len >> 2;
        let mut vec = smallvec![0.; n];
        self.basic
            .decode_fl_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::F32(vec))
    }

    fn read_value_da(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of dates

        self.buffer.resize_with(len, Default::default);
        self.from
            .read_exact(&mut self.buffer)
            .context(ReadValueDataSnafu {
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
            return InvalidDateValueSnafu {
                position: self.position,
                string: lossy_str,
            }
            .fail();
        }
        let vec: Result<_> = buf
            .split(|b| *b == b'\\')
            .map(|part| {
                parse_date_partial(part)
                    .map(|t| t.0)
                    .context(DeserializeValueSnafu {
                        position: self.position,
                    })
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
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        let parts: Result<_> = buf
            .split(|b| *b == b'\\')
            .map(|slice| {
                let codec = DefaultCharacterSetCodec;
                let txt = codec.decode(slice).context(DecodeTextSnafu {
                    position: self.position,
                })?;
                let txt = txt.trim();
                txt.parse::<f64>().context(ReadFloatSnafu {
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
            .context(ReadValueDataSnafu {
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
            return InvalidDateTimeValueSnafu {
                position: self.position,
                string: lossy_str,
            }
            .fail();
        }
        let vec: Result<_> = buf
            .split(|b| *b == b'\\')
            .map(|part| {
                parse_datetime_partial(part).context(DeserializeValueSnafu {
                    position: self.position,
                })
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
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        let buf = trim_trail_empty_bytes(&self.buffer);
        if buf.is_empty() {
            return Ok(PrimitiveValue::Empty);
        }

        let parts: Result<_> = buf
            .split(|v| *v == b'\\')
            .map(|slice| {
                let codec = DefaultCharacterSetCodec;
                let txt = codec.decode(slice).context(DecodeTextSnafu {
                    position: self.position,
                })?;
                let txt = txt.trim();
                txt.parse::<i32>().context(ReadIntSnafu {
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
            .context(ReadValueDataSnafu {
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
            return InvalidTimeValueSnafu {
                position: self.position,
                string: lossy_str,
            }
            .fail();
        }
        let vec: std::result::Result<_, _> = buf
            .split(|b| *b == b'\\')
            .map(|part| {
                parse_time_partial(part)
                    .map(|t| t.0)
                    .context(DeserializeValueSnafu {
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
        let mut vec = smallvec![0.; n];
        self.basic
            .decode_fd_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::F64(vec))
    }

    fn read_value_ul(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 32-bit unsigned integers

        let n = len >> 2;
        let mut vec = smallvec![0u32; n];
        self.basic
            .decode_ul_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::U32(vec))
    }

    fn read_u32(&mut self, n: usize, vec: &mut Vec<u32>) -> Result<()> {
        let base = vec.len();
        vec.resize(base + n, 0);

        self.basic
            .decode_ul_into(&mut self.from, &mut vec[base..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += n as u64 * 4;
        Ok(())
    }

    fn read_value_us(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 16-bit unsigned integers

        let n = len >> 1;
        let mut vec = smallvec![0; n];
        self.basic
            .decode_us_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;

        self.position += len as u64;

        if header.tag == Tag(0x0028, 0x0103) {
            //Pixel Representation is not 0, so 2s complement (signed)
            self.signed_pixeldata = vec.first().map(|rep| *rep != 0);
        }

        Ok(PrimitiveValue::U16(vec))
    }

    fn read_value_uv(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 64-bit unsigned integers

        let n = len >> 3;
        let mut vec = smallvec![0; n];
        self.basic
            .decode_uv_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::U64(vec))
    }

    fn read_value_sl(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 32-bit signed integers

        let n = len >> 2;
        let mut vec = smallvec![0; n];
        self.basic
            .decode_sl_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::I32(vec))
    }

    fn read_value_sv(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        let len = self.require_known_length(header)?;
        // sequence of 64-bit signed integers

        let n = len >> 3;
        let mut vec = smallvec![0; n];
        self.basic
            .decode_sv_into(&mut self.from, &mut vec[..])
            .context(ReadValueDataSnafu {
                position: self.position,
            })?;
        self.position += len as u64;
        Ok(PrimitiveValue::I64(vec))
    }
}

impl<S, D, BD> StatefulDecoder<D, S, BD>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
{
    fn set_character_set(&mut self, charset: SpecificCharacterSet) -> Result<()> {
        self.text = charset;
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
                    tracing::warn!("Unsupported character set `{}`, ignoring", name);
                    None
                })
            }) {
                self.set_character_set(charset)?;
            }
        }

        Ok(out)
    }
}

impl<D> StatefulDecode for &'_ mut D
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

    fn read_u32_to_vec(&mut self, length: u32, vec: &mut Vec<u32>) -> Result<()> {
        (**self).read_u32_to_vec(length, vec)
    }

    fn read_to<W>(&mut self, length: u32, out: W) -> Result<()>
    where
        Self: Sized,
        W: std::io::Write,
    {
        (**self).read_to(length, out)
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

impl<D, S, BD> StatefulDecode for StatefulDecoder<D, S, BD>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
{
    type Reader = S;

    fn decode_header(&mut self) -> Result<DataElementHeader> {
        let mut header = self
            .decoder
            .decode_header(&mut self.from)
            .context(DecodeElementHeaderSnafu {
                position: self.position,
            })
            .map(|(header, bytes_read)| {
                self.position += bytes_read as u64;
                header
            })?;

        //If we are decoding the PixelPadding element, make sure the VR is the same as the pixel
        //representation (US by default, SS for signed data).
        if let Some(vr) = self.determine_vr_based_on_pixel_representation(header.tag) {
            header.vr = vr;
        }

        Ok(header)
    }

    fn decode_item_header(&mut self) -> Result<SequenceItemHeader> {
        self.decoder
            .decode_item_header(&mut self.from)
            .context(DecodeItemHeaderSnafu {
                position: self.position,
            })
            .map(|header| {
                self.position += 8;
                header
            })
    }

    fn read_value(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        if header.length() == Length(0) {
            return Ok(PrimitiveValue::Empty);
        }

        match header.vr() {
            VR::SQ => {
                // sequence objects should not head over here, they are
                // handled at a higher level
                NonPrimitiveTypeSnafu {
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
                NonPrimitiveTypeSnafu {
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
                NonPrimitiveTypeSnafu {
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
        self.read_to(length, vec)
    }

    fn read_u32_to_vec(&mut self, length: u32, vec: &mut Vec<u32>) -> Result<()> {
        self.read_u32((length >> 2) as usize, vec)
    }

    fn read_to<W>(&mut self, length: u32, mut out: W) -> Result<()>
    where
        Self: Sized,
        W: std::io::Write,
    {
        let length = u64::from(length);
        std::io::copy(&mut self.from.by_ref().take(length), &mut out).context(
            ReadValueDataSnafu {
                position: self.position,
            },
        )?;
        self.position += length;
        Ok(())
    }

    fn skip_bytes(&mut self, length: u32) -> Result<()> {
        std::io::copy(
            &mut self.from.by_ref().take(u64::from(length)),
            &mut std::io::sink(),
        )
        .context(ReadValueDataSnafu {
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
            .context(SeekReaderSnafu {
                position: self.position,
                new_position: position,
            })
            .map(|_| ())
    }
}

impl<D, S, BD> StatefulDecoder<D, S, BD>
where
    D: DecodeFrom<S>,
    BD: BasicDecode,
    S: Read,
{
    /// The pixel representation affects the VR for several elements.
    /// Returns `Some(VR::SS)` if the vr needs to be modified to SS. Returns `None`
    /// if this element is not affected _or_ if we have Unsigned Pixel Representation.
    fn determine_vr_based_on_pixel_representation(&self, tag: Tag) -> Option<VR> {
        use dicom_core::dictionary::DataDictionary;

        if self.signed_pixeldata == Some(true)
            && StandardDataDictionary.by_tag(tag).map(|e| e.vr) == Some(VirtualVr::Xs)
        {
            Some(VR::SS)
        } else {
            None
        }
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
    use dicom_core::header::{DataElementHeader, HasLength, Header, Length, SequenceItemHeader};
    use dicom_core::{Tag, VR};
    use dicom_encoding::decode::basic::LittleEndianBasicDecoder;
    use dicom_encoding::decode::{
        explicit_le::ExplicitVRLittleEndianDecoder, implicit_le::ImplicitVRLittleEndianDecoder,
    };
    use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
    use std::io::{Cursor, Seek, SeekFrom};

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
    const RAW: &[u8; 62] = &[
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
            SpecificCharacterSet::default(),
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
        const RAW: &[u8; 18] = &[
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
            SpecificCharacterSet::default(),
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

    #[test]
    fn decode_data_elements_with_position() {
        let data = {
            let mut x = vec![0; 128];
            x.extend(RAW);
            x
        };

        // have cursor start 128 bytes ahead
        let mut cursor = Cursor::new(&data[..]);
        cursor.seek(SeekFrom::Start(128)).unwrap();

        let mut decoder = StatefulDecoder::new_with_position(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
            128,
        );

        is_stateful_decoder(&decoder);

        {
            // read first element
            let elem = decoder.decode_header().expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 2));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(26));

            assert_eq!(decoder.position(), 128 + 8);

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Ok("1.2.840.10008.5.1.4.1.1.1\0"));

            assert_eq!(decoder.position(), 128 + 8 + 26);
        }
        {
            // read second element
            let elem = decoder.decode_header().expect("should find an element");
            assert_eq!(elem.tag(), Tag(2, 16));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(20));

            assert_eq!(decoder.position(), 128 + 8 + 26 + 8);

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Ok("1.2.840.10008.1.2.1\0"));

            assert_eq!(decoder.position(), 128 + 8 + 26 + 8 + 20);

            // rolling back to read the last value again
            decoder.seek(128 + 8 + 26 + 8).unwrap();

            // read value
            let value = decoder
                .read_value(&elem)
                .expect("value after element header");
            assert_eq!(value.multiplicity(), 1);
            assert_eq!(value.string(), Ok("1.2.840.10008.1.2.1\0"));

            assert_eq!(decoder.position(), 128 + 8 + 26 + 8 + 20 + 20);
        }
    }

    #[test]
    fn decode_nested_datasets() {
        const RAW: &[u8; 138] = &[
            // 0: (2001, 9000) private sequence
            0x01, 0x20, 0x00, 0x90, //
            // length: undefined
            0xFF, 0xFF, 0xFF, 0xFF, //
            // 8: Item start
            0xFE, 0xFF, 0x00, 0xE0, //
            // Item length explicit (114)
            0x72, 0x00, 0x00, 0x00, //
            // 16: (0008,1115) ReferencedSeriesSequence
            0x08, 0x00, 0x15, 0x11, //
            // length: undefined
            0xFF, 0xFF, 0xFF, 0xFF, //
            // 24: Item start
            0xFE, 0xFF, 0x00, 0xE0, //
            // Item length undefined
            0xFF, 0xFF, 0xFF, 0xFF, //
            // 32: (0008,1140) ReferencedImageSequence
            0x08, 0x00, 0x40, 0x11, //
            // length: undefined
            0xFF, 0xFF, 0xFF, 0xFF, //
            // 40: Item start
            0xFE, 0xFF, 0x00, 0xE0, //
            // Item length undefined
            0xFF, 0xFF, 0xFF, 0xFF, //
            // 48: (0008,1150) ReferencedSOPClassUID
            0x08, 0x00, 0x50, 0x11, //
            // length: 26
            0x1a, 0x00, 0x00, 0x00, //
            // Value: "1.2.840.10008.5.1.4.1.1.7\0" (SecondaryCaptureImageStorage)
            b'1', b'.', b'2', b'.', b'8', b'4', b'0', b'.', b'1', b'0', b'0', b'0', b'8', b'.',
            b'5', b'.', b'1', b'.', b'4', b'.', b'1', b'.', b'1', b'.', b'7', b'\0',
            // 82: Item End (ReferencedImageSequence)
            0xFE, 0xFF, 0x0D, 0xE0, //
            0x00, 0x00, 0x00, 0x00, //
            // 90: Sequence End (ReferencedImageSequence)
            0xFE, 0xFF, 0xDD, 0xE0, //
            0x00, 0x00, 0x00, 0x00, //
            // 98: Item End (ReferencedSeriesSequence)
            0xFE, 0xFF, 0x0D, 0xE0, //
            0x00, 0x00, 0x00, 0x00, //
            // 106: Sequence End (ReferencedSeriesSequence)
            0xFE, 0xFF, 0xDD, 0xE0, //
            0x00, 0x00, 0x00, 0x00, //
            // 114: (2050,0020) PresentationLUTShape (CS)
            0x50, 0x20, 0x20, 0x00, //
            // length: 8
            0x08, 0x00, 0x00, 0x00, //
            b'I', b'D', b'E', b'N', b'T', b'I', b'T', b'Y', // 130: Sequence end
            0xFE, 0xFF, 0xDD, 0xE0, //
            0x00, 0x00, 0x00, 0x00, //
        ];

        let mut cursor = &RAW[..];
        let mut decoder = StatefulDecoder::new(
            &mut cursor,
            ImplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );

        is_stateful_decoder(&decoder);

        let header = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(header.tag(), Tag(0x2001, 0x9000));
        assert_eq!(header.vr(), VR::UN);
        assert!(header.length().is_undefined());

        assert_eq!(decoder.position(), 8);

        let item_header = decoder
            .decode_item_header()
            .expect("should find an item header");
        assert_eq!(item_header, SequenceItemHeader::Item { len: Length(114) });

        assert_eq!(decoder.position(), 16);

        // enter ReferencedSeriesSequence
        let header = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(header.tag(), Tag(0x0008, 0x1115));
        assert_eq!(header.vr(), VR::SQ);
        assert!(header.length().is_undefined());

        assert_eq!(decoder.position(), 24);

        let item_header = decoder
            .decode_item_header()
            .expect("should find an item header (ReferencedSeriesSequence)");
        assert!(matches!(
            item_header,
            SequenceItemHeader::Item {
                len,
            } if len.is_undefined()
        ));

        assert_eq!(decoder.position(), 32);

        // enter ReferencedImageSequence
        let header = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(header.tag(), Tag(0x0008, 0x1140));
        assert_eq!(header.vr(), VR::SQ);
        assert!(header.length().is_undefined());

        assert_eq!(decoder.position(), 40);

        let item_header = decoder
            .decode_item_header()
            .expect("should find an item header (ReferencedImageSequence)");
        assert!(matches!(
            item_header,
            SequenceItemHeader::Item {
                len,
            } if len.is_undefined()
        ));

        assert_eq!(decoder.position(), 48);

        // read ReferencedSOPClassUID

        let header = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(header.tag(), Tag(0x0008, 0x1150));
        assert_eq!(header.vr(), VR::UI);
        assert_eq!(header.length(), Length(26));

        assert_eq!(decoder.position(), 56);

        let value = decoder
            .read_value(&header)
            .expect("should find a value after element header");
        assert_eq!(value.to_str(), "1.2.840.10008.5.1.4.1.1.7");

        assert_eq!(decoder.position(), 82);

        // exit ReferencedImageSequence

        let item_header = decoder
            .decode_item_header()
            .expect("should find item delimiter");
        assert_eq!(item_header, SequenceItemHeader::ItemDelimiter);

        assert_eq!(decoder.position(), 90);

        let item_header = decoder
            .decode_item_header()
            .expect("should find sequence delimiter");
        assert_eq!(item_header, SequenceItemHeader::SequenceDelimiter);

        assert_eq!(decoder.position(), 98);

        // exit ReferencedSeriesSequence

        let item_header = decoder
            .decode_item_header()
            .expect("should find item delimiter");
        assert_eq!(item_header, SequenceItemHeader::ItemDelimiter);

        assert_eq!(decoder.position(), 106);

        let item_header = decoder
            .decode_item_header()
            .expect("should find sequence delimiter");
        assert_eq!(item_header, SequenceItemHeader::SequenceDelimiter);

        assert_eq!(decoder.position(), 114);

        // read PresentationLUTShape

        let header = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(header.tag(), Tag(0x2050, 0x0020));
        assert_eq!(header.vr(), VR::CS);
        assert_eq!(header.length(), Length(8));

        assert_eq!(decoder.position(), 122);

        let value = decoder
            .read_value(&header)
            .expect("value after element header");
        assert_eq!(value.multiplicity(), 1);
        assert_eq!(value.to_str(), "IDENTITY");

        assert_eq!(decoder.position(), 130);

        // exit private sequence
        // (no item delimiter because length is explicit)

        let item_header = decoder
            .decode_item_header()
            .expect("should find an item header");
        assert_eq!(item_header, SequenceItemHeader::SequenceDelimiter);

        assert_eq!(decoder.position(), 138);
    }

    #[test]
    fn decode_and_use_pixel_representation() {
        const RAW: &[u8; 20] = &[
            0x28, 0x00, 0x03, 0x01, // Tag: (0023,0103) PixelRepresentation
            0x02, 0x00, 0x00, 0x00, // Length: 2
            0x01, 0x00, // Value: "1",
            0x28, 0x00, 0x20, 0x01, // Tag: (0023,0120) PixelPadding
            0x02, 0x00, 0x00, 0x00, // Length: 2
            0x01, 0x00, // Value: "1"
        ];

        let mut cursor = &RAW[..];
        let mut decoder = StatefulDecoder::new(
            &mut cursor,
            ImplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );

        is_stateful_decoder(&decoder);

        let header_1 = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(
            header_1,
            DataElementHeader {
                tag: Tag(0x0028, 0x0103),
                vr: VR::US,
                len: Length(2),
            }
        );

        decoder
            .read_value(&header_1)
            .expect("Can read Pixel Representation");

        let header_2 = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(
            header_2,
            DataElementHeader {
                tag: Tag(0x0028, 0x0120),
                vr: VR::SS,
                len: Length(2),
            }
        );
    }

    #[test]
    fn decode_text_with_charset_override() {
        #[rustfmt::skip]
        const RAW: &[u8; 28] = &[
            // Tag: (0018,0015) Body Part Examined
            0x18, 0x00, 0x15, 0x00,
            // VR: CS
            b'C', b'S',
            // Length: 20
            0x14, 0x00, // Value: "脊柱侧弯-视图" (scoliosis-view)
            232, 132, 138, 230, 159, 177, 228, 190,
            167, 229, 188, 175, 45, 232, 167, 134,
            229, 155, 190, b' '
        ];

        let mut cursor = &RAW[..];

        let mut decoder = StatefulDecoder::new_with_all_options(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::ISO_IR_192,
            // use AnyVr so that Body Part Examined
            super::CharacterSetOverride::AnyVr,
            0,
        );

        is_stateful_decoder(&decoder);

        let header_1 = decoder
            .decode_header()
            .expect("should find an element header");
        assert_eq!(
            header_1,
            DataElementHeader {
                tag: Tag(0x0018, 0x0015),
                vr: VR::CS,
                len: Length(20),
            }
        );

        let val = decoder
            .read_value(&header_1)
            .expect("Can read Body Part Examined");

        assert_eq!(
            val.to_str(),
            "脊柱侧弯-视图",
        );
    }
}
