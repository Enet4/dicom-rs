//! This module contains a mid-level abstraction for reading DICOM content
//! sequentially.
//!
//! The rest of the crate is used to obtain DICOM element headers and values.
//! At this level, headers and values are treated as tokens which can be used
//! to form a syntax tree of a full data set.
use crate::stateful::decode::{CharacterSetOverride, DynStatefulDecoder, Error as DecoderError, StatefulDecode};
use dicom_core::header::{DataElementHeader, Header, Length, SequenceItemHeader};
use dicom_core::{PrimitiveValue, Tag, VR};
use dicom_encoding::text::SpecificCharacterSet;
use dicom_encoding::transfer_syntax::TransferSyntax;
use snafu::{Backtrace, ResultExt, Snafu};
use std::cmp::Ordering;
use std::io::Read;

use super::{DataToken, SeqTokenType};

fn is_stateful_decode<T>(_: &T)
where
    T: StatefulDecode,
{
}

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Could not create decoder"))]
    CreateDecoder {
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Could not read item header"))]
    ReadItemHeader {
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Could not read element header"))]
    ReadHeader {
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Could not read {} value bytes for element tagged {}", len, tag))]
    ReadValue {
        len: u32,
        tag: Tag,
        #[snafu(backtrace)]
        source: DecoderError,
    },
    #[snafu(display("Could not read {} bytes for item value", len))]
    ReadItemValue {
        len: u32,
        #[snafu(backtrace)]
        source: DecoderError,
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
    #[snafu(display("Unexpected item tag {} while reading element header", tag))]
    UnexpectedItemTag { tag: Tag, backtrace: Backtrace },
    #[snafu(display(
        "Unexpected item header outside a dataset sequence at {:#x}",
        bytes_read
    ))]
    UnexpectedItemHeader {
        bytes_read: u64,
        backtrace: Backtrace,
    },
    /// Undefined pixel data item length
    UndefinedItemLength,
    /// Invalid data element length {len:04X} of {tag} at {bytes_read:#x}
    InvalidElementLength { tag: Tag, len: u32, bytes_read: u64 },
    /// Invalid sequence item length {len:04X} at {bytes_read:#x}
    InvalidItemLength { len: u32, bytes_read: u64 },
}

pub type Result<T> = std::result::Result<T, Error>;

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

/// The value reading strategy for the data set reader.
///
/// It defines how the `PrimitiveValue`s in value tokens are constructed.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
pub enum ValueReadStrategy {
    /// Textual values will be decoded according to their value representation.
    ///
    /// Word-sized binary values are read according to
    /// the expected byte order.
    /// Dates, times, and date-times (DA, DT, TM) are parsed
    /// into their more specific variants,
    /// leading to parser failure if they are not valid DICOM.
    /// String numbers (IS, FD) are also converted into binary representations.
    /// For the case of floats, this may introduce precision errors.
    Interpreted,
    /// Values will be stored without decoding dates or textual numbers.
    ///
    /// Word-sized binary values are read according to
    /// the expected byte order.
    /// Date-time values and numbers are kept in their original string
    /// representation as string objects.
    /// All text is still decoded into Rust string values,
    /// in accordance to the standard,
    /// unless its value representation is unknown to the decoder.
    ///
    /// This is the default strategy.
    #[default]
    Preserved,
    /// All primitive values are fetched as raw byte buffers,
    /// without any form of decoding or interpretation.
    /// Not even byte order conversions are made.
    ///
    /// This strategy is not recommended,
    /// as it makes the retrieval of important textual data more difficult.
    Raw,
}

/// A strategy for when the parser finds a data element with an odd number
/// in the _length_ header field.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum OddLengthStrategy {
    /// Accept elements with an odd length as is,
    /// continuing data set reading normally.
    #[default]
    Accept,
    /// Assume that the real length is `length + 1`,
    /// as in the next even number.
    NextEven,
    /// Raise an error instead
    Fail,
}

/// The set of options for the data set reader.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct DataSetReaderOptions {
    /// The value reading strategy
    pub value_read: ValueReadStrategy,

    /// Whether to assume a different character set
    /// depending on certain conditions
    pub charset_override: CharacterSetOverride,

    /// The strategy for handling odd length data elements
    pub odd_length: OddLengthStrategy,
    /// The position of the reader as received at building time in bytes.
    /// Defaults to 0.
    pub base_offset: u64,
}

impl DataSetReaderOptions {
    /// Replace the value reading strategy of the options.
    pub fn value_read(mut self, value_read: ValueReadStrategy) -> Self {
        self.value_read = value_read;
        self
    }
    /// Replace the base reader offset of the options.
    pub fn base_offset(mut self, base_offset: u64) -> Self {
        self.base_offset = base_offset;
        self
    }
}

/// A higher-level reader for retrieving structure in a DICOM data set from an
/// arbitrary data source.
#[derive(Debug)]
pub struct DataSetReader<S> {
    /// the stateful decoder
    parser: S,
    /// the options of this reader
    options: DataSetReaderOptions,
    /// whether the reader is expecting an item header next (or a sequence delimiter)
    in_sequence: bool,
    /// whether the reader is expecting the first item value of a pixel sequence next
    /// (offset table)
    offset_table_next: bool,
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

impl<R> DataSetReader<DynStatefulDecoder<R>> {
    /// Create a new data set token reader with the given byte source,
    /// while considering the given transfer syntax specifier.
    #[inline]
    pub fn new_with_ts(source: R, ts: &TransferSyntax) -> Result<Self>
    where
        R: Read,
    {
        Self::new_with_ts_cs_options(source, ts, Default::default(), Default::default())
    }

    /// Create a new iterator with the given transfer syntax and options.
    #[inline]
    pub fn new_with_ts_options(
        source: R,
        ts: &TransferSyntax,
        options: DataSetReaderOptions,
    ) -> Result<Self>
    where
        R: Read,
    {
        Self::new_with_ts_cs_options(source, ts, SpecificCharacterSet::default(), options)
    }

    /// Create a new data set token reader with the given byte source,
    /// while considering the given transfer syntax specifier
    /// and the specific character set to assume by default.
    ///
    /// Note that the data set being read
    /// can override the character set with the presence of a
    /// _Specific Character Set_ data element.
    #[inline]
    pub fn new_with_ts_cs(source: R, ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self>
    where
        R: Read,
    {
        Self::new_with_ts_cs_options(source, ts, cs, Default::default())
    }

    /// Create a new iterator with the given stateful decoder and options.
    pub fn new_with_ts_cs_options(
        source: R,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
        options: DataSetReaderOptions,
    ) -> Result<Self>
    where
        R: Read,
    {
        let parser = DynStatefulDecoder::new_with_override(source, ts, cs, options.charset_override, 0).context(CreateDecoderSnafu)?;

        is_stateful_decode(&parser);

        Ok(DataSetReader {
            parser,
            options,
            seq_delimiters: Vec::new(),
            delimiter_check_pending: false,
            offset_table_next: false,
            in_sequence: false,
            hard_break: false,
            last_header: None,
            peek: None,
        })
    }
}

impl<S> DataSetReader<S> {
    /// Create a new iterator with the given stateful decoder and options.
    pub fn new(decoder: S, options: DataSetReaderOptions) -> Self {
        DataSetReader {
            parser: decoder,
            options,
            seq_delimiters: Vec::new(),
            delimiter_check_pending: false,
            offset_table_next: false,
            in_sequence: false,
            hard_break: false,
            last_header: None,
            peek: None,
        }
    }
}

impl<S> Iterator for DataSetReader<S>
where
    S: StatefulDecode,
{
    type Item = Result<DataToken>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hard_break {
            return None;
        }
        // if there was a peek, consume peeked token
        if let Some(token) = self.peek.take() {
            return Some(Ok(token));
        }

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
                            let len = match self.sanitize_length(len) {
                                Some(len) => len,
                                None => {
                                    return Some(
                                        InvalidItemLengthSnafu {
                                            bytes_read: self.parser.position(),
                                            len: len.0,
                                        }
                                        .fail(),
                                    )
                                }
                            };
                            // entered a new item
                            self.in_sequence = false;

                            let last_delimiter = match self.seq_delimiters.last() {
                                Some(d) => d,
                                None => {
                                    return Some(
                                        UnexpectedItemHeaderSnafu {
                                            bytes_read: self.parser.position(),
                                        }
                                        .fail(),
                                    )
                                }
                            };
                            self.push_sequence_token(
                                SeqTokenType::Item,
                                len,
                                last_delimiter.pixel_data,
                            );
                            // items can be empty
                            if len == Length(0) {
                                self.delimiter_check_pending = true;
                            }
                            Some(Ok(DataToken::ItemStart { len }))
                        }
                        SequenceItemHeader::ItemDelimiter => {
                            // closed an item
                            self.seq_delimiters.pop();
                            self.in_sequence = true;
                            // sequences can end after an item delimiter
                            self.delimiter_check_pending = true;
                            Some(Ok(DataToken::ItemEnd))
                        }
                        SequenceItemHeader::SequenceDelimiter => {
                            // closed a sequence
                            self.seq_delimiters.pop();
                            self.in_sequence = false;
                            // items can end after a nested sequence ends
                            self.delimiter_check_pending = true;
                            Some(Ok(DataToken::SequenceEnd))
                        }
                    }
                }
                Err(DecoderError::DecodeItemHeader {
                    source: dicom_encoding::decode::Error::ReadItemHeader { source, .. },
                    ..
                }) if source.kind() == std::io::ErrorKind::UnexpectedEof
                   && self.seq_delimiters.pop().is_some_and(|t| t.pixel_data)
                 => {
                    // Note: if `UnexpectedEof` was reached while inside a 
                    // PixelData Sequence, then we assume that
                    // the end of a DICOM object was reached gracefully.
                    self.hard_break = true;
                    None
                }
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(e).context(ReadItemHeaderSnafu))
                }
            }
        } else if let Some(SeqToken {
            typ: SeqTokenType::Item,
            pixel_data: true,
            len,
            ..
        }) = self.seq_delimiters.last()
        {
            let len = match len.get() {
                Some(len) => len as usize,
                None => return Some(UndefinedItemLengthSnafu.fail()),
            };

            if self.offset_table_next {
                // offset table
                let mut offset_table = Vec::with_capacity(len);

                self.offset_table_next = false;

                // need to pop item delimiter on the next iteration
                self.delimiter_check_pending = true;

                Some(
                    match self.parser.read_u32_to_vec(len as u32, &mut offset_table) {
                        Ok(()) => Ok(DataToken::OffsetTable(offset_table)),
                        Err(e) => Err(e).context(ReadItemValueSnafu { len: len as u32 }),
                    },
                )
            } else {
                // item value
                let mut value = Vec::with_capacity(len);

                // need to pop item delimiter on the next iteration
                self.delimiter_check_pending = true;
                Some(
                    self.parser
                        .read_to_vec(len as u32, &mut value)
                        .map(|_| Ok(DataToken::ItemValue(value)))
                        .unwrap_or_else(|e| Err(e).context(ReadItemValueSnafu { len: len as u32 })),
                )
            }
        } else if let Some(header) = self.last_header {
            if header.is_encapsulated_pixeldata() {
                self.push_sequence_token(SeqTokenType::Sequence, Length::UNDEFINED, true);
                self.last_header = None;

                // encapsulated pixel data, expecting offset table
                match self.parser.decode_item_header() {
                    Ok(header) => match header {
                        SequenceItemHeader::Item { len } => {
                            let len = match self.sanitize_length(len) {
                                Some(len) => len,
                                None => {
                                    return Some(
                                        InvalidItemLengthSnafu {
                                            bytes_read: self.parser.position(),
                                            len: len.0,
                                        }
                                        .fail(),
                                    )
                                }
                            };

                            // entered a new item
                            self.in_sequence = false;
                            self.push_sequence_token(SeqTokenType::Item, len, true);
                            // items can be empty
                            if len == Length(0) {
                                self.delimiter_check_pending = true;
                            } else {
                                self.offset_table_next = true;
                            }
                            Some(Ok(DataToken::ItemStart { len }))
                        }
                        SequenceItemHeader::SequenceDelimiter => {
                            // empty pixel data
                            self.seq_delimiters.pop();
                            self.in_sequence = false;
                            Some(Ok(DataToken::SequenceEnd))
                        }
                        item => {
                            self.hard_break = true;
                            Some(UnexpectedItemTagSnafu { tag: item.tag() }.fail())
                        }
                    },
                    Err(e) => {
                        self.hard_break = true;
                        Some(Err(e).context(ReadItemHeaderSnafu))
                    }
                }
            } else {
                // a plain element header was read, so a value is expected
                let value = match self.read_value(&header) {
                    Ok(v) => v,
                    Err(e) => {
                        self.hard_break = true;
                        self.last_header = None;
                        return Some(Err(e));
                    }
                };

                self.last_header = None;

                // sequences can end after this token
                self.delimiter_check_pending = true;

                Some(Ok(DataToken::PrimitiveValue(value)))
            }
        } else {
            // a data element header or item delimiter is expected
            match self.parser.decode_header() {
                Ok(DataElementHeader {
                    tag,
                    vr: VR::SQ,
                    len,
                }) => {
                    let len = match self.sanitize_length(len) {
                        Some(len) => len,
                        None => {
                            return Some(
                                InvalidElementLengthSnafu {
                                    tag,
                                    len: len.0,
                                    bytes_read: self.parser.position(),
                                }
                                .fail(),
                            )
                        }
                    };

                    self.in_sequence = true;
                    self.push_sequence_token(SeqTokenType::Sequence, len, false);

                    // sequences can end right after they start
                    if len == Length(0) {
                        self.delimiter_check_pending = true;
                    }

                    Some(Ok(DataToken::SequenceStart { tag, len }))
                }
                Ok(DataElementHeader {
                    tag: Tag(0xFFFE, 0xE00D),
                    ..
                }) if self.seq_delimiters.is_empty() => {
                    // ignore delimiter, we are not in a sequence
                    tracing::warn!(
                        "Item delimitation item outside of a sequence in position {}",
                        self.parser.position()
                    );
                    // return a new token by calling the method again
                    self.next()
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
                    Some(Ok(DataToken::ItemEnd))
                }
                Ok(header) if header.is_encapsulated_pixeldata() => {
                    // encapsulated pixel data conditions:
                    // expect a sequence of pixel data fragments

                    // save it for the next step
                    self.last_header = Some(header);
                    Some(Ok(DataToken::PixelSequenceStart))
                }
                Ok(header) if header.len.is_undefined() => {
                    // treat other undefined length elements
                    // as data set sequences,
                    // discarding the VR in the process
                    self.in_sequence = true;

                    let DataElementHeader { tag, len, .. } = header;
                    self.push_sequence_token(SeqTokenType::Sequence, len, false);

                    Some(Ok(DataToken::SequenceStart { tag, len }))
                }
                Ok(mut header) => {
                    match self.sanitize_length(header.len) {
                        Some(len) => header.len = len,
                        None => {
                            return Some(
                                InvalidElementLengthSnafu {
                                    tag: header.tag,
                                    len: header.len.0,
                                    bytes_read: self.parser.position(),
                                }
                                .fail(),
                            )
                        }
                    };

                    // save it for the next step
                    self.last_header = Some(header);
                    Some(Ok(DataToken::ElementHeader(header)))
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
                    Some(Err(e).context(ReadHeaderSnafu))
                }
            }
        }
    }
}

impl<S> DataSetReader<S>
where
    S: StatefulDecode,
{
    /// Peek the next token from the source by
    /// reading a new token in the first call.
    /// Subsequent calls to `peek` will return the same token
    /// until another consumer method (such as `Iterator::next`)
    /// is called.
    pub fn peek(&mut self) -> Result<Option<&DataToken>> {
        if self.peek.is_none() {
            // try to read the next token
            match self.next() {
                None => return Ok(None),
                Some(Err(e)) => return Err(e),
                Some(Ok(token)) => {
                    self.peek = Some(token);
                }
            }
        }
        Ok(self.peek.as_ref())
    }

    fn update_seq_delimiters(&mut self) -> Result<Option<DataToken>> {
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
                                token = DataToken::SequenceEnd;
                            }
                            SeqTokenType::Item => {
                                self.in_sequence = true;
                                token = DataToken::ItemEnd;
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

    fn read_value(&mut self, header: &DataElementHeader) -> Result<PrimitiveValue> {
        match self.options.value_read {
            ValueReadStrategy::Interpreted => self.parser.read_value(header),
            ValueReadStrategy::Preserved => self.parser.read_value_preserved(header),
            ValueReadStrategy::Raw => self.parser.read_value_bytes(header),
        }
        .context(ReadValueSnafu {
            len: header.len.0,
            tag: header.tag,
        })
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
    use super::{DataSetReader, DataToken, StatefulDecode};
    use crate::dataset::read::{DataSetReaderOptions, OddLengthStrategy};
    use crate::stateful::decode::StatefulDecoder;
    use dicom_core::header::{DataElementHeader, Length};
    use dicom_core::value::PrimitiveValue;
    use dicom_core::{Tag, VR};
    use dicom_encoding::decode::basic::LittleEndianBasicDecoder;
    use dicom_encoding::decode::{
        explicit_le::ExplicitVRLittleEndianDecoder, implicit_le::ImplicitVRLittleEndianDecoder,
    };
    use dicom_encoding::text::SpecificCharacterSet;

    fn validate_read_data_implicit_vr<I>(data: &[u8], ground_truth: I)
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

        validate_read_data(data, parser, ground_truth)
    }

    fn validate_read_data_explicit_vr<I>(data: &[u8], ground_truth: I)
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

        validate_read_data(data, parser, ground_truth)
    }

    fn validate_read_data<I, D>(data: &[u8], parser: D, ground_truth: I)
    where
        I: IntoIterator<Item = DataToken>,
        D: StatefulDecode,
    {
        let dset_reader = DataSetReader::new(parser, Default::default());
        validate_data_set_reader(data, dset_reader, ground_truth);
    }

    fn validate_data_set_reader<S, I>(
        data: &[u8],
        mut dset_reader: DataSetReader<S>,
        ground_truth: I,
    ) where
        S: StatefulDecode,
        I: IntoIterator<Item = DataToken>,
    {
        let iter = &mut dset_reader;
        let ground_truth = ground_truth.into_iter();

        for gt_token in ground_truth {
            let token = iter
                .next()
                .expect("expecting more tokens from reader")
                .expect("should fetch the next token without an error");
            eprintln!("Next token: {token:2?} ; Expected: {gt_token:2?}");
            assert_eq!(
                token, gt_token,
                "Got token {token:2?} ; but expected {gt_token:2?}"
            );
        }

        let extra: Vec<_> = iter.collect();
        assert_eq!(
            extra.len(), // we have already read all of them
            0,
            "extraneous tokens remaining: {extra:?}",
        );
        assert_eq!(
            dset_reader.parser.position(),
            data.len() as u64,
            "Decoder position did not match end of data",
        );
    }

    #[test]
    fn read_sequence_explicit() {
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

        validate_read_data_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn read_sequence_explicit_2() {
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

        validate_read_data_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn read_empty_sequence_explicit() {
        static DATA: &[u8] = &[
            // SequenceStart: (0008,1032) ProcedureCodeSequence ; len = 0
            0x08, 0x00, 0x18, 0x22, // VR: SQ
            b'S', b'Q', // Reserved
            0x00, 0x00, // Length: 0
            0x00, 0x00, 0x00, 0x00,
        ];

        let ground_truth = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0008, 0x2218),
                len: Length(0),
            },
            DataToken::SequenceEnd,
        ];

        validate_read_data_explicit_vr(DATA, ground_truth);
    }

    /// Gracefully ignore a stray item end tag in the data set.
    #[test]
    fn ignore_trailing_item_delimitation_item() {
        static DATA: &[u8] = &[
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04,
            0x00, // (0020,4000) ImageComments, len = 4
            b'T', b'E', b'S', b'T', // value = "TEST"
            0xfe, 0xff, 0x0d, 0xe0, 0x00, 0x00, 0x00, 0x00, // item end
        ];

        let ground_truth = vec![
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0020, 0x4000),
                vr: VR::LT,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("TEST".into())),
            // no item end
        ];

        validate_read_data_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn read_sequence_implicit() {
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

        validate_read_data_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn read_implicit_len_sequence_implicit_vr_unknown() {
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

        validate_read_data_implicit_vr(DATA, ground_truth);
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

        validate_read_data_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn read_encapsulated_pixeldata_with_offset_table() {
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
            DataToken::OffsetTable(vec![16]),
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

        validate_read_data_explicit_vr(DATA, ground_truth);
    }

    #[test]
    fn read_dataset_in_dataset() {
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

        validate_read_data_implicit_vr(DATA, ground_truth);
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
            // -- 82 --
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04, 0x00, // (0020,4000) ImageComments, len = 4
            b'T', b'E', b'S', b'T', // value = "TEST"
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
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );
        let mut dset_reader = DataSetReader::new(parser, Default::default());

        let iter = &mut dset_reader;

        // peek at first token
        let token = iter.peek().expect("should peek first token OK");
        assert_eq!(token, Some(&ground_truth[0]));

        // peeking multiple times gives the same result
        let token = iter.peek().expect("should peek first token again OK");
        assert_eq!(token, Some(&ground_truth[0]));

        // Using `next` give us the same token
        let token = iter
            .next()
            .expect("expected token")
            .expect("should read token peeked OK");
        assert_eq!(&token, &ground_truth[0]);

        // read some more tokens

        // sequence end
        let token = iter.next().unwrap().unwrap();
        assert_eq!(&token, &ground_truth[1]);
        // data element header
        let token = iter.next().unwrap().unwrap();
        assert_eq!(&token, &ground_truth[2]);

        // peek string value
        let token = iter.peek().unwrap();
        assert_eq!(token, Some(&ground_truth[3]));
        // peek it again
        let token = iter.peek().unwrap();
        assert_eq!(token, Some(&ground_truth[3]));
        // then read it
        let token = iter.next().unwrap().unwrap();
        assert_eq!(&token, &ground_truth[3]);

        // finished reading, peek should return None
        assert!(iter.peek().unwrap().is_none());
    }

    #[test]
    fn read_pixel_sequence_bad_item_end() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0xe0, 0x7f, 0x10, 0x00, // (7FE0, 0010) PixelData
            b'O', b'B', // VR 
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x00, 0x00, 0x00, 0x00, // item length: 0
            // -- 20 --
            0xfe, 0xff, 0x0d, 0xe0, // item end
            0x00, 0x00, 0x00, 0x00, // length is always zero
            // -- 28 --
            0xfe, 0xff, 0x0d, 0xe0, // another item end (bad)
            0x00, 0x00, 0x00, 0x00, //
            // -- 36 --
            0xfe, 0xff, 0x00, 0xe0, // another item start
            0x00, 0x00, 0x00, 0x00, // item length: 0
        ];

        let mut cursor = DATA;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );
        let mut dset_reader = DataSetReader::new(parser, Default::default());

        let token_res = (&mut dset_reader)
            .collect::<Result<Vec<_>, _>>();
        dbg!(&token_res);
        assert!(token_res.is_err());
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
        let dset_reader = DataSetReader::new(
            parser,
            DataSetReaderOptions {
                odd_length: OddLengthStrategy::NextEven,
                ..Default::default()
            },
        );

        validate_data_set_reader(DATA, dset_reader, ground_truth);

        // strategy: fail

        let mut cursor = DATA;
        let parser = StatefulDecoder::new(
            &mut cursor,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::default(),
        );
        let dset_reader = DataSetReader::new(
            parser,
            DataSetReaderOptions {
                odd_length: OddLengthStrategy::Fail,
                ..Default::default()
            },
        );

        let mut tokens = dset_reader.into_iter();
        let token = tokens.next();

        assert!(matches!(
            token,
            Some(Err(super::Error::InvalidElementLength {
                tag: Tag(0x0008, 0x0016),
                len: 11,
                bytes_read: 8,
            })),
        ), "got: {:?}", token);
    }
}
