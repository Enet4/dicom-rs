//! This module contains a mid-level abstraction for reading DICOM content sequentially.
//!
//! The `parser` module is used to obtain DICOM element headers and values. At this level,
//! headers and values are treated as tokens which can be used to form a syntax tree of
//! a full data set.
use dicom_core::dictionary::DataDictionary;
use dicom_core::header::{
    DataElementHeader, Header, Length, PrimitiveDataElement, SequenceItemHeader,
};
use dicom_core::value::{DicomValueType, PrimitiveValue};
use dicom_core::{Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use crate::error::{Error, InvalidValueReadError, Result};
use crate::parser::{DicomParser, DynamicDicomParser, Parse};
use crate::text::SpecificCharacterSet;
use crate::transfer_syntax::Codec;
use crate::util::{ReadSeek, SeekInterval};
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::iter::Iterator;
use std::marker::PhantomData;
use std::ops::DerefMut;

/// A higher-level reader for retrieving structure in a DICOM data set from an
/// arbitrary data source.
#[derive(Debug)]
pub struct DataSetReader<S, P, D> {
    source: S,
    parser: P,
    dict: D,
    /// the current depth in the sequence tree
    depth: u32,
    /// whether the reader is expecting an item next (or a sequence delimiter)
    in_sequence: bool,
    /// fuse the iteration process if true
    hard_break: bool,
    /// last decoded header
    last_header: Option<DataElementHeader>,
}

fn is_parse<S: ?Sized, P>(_: &P)
where
    S: Read,
    P: Parse<S>,
{
}

impl<'s, S: 's> DataSetReader<S, DynamicDicomParser, StandardDataDictionary> {
    /// Creates a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(source: S, ts: &dyn Codec, cs: SpecificCharacterSet) -> Result<Self> {
        let parser = DynamicDicomParser::new_with(ts, cs)?;

        is_parse(&parser);

        Ok(DataSetReader {
            source,
            parser,
            dict: StandardDataDictionary,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            last_header: None,
        })
    }
}

impl<'s, S: 's, D> DataSetReader<S, DynamicDicomParser, D> {
    /// Creates a new iterator with the given random access source and data dictionary,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with_dictionary(
        source: S,
        dict: D,
        ts: &dyn Codec,
        cs: SpecificCharacterSet,
    ) -> Result<Self> {
        let parser = DynamicDicomParser::new_with(ts, cs)?;

        is_parse(&parser);

        Ok(DataSetReader {
            source,
            parser,
            dict,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            last_header: None,
        })
    }
}

impl<S, P> DataSetReader<S, P, StandardDataDictionary>
where
    S: Read,
    P: Parse<Read>,
{
    /// Create a new iterator with the given parser.
    pub fn new(source: S, parser: P) -> Self {
        DataSetReader {
            source,
            parser,
            dict: StandardDataDictionary,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            last_header: None,
        }
    }
}

impl<'s, S: 's, P, D> DataSetReader<S, P, D>
where
    S: Read,
    P: Parse<Read + 's>,
{
    fn read_primitive_element<O>(
        &mut self,
        header: DataElementHeader,
    ) -> Result<PrimitiveDataElement> {
        let v = self.parser.read_value(&mut self.source, &header)?;
        Ok(PrimitiveDataElement::new(header, v))
    }
}

/// A token of a DICOM data set stream. This is part of the interpretation of a
/// data set as a stream of symbols, which may either represent data headers or
/// actual value data.
#[derive(Debug, Clone, PartialEq)]
pub enum DataToken {
    /// A data header of a primitive value.
    ElementHeader(DataElementHeader),
    /// The beginning of a sequence element.
    SequenceStart { tag: Tag, len: Length },
    /// The ending delimiter of a sequence.
    SequenceEnd,
    /// The beginning of a new item in the sequence.
    ItemStart { len: Length },
    /// The ending delimiter of an item.
    ItemEnd,
    /// A primitive data element value.
    PrimitiveValue(PrimitiveValue),
}

impl fmt::Display for DataToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &DataToken::PrimitiveValue(ref v) => {
                write!(f, "PrimitiveValue({:?})", v.value_type())
            }
            other => write!(f, "{:?}", other),
        }
    }
}

impl<'s, S: 's, P, D> Iterator for DataSetReader<S, P, D>
where
    S: Read,
    P: Parse<Read + 's>,
    D: DataDictionary,
{
    type Item = Result<DataToken>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hard_break {
            return None;
        }
        if self.in_sequence {
            match self.parser.decode_item_header(&mut self.source) {
                Ok(header) => match header {
                    SequenceItemHeader::Item { len } => {
                        // entered a new item
                        self.in_sequence = false;
                        Some(Ok(DataToken::ItemStart { len }))
                    }
                    SequenceItemHeader::ItemDelimiter => {
                        // closed an item
                        self.in_sequence = true;
                        Some(Ok(DataToken::ItemEnd))
                    }
                    SequenceItemHeader::SequenceDelimiter => {
                        // closed a sequence
                        self.depth -= 1;
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
            let v = match self.parser.read_value(&mut self.source, &header) {
                Ok(v) => v,
                Err(e) => {
                    self.hard_break = true;
                    self.last_header = None;
                    return Some(Err(e));
                }
            };

            // if it's a Specific Character Set, update the parser immediately.
            if let Some(DataElementHeader {
                tag: Tag(0x0008, 0x0005),
                ..
            }) = self.last_header
            {
                // TODO trigger an error or warning on unsupported specific character sets.
                // Edge case handling strategies should be considered in the future.
                if let Some(charset) = v.string().and_then(SpecificCharacterSet::from_code) {
                    if let Err(e) = self.parser.set_character_set(charset) {
                        self.hard_break = true;
                        self.last_header = None;
                        return Some(Err(e));
                    }
                }
            }
            self.last_header = None;
            Some(Ok(DataToken::PrimitiveValue(v)))
        } else {
            // a data element header or item delimiter is expected
            match self.parser.decode_header(&mut self.source) {
                Ok(DataElementHeader {
                    tag,
                    vr: VR::SQ,
                    len,
                }) => {
                    self.in_sequence = true;
                    self.depth += 1;
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
                    // TODO there might be a better way to check for the end of
                    // a DICOM object. This approach might ignore trailing
                    // garbage.
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

/// An iterator for retrieving DICOM object element markers from a random
/// access data source.
#[derive(Debug)]
pub struct LazyDataSetReader<S, DS, P> {
    source: S,
    parser: P,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
    phantom: PhantomData<DS>,
}

impl<'s> LazyDataSetReader<&'s mut ReadSeek, &'s mut Read, DynamicDicomParser> {
    /// Create a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(
        source: &'s mut ReadSeek,
        ts: &dyn Codec,
        cs: SpecificCharacterSet,
    ) -> Result<Self> {
        let parser = DicomParser::new_with(ts, cs)?;

        Ok(LazyDataSetReader {
            source,
            parser,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            phantom: PhantomData,
        })
    }
}

impl<S, DS, P> LazyDataSetReader<S, DS, P>
where
    S: ReadSeek,
{
    /// Create a new iterator with the given parser.
    pub fn new(source: S, parser: P) -> LazyDataSetReader<S, DS, P> {
        LazyDataSetReader {
            source,
            parser,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            phantom: PhantomData,
        }
    }

    /// Get the inner source's position in the stream using `seek()`.
    fn get_position(&mut self) -> Result<u64>
    where
        S: Seek,
    {
        self.source.seek(SeekFrom::Current(0)).map_err(Error::from)
    }

    fn create_element_marker(&mut self, header: DataElementHeader) -> Result<DicomElementMarker> {
        match self.get_position() {
            Ok(pos) => Ok(DicomElementMarker { header, pos }),
            Err(e) => {
                self.hard_break = true;
                Err(e)
            }
        }
    }

    fn create_item_marker(&mut self, header: SequenceItemHeader) -> Result<DicomElementMarker> {
        match self.get_position() {
            Ok(pos) => Ok(DicomElementMarker {
                header: From::from(header),
                pos,
            }),
            Err(e) => {
                self.hard_break = true;
                Err(e)
            }
        }
    }
}

impl<S, P> Iterator for LazyDataSetReader<S, (), P>
where
    S: ReadSeek,
    P: Parse<S>,
{
    type Item = Result<DicomElementMarker>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hard_break {
            return None;
        }
        if self.in_sequence {
            match self.parser.decode_item_header(&mut self.source) {
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
            match self.parser.decode_header(&mut self.source) {
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
