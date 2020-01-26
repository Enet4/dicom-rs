//! This module contains a mid-level abstraction for reading DICOM content
//! sequentially.
//!
//! The rest of the crate is used to obtain DICOM element headers and values.
//! At this level, headers and values are treated as tokens which can be used
//! to form a syntax tree of a full data set.
use crate::error::{Error, InvalidValueReadError, Result};
use crate::stateful::decode::{DynStatefulDecoder, StatefulDecode, StatefulDecoder};
use crate::util::{ReadSeek, SeekInterval};
use dicom_core::dictionary::DataDictionary;
use dicom_core::header::{DataElementHeader, Header, Length, SequenceItemHeader};
use dicom_core::{Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_encoding::text::SpecificCharacterSet;
use dicom_encoding::transfer_syntax::TransferSyntax;
use std::io::{Read, Seek, SeekFrom};
use std::iter::Iterator;
use std::marker::PhantomData;
use std::ops::DerefMut;

use super::{DataToken, SeqTokenType};

fn is_stateful_decode<T>(_: &T)
where
    T: StatefulDecode,
{
}

/// A reader-specific token representing a sequence or item start.
#[derive(Debug, Copy, Clone, PartialEq)]
struct SeqToken {
    /// Whether it is the start of a sequence or the start of an item.
    typ: SeqTokenType,
    /// The length of the value, as indicated by the starting element,
    /// can be unknown.
    len: Length,
    /// The number of bytes the parser has read until it reached the
    /// beginning of the sequence or item value data.
    base_offset: u64,
}

/// A higher-level reader for retrieving structure in a DICOM data set from an
/// arbitrary data source.
#[derive(Debug)]
pub struct DataSetReader<S, D> {
    parser: S,
    dict: D,
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
}

impl<'s> DataSetReader<DynStatefulDecoder<'s>, StandardDataDictionary> {
    /// Creates a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with<S: 's>(source: S, ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self>
    where
        S: Read,
    {
        let parser = DynStatefulDecoder::new_with(source, ts, cs)?;

        is_stateful_decode(&parser);

        Ok(DataSetReader {
            parser,
            dict: StandardDataDictionary,
            seq_delimiters: Vec::new(),
            delimiter_check_pending: false,
            in_sequence: false,
            hard_break: false,
            last_header: None,
        })
    }
}

impl<'s, D> DataSetReader<DynStatefulDecoder<'s>, D> {
    /// Creates a new iterator with the given random access source and data dictionary,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with_dictionary<S: 's>(
        source: S,
        dict: D,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<Self>
    where
        S: Read,
    {
        let parser = DynStatefulDecoder::new_with(source, ts, cs)?;

        is_stateful_decode(&parser);

        Ok(DataSetReader {
            parser,
            dict,
            seq_delimiters: Vec::new(),
            delimiter_check_pending: false,
            in_sequence: false,
            hard_break: false,
            last_header: None,
        })
    }
}

impl<S> DataSetReader<S, StandardDataDictionary> {
    /// Create a new iterator with the given parser.
    pub fn new(decoder: S) -> Self {
        DataSetReader {
            parser: decoder,
            dict: StandardDataDictionary,
            seq_delimiters: Vec::new(),
            delimiter_check_pending: false,
            in_sequence: false,
            hard_break: false,
            last_header: None,
        }
    }
}

impl<S, D> Iterator for DataSetReader<S, D>
where
    S: StatefulDecode,
    D: DataDictionary,
{
    type Item = Result<DataToken>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hard_break {
            return None;
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
            match self.parser.decode_item_header() {
                Ok(header) => match header {
                    SequenceItemHeader::Item { len } => {
                        // entered a new item
                        self.in_sequence = false;
                        self.seq_delimiters.push(SeqToken {
                            typ: SeqTokenType::Item,
                            len,
                            base_offset: self.parser.bytes_read(),
                        });
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
                        Some(Ok(DataToken::ItemEnd))
                    }
                    SequenceItemHeader::SequenceDelimiter => {
                        // closed a sequence
                        self.seq_delimiters.pop();
                        self.in_sequence = false;
                        Some(Ok(DataToken::SequenceEnd))
                    }
                },
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(e))
                }
            }
        } else if self.last_header.is_some() {
            // a plain element header was read, so a value is expected
            let header = self.last_header.unwrap();
            let value = match self.parser.read_value(&header) {
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
        } else {
            // a data element header or item delimiter is expected
            match self.parser.decode_header() {
                Ok(DataElementHeader {
                    tag,
                    vr: VR::SQ,
                    len,
                }) => {
                    self.in_sequence = true;
                    self.seq_delimiters.push(SeqToken {
                        typ: SeqTokenType::Sequence,
                        len,
                        base_offset: self.parser.bytes_read(),
                    });

                    // sequences can end right after they start
                    if len == Length(0) {
                        self.delimiter_check_pending = true;
                    }

                    Some(Ok(DataToken::SequenceStart { tag, len }))
                }
                Ok(DataElementHeader {
                    tag: Tag(0xFFFE, 0xE00D),
                    ..
                }) => {
                    self.in_sequence = true;
                    Some(Ok(DataToken::ItemEnd))
                }
                Ok(header) => {
                    // save it for the next step
                    self.last_header = Some(header);
                    Some(Ok(DataToken::ElementHeader(header)))
                }
                Err(Error::Io(ref e)) if e.kind() == ::std::io::ErrorKind::UnexpectedEof => {
                    // TODO there might be a more informative way to check
                    // whether the end of a DICOM object was reached gracefully
                    // or with problems. This approach may consume trailing
                    // bytes, and will ignore the possibility of trailing bytes
                    // having already been interpreted as an element header.
                    self.hard_break = true;
                    None
                }
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(e))
                }
            }
        }
    }
}

impl<S, D> DataSetReader<S, D>
where
    S: StatefulDecode,
{
    fn update_seq_delimiters(&mut self) -> Result<Option<DataToken>> {
        if let Some(sd) = self.seq_delimiters.last() {
            if let Some(len) = sd.len.get() {
                let eos = sd.base_offset + len as u64;
                let bytes_read = self.parser.bytes_read();
                if eos == bytes_read {
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
                } else if eos < bytes_read {
                    return Err(Error::InconsistentSequenceEnd(eos, bytes_read));
                }
            }
        }
        self.delimiter_check_pending = false;
        Ok(None)
    }
}

/// An iterator for retrieving DICOM object element markers from a random
/// access data source.
#[derive(Debug)]
pub struct LazyDataSetReader<DS, P> {
    parser: P,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
    phantom: PhantomData<DS>,
}

impl<'s> LazyDataSetReader<&'s mut dyn Read, DynStatefulDecoder<'s>> {
    /// Create a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(
        source: &'s mut dyn ReadSeek,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<Self> {
        let parser = StatefulDecoder::new_with(source, ts, cs)?;

        Ok(LazyDataSetReader {
            parser,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            phantom: PhantomData,
        })
    }
}

impl<DS, P> LazyDataSetReader<DS, P>
where
    P: StatefulDecode,
{
    /// Create a new iterator with the given parser.
    pub fn new(parser: P) -> LazyDataSetReader<DS, P> {
        LazyDataSetReader {
            parser,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            phantom: PhantomData,
        }
    }

    fn create_element_marker(&mut self, header: DataElementHeader) -> Result<DicomElementMarker> {
        Ok(DicomElementMarker {
            header,
            pos: self.parser.bytes_read(),
        })
    }

    fn create_item_marker(&mut self, header: SequenceItemHeader) -> Result<DicomElementMarker> {
        Ok(DicomElementMarker {
            header: From::from(header),
            pos: self.parser.bytes_read(),
        })
    }
}

impl<P> Iterator for LazyDataSetReader<(), P>
where
    P: StatefulDecode,
{
    type Item = Result<DicomElementMarker>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hard_break {
            return None;
        }
        if self.in_sequence {
            match self.parser.decode_item_header() {
                Ok(header) => match header {
                    header @ SequenceItemHeader::Item { .. } => {
                        self.in_sequence = false;
                        Some(self.create_item_marker(header))
                    }
                    SequenceItemHeader::ItemDelimiter => {
                        self.in_sequence = true;
                        Some(self.create_item_marker(header))
                    }
                    SequenceItemHeader::SequenceDelimiter => {
                        self.depth -= 1;
                        self.in_sequence = false;
                        Some(self.create_item_marker(header))
                    }
                },
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(e))
                }
            }
        } else {
            match self.parser.decode_header() {
                Ok(
                    header @ DataElementHeader {
                        tag: Tag(0x0008, 0x0005),
                        ..
                    },
                ) => {
                    let marker = self.create_element_marker(header);

                    Some(marker)
                }
                Ok(header) => {
                    // check if SQ
                    if header.vr() == VR::SQ {
                        self.in_sequence = true;
                        self.depth += 1;
                    }
                    Some(self.create_element_marker(header))
                }
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(e))
                }
            }
        }
    }
}

/// A data type for a DICOM element residing in a file, or any other source
/// with random access. A position in the file is kept for future access.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct DicomElementMarker {
    /// The header, kept in memory. At this level, the value representation
    /// "UN" may also refer to a non-applicable vr (i.e. for items and
    /// delimiters).
    pub header: DataElementHeader,
    /// The ending position of the element's header (or the starting position
    /// of the element's value if it exists), relative to the beginning of the
    /// file.
    pub pos: u64,
}

impl DicomElementMarker {
    /// Obtain an interval of the raw data associated to this element's data value.
    pub fn get_data_stream<S: ?Sized, B: DerefMut<Target = S>>(
        &self,
        source: B,
    ) -> Result<SeekInterval<S, B>>
    where
        S: ReadSeek,
    {
        let len = u64::from(
            self.header
                .len()
                .get()
                .ok_or(InvalidValueReadError::UnresolvedValueLength)?,
        );
        let interval = SeekInterval::new_at(source, self.pos..len)?;
        Ok(interval)
    }

    /// Move the source to the position indicated by the marker
    pub fn move_to_start<S: ?Sized, B: DerefMut<Target = S>>(&self, mut source: B) -> Result<()>
    where
        S: Seek,
    {
        source.seek(SeekFrom::Start(self.pos))?;
        Ok(())
    }

    /// Getter for this element's value representation. May be `UN`
    /// when this is not applicable.
    pub fn vr(&self) -> VR {
        self.header.vr()
    }
}

impl Header for DicomElementMarker {
    fn tag(&self) -> Tag {
        self.header.tag()
    }

    fn len(&self) -> Length {
        self.header.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{DataSetReader, DataToken, StatefulDecode, StatefulDecoder};
    use dicom_core::header::{DataElementHeader, Length};
    use dicom_core::value::PrimitiveValue;
    use dicom_core::{Tag, VR};
    use dicom_encoding::decode::basic::LittleEndianBasicDecoder;
    use dicom_encoding::text::DefaultCharacterSetCodec;
    use dicom_encoding::transfer_syntax::explicit_le::ExplicitVRLittleEndianDecoder;
    use std::io::Read;

    fn validate_dataset_reader<I>(data: &[u8], ground_truth: I)
    where
        I: IntoIterator<Item = DataToken>,
    {
        let mut cursor = data;
        let parser = StatefulDecoder::new(
            cursor.by_ref(),
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder::default(),
            Box::new(DefaultCharacterSetCodec::default()) as Box<_>, // trait object
        );

        let mut dset_reader = DataSetReader::new(parser);

        let mut iter = Iterator::zip(&mut dset_reader, ground_truth);

        while let Some((res, gt_token)) = iter.next() {
            let token = res.expect("should parse without an error");
            eprintln!("Next token: {:?}", token);
            assert_eq!(token, gt_token);
        }

        assert_eq!(
            iter.count(), // consume til the end
            0,            // we have already read all of them
            "unexpected number of tokens remaining"
        );
        assert_eq!(dset_reader.parser.bytes_read(), data.len() as u64);
    }

    #[test]
    fn sequence_reading_explicit() {
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

        validate_dataset_reader(DATA, ground_truth);
    }

    #[test]
    fn sequence_reading_explicit_2() {
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

        validate_dataset_reader(DATA, ground_truth);
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

        validate_dataset_reader(DATA, ground_truth);
    }
}
