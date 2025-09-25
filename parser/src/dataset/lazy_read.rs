//! This module contains a mid-level abstraction for reading DICOM content
//! sequentially and in a lazy fashion.
//! That is, unlike the reader in the [`read`](super::read) module,
//! DICOM values can be skipped and most allocations can be avoided.
//!
//! At this level, headers and values are treated as tokens which can be used
//! to form a syntax tree of a full data set.
//! Whenever an element value or pixel sequence item is encountered,
//! the given token does not consume the value from the reader,
//! thus letting users decide whether to:
//! - fully read the value and turn it into an in-memory representation;
//! - skip the value altogether, by reading into a sink;
//! - copying the bytes of the value into another writer,
//!   such as a previously allocated buffer.
use crate::dataset::read::OddLengthStrategy;
use crate::stateful::decode::{CharacterSetOverride, DynStatefulDecoder, Error as DecoderError, StatefulDecode};
use crate::util::ReadSeek;
use dicom_core::header::{DataElementHeader, Header, Length, SequenceItemHeader};
use dicom_core::{Tag, VR};
use dicom_encoding::text::SpecificCharacterSet;
use dicom_encoding::transfer_syntax::TransferSyntax;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::cmp::Ordering;

use super::{DataToken, LazyDataToken, SeqTokenType};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Could not create decoder"))]
    CreateDecoder {
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Could not read item header at {} bytes", bytes_read))]
    ReadItemHeader {
        bytes_read: u64,
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Could not read element header at {} bytes", bytes_read))]
    ReadHeader {
        bytes_read: u64,
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Could not read value"))]
    ReadValue {
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Failed to get reader position"))]
    GetPosition {
        source: std::io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display(
        "Inconsistent sequence end: expected end at {} bytes but read {}",
        end_of_sequence,
        bytes_read
    ))]
    InconsistentSequenceEnd {
        end_of_sequence: u64,
        bytes_read: u64,
        backtrace: Backtrace,
    },
    #[snafu(display("Unexpected item delimiter at {} bytes", bytes_read))]
    UnexpectedItemDelimiter {
        bytes_read: u64,
        backtrace: Backtrace,
    },
    #[snafu(display("Unexpected undefined value length at {} bytes", bytes_read))]
    UndefinedLength {
        bytes_read: u64,
        backtrace: Backtrace,
    },

    /// Invalid data element length {len:04X} of {tag} at {bytes_read:#x}
    InvalidElementLength {
        tag: Tag,
        len: u32,
        bytes_read: u64,
        backtrace: Backtrace,
    },

    /// Invalid sequence item length {len:04X} at {bytes_read:#x}
    InvalidItemLength {
        len: u32,
        bytes_read: u64,
        backtrace: Backtrace,
    },

    #[snafu(display("Attempted to inspect a header at {} bytes", bytes_read))]
    Peek {
        bytes_read: u64,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A reader-specific token representing a sequence or item start.
#[derive(Debug, Copy, Clone, PartialEq)]
struct SeqToken {
    /// Whether it is the start of a sequence or the start of an item.
    typ: SeqTokenType,
    /// The length of the value, as indicated by the starting element,
    /// can be unknown.
    len: Length,
    /// Whether this sequence token is part of an encapsulated pixel data.
    pixel_data: bool,
    /// The number of bytes the parser has read until it reached the
    /// beginning of the sequence or item value data.
    base_offset: u64,
}

/// An attached iterator for retrieving DICOM object element markers
/// from a random access data source.
///
/// This iterator produces data tokens without eagerly reading the bytes
/// of a value.
#[derive(Debug)]
pub struct LazyDataSetReader<S> {
    /// the stateful decoder
    parser: S,
    /// data set reading options
    options: LazyDataSetReaderOptions,
    /// whether the reader is expecting an item next (or a sequence delimiter)
    in_sequence: bool,
    /// whether a check for a sequence or item delimitation is pending
    delimiter_check_pending: bool,
    /// a stack of delimiters
    seq_delimiters: Vec<SeqToken>,
    /// fuse the iteration process if true
    hard_break: bool,
    /// last decoded header
    last_header: Option<DataElementHeader>,
    /// if a peek was taken, this holds the token peeked
    peek: Option<DataToken>,
}

/// The set of options for the lazy data set reader.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct LazyDataSetReaderOptions {
    /// The strategy for handling odd length data elements
    pub odd_length: OddLengthStrategy,

    /// Override for how text is decoded
    pub charset_override: CharacterSetOverride,
}

impl<R> LazyDataSetReader<DynStatefulDecoder<R>> {
    /// Create a new lazy data set reader
    /// expecting the given transfer syntax
    /// that reads from the given random access source.
    #[inline]
    pub fn new_with_ts(source: R, ts: &TransferSyntax) -> Result<Self>
    where
        R: ReadSeek,
    {
        Self::new_with_ts_cs(source, ts, SpecificCharacterSet::default())
    }

    /// Create a new lazy data set reader
    /// with the given random access source and element dictionary,
    /// while considering the given transfer syntax and specific character set.
    #[inline]
    pub fn new_with_ts_cs(source: R, ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self>
    where
        R: ReadSeek,
    {
        Self::new_with_ts_cs_options(source, ts, cs, Default::default())
    }

    /// Create a new lazy data set reader
    /// expecting the given transfer syntax
    /// that reads from the given random access source,
    /// with extra parsing options.
    #[inline]
    pub fn new_with_ts_options(
        source: R,
        ts: &TransferSyntax,
        options: LazyDataSetReaderOptions,
    ) -> Result<Self>
    where
        R: ReadSeek,
    {
        Self::new_with_ts_cs_options(source, ts, SpecificCharacterSet::default(), options)
    }

    /// Create a new lazy data set reader
    /// with the given random access source and element dictionary,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with_ts_cs_options(
        mut source: R,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
        options: LazyDataSetReaderOptions,
    ) -> Result<Self>
    where
        R: ReadSeek,
    {
        let position = source.stream_position().context(GetPositionSnafu)?;
        let parser =
            DynStatefulDecoder::new_with_override(source, ts, cs, options.charset_override, position).context(CreateDecoderSnafu)?;

        Ok(LazyDataSetReader {
            parser,
            options,
            seq_delimiters: Vec::new(),
            delimiter_check_pending: false,
            in_sequence: false,
            hard_break: false,
            last_header: None,
            peek: None,
        })
    }
}

impl<S> LazyDataSetReader<S>
where
    S: StatefulDecode,
{
    /// Create a new iterator with the given stateful decoder.
    #[inline]
    pub fn new(parser: S) -> Self {
        LazyDataSetReader::new_with_options(parser, Default::default())
    }

    /// Create a new lazy data set reader
    /// using the given stateful decoder,
    /// with extra parsing options.
    pub fn new_with_options(parser: S, options: LazyDataSetReaderOptions) -> Self
    where
        S: StatefulDecode,
    {
        LazyDataSetReader {
            parser,
            options,
            seq_delimiters: Vec::new(),
            delimiter_check_pending: false,
            in_sequence: false,
            hard_break: false,
            last_header: None,
            peek: None,
        }
    }
}

impl<S> LazyDataSetReader<S>
where
    S: StatefulDecode,
{
    fn update_seq_delimiters<'b>(&mut self) -> Result<Option<LazyDataToken<&'b mut S>>> {
        if let Some(sd) = self.seq_delimiters.last() {
            if let Some(len) = sd.len.get() {
                let end_of_sequence = sd.base_offset + len as u64;
                let bytes_read = self.parser.position();
                match end_of_sequence.cmp(&bytes_read) {
                    Ordering::Equal => {
                        // end of delimiter, as indicated by the element's length
                        let token;
                        match sd.typ {
                            SeqTokenType::Sequence => {
                                self.in_sequence = false;
                                token = LazyDataToken::SequenceEnd;
                            }
                            SeqTokenType::Item => {
                                self.in_sequence = true;
                                token = LazyDataToken::ItemEnd;
                            }
                        }
                        self.seq_delimiters.pop();
                        return Ok(Some(token));
                    }
                    Ordering::Less => {
                        return InconsistentSequenceEndSnafu {
                            end_of_sequence,
                            bytes_read,
                        }
                        .fail();
                    }
                    Ordering::Greater => {} // continue normally
                }
            }
        }
        self.delimiter_check_pending = false;
        Ok(None)
    }

    #[inline]
    fn push_sequence_token(&mut self, typ: SeqTokenType, len: Length, pixel_data: bool) {
        self.seq_delimiters.push(SeqToken {
            typ,
            pixel_data,
            len,
            base_offset: self.parser.position(),
        })
    }

    /// Retrieve the inner stateful decoder from this data set reader.
    pub fn into_decoder(self) -> S {
        self.parser
    }

    /// Advance and retrieve the next DICOM data token.
    ///
    /// **Note:** For the data set to be successfully parsed,
    /// the resulting data tokens needs to be consumed
    /// if they are of a value type.
    pub fn advance(&mut self) -> Option<Result<LazyDataToken<&mut S>>> {
        if self.hard_break {
            return None;
        }

        // if there was a peek, consume peeked token
        if let Some(peek) = self.peek.take() {
            let token = match peek {
                DataToken::ElementHeader(header) => LazyDataToken::ElementHeader(header),
                DataToken::SequenceStart { tag, len } => LazyDataToken::SequenceStart { tag, len },
                DataToken::ItemStart { len } => LazyDataToken::ItemStart { len },
                DataToken::ItemEnd => LazyDataToken::ItemEnd,
                DataToken::SequenceEnd => LazyDataToken::SequenceEnd,
                _ => unreachable!("peeked token should not be a value token"),
            };
            return Some(Ok(token));
        }

        // record the reading position before any further reading
        let bytes_read = self.parser.position();

        // item or sequence delimitation logic for explicit lengths
        if self.delimiter_check_pending {
            match self.update_seq_delimiters() {
                Err(e) => {
                    self.hard_break = true;
                    return Some(Err(e));
                }
                Ok(Some(token)) => return Some(Ok(token)),
                Ok(None) => { /* no-op */ }
            }
        }

        if self.in_sequence {
            // at sequence level, expecting item header

            match self.parser.decode_item_header() {
                Ok(header) => {
                    match header {
                        SequenceItemHeader::Item { len } => {

                            // sanitize length
                            let Some(len) = self.sanitize_length(len) else {
                                return Some(
                                    InvalidItemLengthSnafu {
                                        len: len.0,
                                        bytes_read: self.parser.position(),
                                    }
                                    .fail(),
                                )
                            };

                            // entered a new item
                            self.in_sequence = false;
                            self.push_sequence_token(
                                SeqTokenType::Item,
                                len,
                                self.seq_delimiters.last()
                                    .expect("item header should be read only inside an existing sequence")
                                    .pixel_data);
                            // items can be empty
                            if len == Length(0) {
                                self.delimiter_check_pending = true;
                            }
                            Some(Ok(LazyDataToken::ItemStart { len }))
                        },
                        SequenceItemHeader::ItemDelimiter => {
                            // closed an item
                            self.seq_delimiters.pop();
                            self.in_sequence = true;
                            // sequences can end after an item delimiter
                            self.delimiter_check_pending = true;
                            Some(Ok(LazyDataToken::ItemEnd))
                        }
                        SequenceItemHeader::SequenceDelimiter => {
                            // closed a sequence
                            self.seq_delimiters.pop();
                            self.in_sequence = false;
                            // items can end after a nested sequence ends
                            self.delimiter_check_pending = true;
                            Some(Ok(LazyDataToken::SequenceEnd))
                        }
                    }
                }
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(e).context(ReadItemHeaderSnafu { bytes_read }))
                }
            }
        } else if let Some(SeqToken {
            typ: SeqTokenType::Item,
            pixel_data: true,
            len,
            ..
        }) = self.seq_delimiters.last()
        {
            // item value

            let Some(len) = self.sanitize_length(*len) else {
                return Some(
                    InvalidItemLengthSnafu {
                        len: len.0,
                        bytes_read: self.parser.position(),
                    }
                    .fail(),
                );
            };

            let len = match len
                .get()
                .with_context(|| UndefinedLengthSnafu { bytes_read })
            {
                Ok(len) => len,
                Err(e) => return Some(Err(e)),
            };

            // need to pop item delimiter on the next iteration
            self.delimiter_check_pending = true;
            Some(Ok(LazyDataToken::LazyItemValue {
                len,
                decoder: &mut self.parser,
            }))
        } else if let Some(header) = self.last_header {
            if header.is_encapsulated_pixeldata() {
                self.push_sequence_token(SeqTokenType::Sequence, Length::UNDEFINED, true);
                self.last_header = None;

                // encapsulated pixel data, expecting offset table
                match self.parser.decode_item_header() {
                    Ok(header) => match header {
                        SequenceItemHeader::Item { len } => {

                            // sanitize length
                            let Some(len) = self.sanitize_length(len) else {
                                return Some(
                                    InvalidItemLengthSnafu {
                                        len: len.0,
                                        bytes_read: self.parser.position(),
                                    }
                                    .fail(),
                                );
                            };

                            // entered a new item
                            self.in_sequence = false;
                            self.push_sequence_token(SeqTokenType::Item, len, true);
                            // items can be empty
                            if len == Length(0) {
                                self.delimiter_check_pending = true;
                            }
                            Some(Ok(LazyDataToken::ItemStart { len }))
                        }
                        SequenceItemHeader::SequenceDelimiter => {
                            // empty pixel data
                            self.seq_delimiters.pop();
                            self.in_sequence = false;
                            Some(Ok(LazyDataToken::SequenceEnd))
                        }
                        SequenceItemHeader::ItemDelimiter => {
                            self.hard_break = true;
                            Some(UnexpectedItemDelimiterSnafu { bytes_read }.fail())
                        }
                    },
                    Err(e) => {
                        self.hard_break = true;
                        Some(Err(e).context(ReadItemHeaderSnafu { bytes_read }))
                    }
                }
            } else {
                // a plain element header was read, so an element value is expected
                self.last_header = None;

                // sequences can end after this token
                self.delimiter_check_pending = true;

                Some(Ok(LazyDataToken::LazyValue {
                    header,
                    decoder: &mut self.parser,
                }))
            }
        } else {
            // a data element header or item delimiter is expected
            match self.parser.decode_header() {
                Ok(DataElementHeader {
                    tag,
                    vr: VR::SQ,
                    len,
                }) => {
                    let Some(len) = self.sanitize_length(len) else {
                        return Some(
                            InvalidElementLengthSnafu {
                                tag,
                                len: len.0,
                                bytes_read: self.parser.position(),
                            }
                            .fail(),
                        );
                    };

                    self.in_sequence = true;
                    self.push_sequence_token(SeqTokenType::Sequence, len, false);

                    // sequences can end right after they start
                    if len == Length(0) {
                        self.delimiter_check_pending = true;
                    }

                    Some(Ok(LazyDataToken::SequenceStart { tag, len }))
                }
                Ok(DataElementHeader {
                    tag: Tag(0xFFFE, 0xE00D),
                    ..
                }) => {
                    self.in_sequence = true;
                    // pop item delimiter
                    self.seq_delimiters.pop();
                    // sequences can end after this token
                    self.delimiter_check_pending = true;
                    Some(Ok(LazyDataToken::ItemEnd))
                }
                Ok(header) if header.is_encapsulated_pixeldata() => {
                    // encapsulated pixel data conditions:
                    // expect a sequence of pixel data fragments

                    // save it for the next step
                    self.last_header = Some(header);
                    Some(Ok(LazyDataToken::PixelSequenceStart))
                }
                Ok(header) if header.len.is_undefined() => {
                    // treat other undefined length elements
                    // as data set sequences,
                    // discarding the VR in the process
                    self.in_sequence = true;

                    let DataElementHeader { tag, len, .. } = header;
                    self.push_sequence_token(SeqTokenType::Sequence, len, false);

                    Some(Ok(LazyDataToken::SequenceStart { tag, len }))
                }
                Ok(mut header) => {
                    // sanitize length
                    let Some(len) = self.sanitize_length(header.len) else {
                        return Some(
                            InvalidElementLengthSnafu {
                                tag: header.tag,
                                len: header.len.0,
                                bytes_read: self.parser.position(),
                            }
                            .fail(),
                        );
                    };
                    header.len = len;

                    // save it for the next step
                    self.last_header = Some(header);
                    Some(Ok(LazyDataToken::ElementHeader(header)))
                }
                Err(DecoderError::DecodeElementHeader {
                    source: dicom_encoding::decode::Error::ReadHeaderTag { source, .. },
                    ..
                }) if source.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // Note: if `UnexpectedEof` was reached while trying to read
                    // an element tag, then we assume that
                    // the end of a DICOM object was reached gracefully.
                    // This approach is unlikely to consume trailing bytes,
                    // but may ignore the current depth of the data set tree.
                    self.hard_break = true;
                    None
                }
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(e).context(ReadHeaderSnafu { bytes_read }))
                }
            }
        }
    }

    /// Peek the next token from the source by
    /// reading a new token in the first call.
    /// Subsequent calls to `peek` will return the same token
    /// until another consumer method is called.
    ///
    /// Peeking only works in a data or item element boundary,
    /// so the returned data token is either an element header or an item header.
    /// At the moment, a failed peek will result in a hard break,
    /// preventing further iteration.
    pub fn peek(&mut self) -> Result<Option<&DataToken>> {
        if self.peek.is_none() {
            // try to read the next token
            match self.advance() {
                None => return Ok(None),
                Some(Err(e)) => return Err(e),
                Some(Ok(token)) => match token {
                    LazyDataToken::ElementHeader(header) => {
                        self.peek = Some(DataToken::ElementHeader(header));
                    }
                    LazyDataToken::SequenceStart { tag, len } => {
                        self.peek = Some(DataToken::SequenceStart { tag, len });
                    }
                    LazyDataToken::ItemStart { len } => {
                        self.peek = Some(DataToken::ItemStart { len });
                    }
                    LazyDataToken::ItemEnd => {
                        self.peek = Some(DataToken::ItemEnd);
                    }
                    LazyDataToken::SequenceEnd => {
                        self.peek = Some(DataToken::SequenceEnd);
                    }
                    _ => {
                        self.hard_break = true;
                        return PeekSnafu {
                            bytes_read: self.parser.position(),
                        }
                        .fail();
                    }
                },
            }
        }
        Ok(self.peek.as_ref())
    }

    /// Check for a non-compliant length
    /// and handle it according to the current strategy.
    /// Returns `None` if the length cannot or should not be resolved.
    fn sanitize_length(&self, length: Length) -> Option<Length> {
        if length.is_defined() && length.0 & 1 != 0 {
            match self.options.odd_length {
                OddLengthStrategy::Accept => Some(length),
                OddLengthStrategy::NextEven => Some(length + 1),
                OddLengthStrategy::Fail => None,
            }
        } else {
            Some(length)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LazyDataSetReader, StatefulDecode};
    use crate::{
        dataset::{
            lazy_read::LazyDataSetReaderOptions, read::OddLengthStrategy, DataToken, LazyDataToken,
        },
        StatefulDecoder,
    };
    use dicom_core::value::PrimitiveValue;
    use dicom_core::{
        dicom_value,
        header::{DataElementHeader, Length},
    };
    use dicom_core::{Tag, VR};
    use dicom_encoding::decode::{
        explicit_le::ExplicitVRLittleEndianDecoder, implicit_le::ImplicitVRLittleEndianDecoder,
    };
    use dicom_encoding::{decode::basic::LittleEndianBasicDecoder, text::SpecificCharacterSet};

    fn validate_dataset_reader_implicit_vr<I>(data: &[u8], ground_truth: I)
    where
        I: IntoIterator<Item = DataToken>,
    {
        let mut cursor = data;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ImplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );

        validate_dataset_reader(data, parser, ground_truth)
    }

    fn validate_dataset_reader_explicit_vr<I>(data: &[u8], ground_truth: I)
    where
        I: IntoIterator<Item = DataToken>,
    {
        let mut cursor = data;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );

        validate_dataset_reader(data, parser, ground_truth)
    }

    fn validate_dataset_reader<I, D>(data: &[u8], parser: D, ground_truth: I)
    where
        I: IntoIterator<Item = DataToken>,
        D: StatefulDecode,
    {
        let mut dset_reader = LazyDataSetReader::new(parser);

        let mut gt_iter = ground_truth.into_iter();
        while let Some(res) = dset_reader.advance() {
            let gt_token = gt_iter.next().expect("ground truth is shorter");
            let token = res.expect("should parse without an error");
            let token = token.into_owned().unwrap();
            assert_eq!(token, gt_token);
        }

        assert_eq!(
            gt_iter.count(), // consume til the end
            0,               // we have already read all of them
            "unexpected number of tokens remaining"
        );
        assert_eq!(dset_reader.parser.position(), data.len() as u64);
    }

    #[test]
    fn lazy_read_sequence_explicit() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0x18, 0x00, 0x11, 0x60, // sequence tag: (0018,6011) SequenceOfUltrasoundRegions
            b'S', b'Q', // VR
            0x00, 0x00, // reserved
            0x2e, 0x00, 0x00, 0x00, // length: 28 + 18 = 46 (#= 2)
            // -- 12 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x14, 0x00, 0x00, 0x00, // item length: 20 (#= 2)
            // -- 20 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x01, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 1
            // -- 30 --
            0x18, 0x00, 0x14, 0x60, b'U', b'S', 0x02, 0x00, 0x02, 0x00, // (0018, 6012) RegionDataType, len = 2, value = 2
            // -- 40 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x0a, 0x00, 0x00, 0x00, // item length: 10 (#= 1)
            // -- 48 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x04, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 4
            // -- 58 --
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04, 0x00, // (0020,4000) ImageComments, len = 4
            b'T', b'E', b'S', b'T', // value = "TEST"
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0018, 0x6011),
                len: Length(46),
            },
            DataToken::ItemStart { len: Length(20) },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([1].as_ref().into())),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6014),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([2].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::ItemStart { len: Length(10) },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([4].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0020, 0x4000),
                vr: VR::LT,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("TEST".into())),
        ];

        validate_dataset_reader_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn lazy_read_sequence_explicit_2() {
        static DATA: &[u8] = &[
            // SequenceStart: (0008,2218) ; len = 54 (#=3)
            0x08, 0x00, 0x18, 0x22, b'S', b'Q', 0x00, 0x00, 0x36, 0x00, 0x00, 0x00,
            // -- 12, --
            // ItemStart: len = 46
            0xfe, 0xff, 0x00, 0xe0, 0x2e, 0x00, 0x00, 0x00,
            // -- 20, --
            // ElementHeader: (0008,0100) CodeValue; len = 8
            0x08, 0x00, 0x00, 0x01, b'S', b'H', 0x08, 0x00, // PrimitiveValue
            0x54, 0x2d, 0x44, 0x31, 0x32, 0x31, 0x33, b' ',
            // -- 36, --
            // ElementHeader: (0008,0102) CodingSchemeDesignator; len = 4
            0x08, 0x00, 0x02, 0x01, b'S', b'H', 0x04, 0x00, // PrimitiveValue
            0x53, 0x52, 0x54, b' ',
            // -- 48, --
            // (0008,0104) CodeMeaning; len = 10
            0x08, 0x00, 0x04, 0x01, b'L', b'O', 0x0a, 0x00, // PrimitiveValue
            0x4a, 0x61, 0x77, b' ', 0x72, 0x65, 0x67, 0x69, 0x6f, 0x6e,
            // -- 66 --
            // SequenceStart: (0040,0555) AcquisitionContextSequence; len = 0
            0x40, 0x00, 0x55, 0x05, b'S', b'Q', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // ElementHeader: (2050,0020) PresentationLUTShape; len = 8
            0x50, 0x20, 0x20, 0x00, b'C', b'S', 0x08, 0x00, // PrimitiveValue
            b'I', b'D', b'E', b'N', b'T', b'I', b'T', b'Y',
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0008, 0x2218),
                len: Length(54),
            },
            DataToken::ItemStart { len: Length(46) },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0100),
                vr: VR::SH,
                len: Length(8),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(
                ["T-D1213 ".to_owned()].as_ref().into(),
            )),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0102),
                vr: VR::SH,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(["SRT ".to_owned()].as_ref().into())),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0104),
                vr: VR::LO,
                len: Length(10),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(
                ["Jaw region".to_owned()].as_ref().into(),
            )),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::SequenceStart {
                tag: Tag(0x0040, 0x0555),
                len: Length(0),
            },
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x2050, 0x0020),
                vr: VR::CS,
                len: Length(8),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(
                ["IDENTITY".to_owned()].as_ref().into(),
            )),
        ];

        validate_dataset_reader_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn lazy_read_sequence_implicit() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0x18, 0x00, 0x11, 0x60, // sequence tag: (0018,6011) SequenceOfUltrasoundRegions
            b'S', b'Q', // VR
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0xff, 0xff, 0xff, 0xff, // item length: undefined
            // -- 20 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x01, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 1
            // -- 30 --
            0x18, 0x00, 0x14, 0x60, b'U', b'S', 0x02, 0x00, 0x02, 0x00, // (0018, 6012) RegionDataType, len = 2, value = 2
            // -- 40 --
            0xfe, 0xff, 0x0d, 0xe0, 0x00, 0x00, 0x00, 0x00, // item end
            // -- 48 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0xff, 0xff, 0xff, 0xff, // item length: undefined
            // -- 56 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x04, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 4
            // -- 66 --
            0xfe, 0xff, 0x0d, 0xe0, 0x00, 0x00, 0x00, 0x00, // item end
            // -- 74 --
            0xfe, 0xff, 0xdd, 0xe0, 0x00, 0x00, 0x00, 0x00, // sequence end
            // -- 82 --
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04, 0x00, // (0020,4000) ImageComments, len = 4
            b'T', b'E', b'S', b'T', // value = "TEST"
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0018, 0x6011),
                len: Length::UNDEFINED,
            },
            DataToken::ItemStart {
                len: Length::UNDEFINED,
            },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([1].as_ref().into())),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6014),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([2].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::ItemStart {
                len: Length::UNDEFINED,
            },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([4].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0020, 0x4000),
                vr: VR::LT,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("TEST".into())),
        ];

        validate_dataset_reader_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn lazy_read_dataset_in_dataset() {
        #[rustfmt::skip]
        const DATA: &[u8; 138] = &[
            // 0: (2001, 9000) private sequence
            0x01, 0x20, 0x00, 0x90, //
            // length: undefined
            0xFF, 0xFF, 0xFF, 0xFF, //
            // 8: Item start
            0xFE, 0xFF, 0x00, 0xE0, //
            // Item length explicit (114 bytes)
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
            b'I', b'D', b'E', b'N', b'T', b'I', b'T', b'Y', //
            // 130: Sequence end
            0xFE, 0xFF, 0xDD, 0xE0, //
            0x00, 0x00, 0x00, 0x00, //
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x2001, 0x9000),
                len: Length::UNDEFINED,
            },
            DataToken::ItemStart { len: Length(114) },
            DataToken::SequenceStart {
                tag: Tag(0x0008, 0x1115),
                len: Length::UNDEFINED,
            },
            DataToken::ItemStart {
                len: Length::UNDEFINED,
            },
            DataToken::SequenceStart {
                tag: Tag(0x0008, 0x1140),
                len: Length::UNDEFINED,
            },
            DataToken::ItemStart {
                len: Length::UNDEFINED,
            },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x1150),
                vr: VR::UI,
                len: Length(26),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::from("1.2.840.10008.5.1.4.1.1.7\0")),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x2050, 0x0020),
                vr: VR::CS,
                len: Length(8),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::from("IDENTITY")),
            DataToken::ItemEnd, // inserted automatically
            DataToken::SequenceEnd,
        ];

        validate_dataset_reader_implicit_vr(DATA, ground_truth);
    }

    #[test]
    fn lazy_read_implicit_len_sequence_implicit_vr_unknown() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0x33, 0x55, 0x33, 0x55, // sequence tag: (5533,5533) «private, unknown attribute»
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 8 --
            0xfe, 0xff, 0x00, 0xe0, // item begin
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 16 --
            0xfe, 0xff, 0x0d, 0xe0, // item end
            0x00, 0x00, 0x00, 0x00, // length is always zero
            // -- 24 --
            0xfe, 0xff, 0xdd, 0xe0,
            0x00, 0x00, 0x00, 0x00, // sequence end
            // -- 32 --
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x5533, 0x5533),
                len: Length::UNDEFINED,
            },
            DataToken::ItemStart {
                len: Length::UNDEFINED,
            },
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
        ];

        validate_dataset_reader_implicit_vr(DATA, ground_truth);
    }

    #[test]
    fn read_encapsulated_pixeldata() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0xe0, 0x7f, 0x10, 0x00, // (7FE0, 0010) PixelData
            b'O', b'B', // VR 
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 -- Basic offset table
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x00, 0x00, 0x00, 0x00, // item length: 0
            // -- 20 -- First fragment of pixel data
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x20, 0x00, 0x00, 0x00, // item length: 32
            // -- 28 -- Compressed Fragment
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            // -- 60 -- End of pixel data
            0xfe, 0xff, 0xdd, 0xe0, // sequence end tag
            0x00, 0x00, 0x00, 0x00,
            // -- 68 -- padding
            0xfc, 0xff, 0xfc, 0xff, // (fffc,fffc) DataSetTrailingPadding
            b'O', b'B', // VR
            0x00, 0x00, // reserved
            0x08, 0x00, 0x00, 0x00, // length: 8
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let ground_truth = vec![
            DataToken::PixelSequenceStart,
            DataToken::ItemStart { len: Length(0) },
            DataToken::ItemEnd,
            DataToken::ItemStart { len: Length(32) },
            DataToken::ItemValue(vec![0x99; 32]),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader::new(
                Tag(0xfffc, 0xfffc),
                VR::OB,
                Length(8),
            )),
            DataToken::PrimitiveValue(PrimitiveValue::U8([0x00; 8].as_ref().into())),
        ];

        validate_dataset_reader_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn lazy_read_encapsulated_pixeldata_with_offset_table() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0xe0, 0x7f, 0x10, 0x00, // (7FE0, 0010) PixelData
            b'O', b'B', // VR 
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 -- Basic offset table
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x04, 0x00, 0x00, 0x00, // item length: 4
            // -- 20 -- item value
            0x10, 0x00, 0x00, 0x00, // 16
            // -- 24 -- First fragment of pixel data
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x20, 0x00, 0x00, 0x00, // item length: 32
            // -- 32 -- Compressed Fragment
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            // -- 60 -- End of pixel data
            0xfe, 0xff, 0xdd, 0xe0, // sequence end tag
            0x00, 0x00, 0x00, 0x00,
            // -- 68 -- padding
            0xfc, 0xff, 0xfc, 0xff, // (fffc,fffc) DataSetTrailingPadding
            b'O', b'B', // VR
            0x00, 0x00, // reserved
            0x08, 0x00, 0x00, 0x00, // length: 8
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let ground_truth = vec![
            DataToken::PixelSequenceStart,
            DataToken::ItemStart { len: Length(4) },
            DataToken::ItemValue(vec![0x10, 0x00, 0x00, 0x00]),
            DataToken::ItemEnd,
            DataToken::ItemStart { len: Length(32) },
            DataToken::ItemValue(vec![0x99; 32]),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader::new(
                Tag(0xfffc, 0xfffc),
                VR::OB,
                Length(8),
            )),
            DataToken::PrimitiveValue(PrimitiveValue::U8([0x00; 8].as_ref().into())),
        ];

        validate_dataset_reader_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn lazy_read_sequence_explicit_2_skip_values() {
        static DATA: &[u8] = &[
            // SequenceStart: (0008,2218) ; len = 54 (#=3)
            0x08, 0x00, 0x18, 0x22, b'S', b'Q', 0x00, 0x00, 0x36, 0x00, 0x00, 0x00,
            // -- 12, --
            // ItemStart: len = 46
            0xfe, 0xff, 0x00, 0xe0, 0x2e, 0x00, 0x00, 0x00,
            // -- 20, --
            // ElementHeader: (0008,0100) CodeValue; len = 8
            0x08, 0x00, 0x00, 0x01, b'S', b'H', 0x08, 0x00, // PrimitiveValue
            0x54, 0x2d, 0x44, 0x31, 0x32, 0x31, 0x33, b' ',
            // -- 36, --
            // ElementHeader: (0008,0102) CodingSchemeDesignator; len = 4
            0x08, 0x00, 0x02, 0x01, b'S', b'H', 0x04, 0x00, // PrimitiveValue
            0x53, 0x52, 0x54, b' ',
            // -- 48, --
            // (0008,0104) CodeMeaning; len = 10
            0x08, 0x00, 0x04, 0x01, b'L', b'O', 0x0a, 0x00, // PrimitiveValue
            0x4a, 0x61, 0x77, b' ', 0x72, 0x65, 0x67, 0x69, 0x6f, 0x6e,
            // -- 66 --
            // SequenceStart: (0040,0555) AcquisitionContextSequence; len = 0
            0x40, 0x00, 0x55, 0x05, b'S', b'Q', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // ElementHeader: (2050,0020) PresentationLUTShape; len = 8
            0x50, 0x20, 0x20, 0x00, b'C', b'S', 0x08, 0x00, // PrimitiveValue
            b'I', b'D', b'E', b'N', b'T', b'I', b'T', b'Y',
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0008, 0x2218),
                len: Length(54),
            },
            DataToken::ItemStart { len: Length(46) },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0100),
                vr: VR::SH,
                len: Length(8),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(
                ["T-D1213 ".to_owned()].as_ref().into(),
            )),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0102),
                vr: VR::SH,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(["SRT ".to_owned()].as_ref().into())),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0104),
                vr: VR::LO,
                len: Length(10),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(
                ["Jaw region".to_owned()].as_ref().into(),
            )),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::SequenceStart {
                tag: Tag(0x0040, 0x0555),
                len: Length(0),
            },
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x2050, 0x0020),
                vr: VR::CS,
                len: Length(8),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Strs(
                ["IDENTITY".to_owned()].as_ref().into(),
            )),
        ];

        let mut cursor = DATA;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );

        let mut dset_reader = LazyDataSetReader::new(parser);

        let mut gt_iter = ground_truth.into_iter();
        while let Some(res) = dset_reader.advance() {
            let token = res.expect("should parse without an error");
            let gt_token = gt_iter.next().expect("ground truth is shorter");
            match token {
                LazyDataToken::LazyValue { .. } | LazyDataToken::LazyItemValue { .. } => {
                    token.skip().unwrap();
                }
                token => {
                    let token = token.into_owned().unwrap();
                    assert_eq!(token, gt_token);
                }
            }
        }

        assert_eq!(
            gt_iter.count(), // consume til the end
            0,               // we have already read all of them
            "unexpected number of tokens remaining"
        );
        assert_eq!(dset_reader.parser.position(), DATA.len() as u64);
    }

    #[test]
    fn lazy_read_value_via_into_value() {
        // manually crafted DICOM data elements
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
            0x02, 0x00, 0x02, 0x00, 0x55, 0x49, 0x1a, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34,
            0x30, 0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e,
            0x31, 0x2e, 0x31, 0x2e, 0x31, 0x00, 0x02, 0x00, 0x10, 0x00, 0x55, 0x49, 0x14, 0x00,
            0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e,
            0x31, 0x2e, 0x32, 0x2e, 0x31, 0x00,
        ];
        let mut cursor = &RAW[..];
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );

        let mut dset_reader = LazyDataSetReader::new(parser);

        let token = dset_reader
            .advance()
            .expect("Expected token 1")
            .expect("Failed to read token 1");

        let header_token1 = match token {
            LazyDataToken::ElementHeader(header) => header,
            _ => {
                panic!("Unexpected token type (1)");
            }
        };

        let token = dset_reader
            .advance()
            .expect("Expected token 2")
            .expect("Failed to read token 2");

        match token {
            LazyDataToken::LazyValue { header, decoder: _ } => {
                assert_eq!(header_token1, header);
            }
            _ => {
                panic!("Unexpected token type (2)");
            }
        }

        // consume via into_value
        assert_eq!(
            token.into_value().unwrap(),
            dicom_value!(Strs, ["1.2.840.10008.5.1.4.1.1.1\0"]),
        );

        let token = dset_reader
            .advance()
            .expect("Expected token 3")
            .expect("Failed to read token 3");

        let header_token3 = match token {
            LazyDataToken::ElementHeader(header) => header,
            _ => {
                panic!("Unexpected token type (3)");
            }
        };

        let token = dset_reader
            .advance()
            .expect("Expected token 4")
            .expect("Failed to read token 4");

        match token {
            LazyDataToken::LazyValue { header, decoder: _ } => {
                assert_eq!(header_token3, header);
            }
            _ => {
                panic!("Unexpected token type (4)");
            }
        }

        // consume via into_value
        assert_eq!(
            token.into_value().unwrap(),
            dicom_value!(Strs, ["1.2.840.10008.1.2.1\0"]),
        );

        assert!(
            dset_reader.advance().is_none(),
            "unexpected number of tokens remaining"
        );
    }

    #[test]
    fn peek_data_elements() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0x18, 0x00, 0x11, 0x60, // sequence tag: (0018,6011) SequenceOfUltrasoundRegions
            b'S', b'Q', // VR
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 --
            0xfe, 0xff, 0xdd, 0xe0, 0x00, 0x00, 0x00, 0x00, // sequence end
            // -- 20 --
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04, 0x00, // (0020,4000) ImageComments, len = 4
            // -- 28 --
            b'T', b'E', b'S', b'T', // value = "TEST"
            // -- 32 --
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0018, 0x6011),
                len: Length::UNDEFINED,
            },
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0020, 0x4000),
                vr: VR::LT,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("TEST".into())),
        ];

        let mut cursor = DATA;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder::default(),
            SpecificCharacterSet::default(),
        );
        let mut dset_reader = LazyDataSetReader::new(parser);

        // peek at first token
        let token = dset_reader.peek().expect("should peek first token OK");
        assert_eq!(token, Some(&ground_truth[0]));

        assert_eq!(dset_reader.parser.position(), 12);

        // peeking multiple times gives the same result
        let token = dset_reader
            .peek()
            .expect("should peek first token again OK");
        assert_eq!(token, Some(&ground_truth[0]));

        assert_eq!(dset_reader.parser.position(), 12);

        // Using `advance` give us the same token
        let token = dset_reader
            .advance()
            .expect("expected token")
            .expect("should read token peeked OK");
        assert_eq!(&token.into_owned().unwrap(), &ground_truth[0]);

        assert_eq!(dset_reader.parser.position(), 12);

        // sequence end
        let token = dset_reader
            .advance()
            .expect("expected token")
            .expect("should read token OK");
        assert_eq!(&token.into_owned().unwrap(), &ground_truth[1]);

        assert_eq!(dset_reader.parser.position(), 20);

        // peek data element header
        let token = dset_reader.peek().expect("should peek first token OK");
        assert_eq!(token, Some(&ground_truth[2]));

        assert_eq!(dset_reader.parser.position(), 28);

        // read data element header
        let token = dset_reader
            .advance()
            .expect("expected token")
            .expect("should read token OK");
        assert_eq!(&token.into_owned().unwrap(), &ground_truth[2]);

        // should not have read anything else
        assert_eq!(dset_reader.parser.position(), 28);

        // read string value
        let token = dset_reader
            .advance()
            .expect("expected token")
            .expect("should read token OK");
        assert_eq!(&token.into_owned().unwrap(), &ground_truth[3]);

        // finished reading, peek should return None
        assert!(dset_reader.peek().unwrap().is_none());
    }

    #[test]
    fn read_odd_length_element() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0x08, 0x00, 0x16, 0x00, // (0008,0016) SOPClassUID
            b'U', b'I', // VR
            0x0b, 0x00, // len = 11
            b'1', b'.', b'2', b'.', b'8', b'4', b'0', b'.', b'1', b'0', b'0',
            0x00, // padding
        ];

        let ground_truth = vec![
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0016),
                vr: VR::UI,
                len: Length(12),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::from("1.2.840.100\0")),
        ];

        // strategy: assume next even

        let mut cursor = DATA;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );
        let mut dset_reader = LazyDataSetReader::new_with_options(
            parser,
            LazyDataSetReaderOptions {
                odd_length: OddLengthStrategy::NextEven,
                ..Default::default()
            },
        );

        // read next
        let token = dset_reader
            .advance()
            .expect("expected token")
            .expect("should read token OK");

        assert_eq!(&token.into_owned().unwrap(), &ground_truth[0],);

        // strategy: fail

        let mut cursor = DATA;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );
        let mut dset_reader = LazyDataSetReader::new_with_options(
            parser,
            LazyDataSetReaderOptions {
                odd_length: OddLengthStrategy::Fail,
                ..Default::default()
            },
        );

        let token = dset_reader.advance();

        assert!(
            matches!(
                token,
                Some(Err(super::Error::InvalidElementLength {
                    tag: Tag(0x0008, 0x0016),
                    len: 11,
                    bytes_read: 8,
                    ..
                })),
            ),
            "got: {:?}",
            token
        );
    }
}
