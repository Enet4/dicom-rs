//! This module contains a mid-level abstraction for reading DICOM content sequentially.
//!
use std::io::{Read, Seek, SeekFrom};
use std::iter::FromIterator;
use std::ops::DerefMut;
use std::marker::PhantomData;
use std::iter::Iterator;
use transfer_syntax::TransferSyntax;
use dictionary::{standard_dictionary, DataDictionary, DictionaryEntry, StandardDataDictionary};
use data::{DataElement, DataElementHeader, Header, SequenceItemHeader};
use data::parser::{DicomParser, DynamicDicomParser, Parse};
use data::text::SpecificCharacterSet;
use data::value::{Value, PrimitiveValue};
use util::{ReadSeek, SeekInterval};
use error::{Error, Result};
use object::mem::InMemDicomObject;
use data::VR;
use data::Tag;

/// An iterator for retrieving DICOM object element markers from a random
/// access data source.
#[derive(Debug)]
pub struct DicomElementIterator<S, P, D> {
    source: S,
    parser: P,
    dict: D,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
}

type InMemElement<D> = DataElement<InMemDicomObject<D>>;

fn is_parse<S: ?Sized + Read, P>(_: &P)
where
    P: Parse<S>,
{
}

impl<'s, S: 's> DicomElementIterator<S, DynamicDicomParser, &'static StandardDataDictionary> {
    /// Creates a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(source: S, ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self> {
        let parser = DynamicDicomParser::new_with(ts, cs)?;

        is_parse(&parser);

        Ok(DicomElementIterator {
            source: source,
            parser: parser,
            dict: standard_dictionary(),
            depth: 0,
            in_sequence: false,
            hard_break: false,
        })
    }
}

impl<'s, S: 's, D> DicomElementIterator<S, DynamicDicomParser, D> {
    /// Creates a new iterator with the given random access source and data dictionary,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with_dictionary(source: S, dict: D, ts: &TransferSyntax, cs: SpecificCharacterSet) -> Result<Self> {
        let parser = DynamicDicomParser::new_with(ts, cs)?;

        is_parse(&parser);

        Ok(DicomElementIterator {
            source: source,
            parser: parser,
            dict,
            depth: 0,
            in_sequence: false,
            hard_break: false,
        })
    }
}

impl<S, P> DicomElementIterator<S, P, &'static StandardDataDictionary>
where
    S: Read,
    P: Parse<Read>,
{
    /// Create a new iterator with the given parser.
    pub fn new(source: S, parser: P) -> Self {
        DicomElementIterator {
            source: source,
            parser: parser,
            dict: standard_dictionary(),
            depth: 0,
            in_sequence: false,
            hard_break: false,
        }
    }
}

impl<'s, S: 's, P, D> DicomElementIterator<S, P, D>
where
    S: Read,
    P: Parse<Read + 's>,
{
    fn read_element(&mut self, header: DataElementHeader) -> Result<InMemElement<D>> {
        let value = self.parser.read_value(&mut self.source, &header)?.into();
        Ok(DataElement { header, value })
    }

    fn collect_item<T>(&mut self, header: &SequenceItemHeader) -> Result<T>
    where
        T: FromIterator<Result<InMemElement<D>>>,
    {
        unimplemented!()
    }

    fn create_item_marker(&mut self, header: SequenceItemHeader) -> Result<InMemElement<D>> {
        Ok(DataElement {
            header: header.into(),
            value: PrimitiveValue::Empty.into(),
        })
    }
}

impl<'s, S: 's, P, D> Iterator for DicomElementIterator<S, P, D>
where
    S: Read,
    P: Parse<Read + 's>,
    D: DataDictionary,
{
    type Item = Result<InMemElement<D>>;

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
                    Some(Err(Error::from(e)))
                }
            }
        } else {
            match self.parser.decode_header(&mut self.source) {
                Ok(header @ DataElementHeader {
                    tag: Tag(0x0008, 0x0005),
                    vr: _,
                    len: _,
                }) => {
                    // (0008, 0005) Specific Character Set
                    // Save it for subsequent text decoding.
                    let marker = self.read_element(header);
                    // TODO
                    Some(marker)
                }
                Ok(header) => {
                    // check if SQ
                    if header.vr() == VR::SQ {
                        self.in_sequence = true;
                        self.depth += 1;
                        unimplemented!("reading SQ items with an in-mem iterator")
                    } else {
                        Some(self.read_element(header).into())
                    }
                }
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(Error::from(e)))
                }
            }
        }
    }
}

/// An iterator for retrieving DICOM object element markers from a random
/// access data source.
#[derive(Debug)]
pub struct LazyDicomElementIterator<S, DS, P> {
    source: S,
    parser: P,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
    phantom: PhantomData<DS>,
}

impl<'s> LazyDicomElementIterator<&'s mut ReadSeek, &'s mut Read, DynamicDicomParser> {
    /// Create a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(
        source: &'s mut ReadSeek,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<Self> {
        let parser = DicomParser::new_with(ts, cs)?;

        Ok(LazyDicomElementIterator {
            source: source,
            parser: parser,
            depth: 0,
            in_sequence: false,
            hard_break: false,
            phantom: PhantomData,
        })
    }
}

impl<S, DS, P> LazyDicomElementIterator<S, DS, P>
where
    S: ReadSeek,
{
    /// Create a new iterator with the given parser.
    pub fn new(source: S, parser: P) -> LazyDicomElementIterator<S, DS, P> {
        LazyDicomElementIterator {
            source: source,
            parser: parser,
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
            Ok(pos) => Ok(DicomElementMarker {
                header: header,
                pos: pos,
            }),
            Err(e) => {
                self.hard_break = true;
                Err(Error::from(e))
            }
        }
    }

    fn create_item_marker(&mut self, header: SequenceItemHeader) -> Result<DicomElementMarker> {
        match self.get_position() {
            Ok(pos) => Ok(DicomElementMarker {
                header: From::from(header),
                pos: pos,
            }),
            Err(e) => {
                self.hard_break = true;
                Err(Error::from(e))
            }
        }
    }
}

impl<S, P> Iterator for LazyDicomElementIterator<S, (), P>
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
                    Some(Err(Error::from(e)))
                }
            }
        } else {
            match self.parser.decode_header(&mut self.source) {
                Ok(header @ DataElementHeader {
                    tag: Tag(0x0008, 0x0005),
                    vr: _,
                    len: _,
                }) => {
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
                    Some(Err(Error::from(e)))
                }
            }
        }
    }
}

/// A data type for a DICOM element residing in a file,
/// or any other source with random access. A position
/// in the file is kept for future
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct DicomElementMarker {
    /// The header, kept in memory. At this level, the value
    /// representation "UN" may also refer to a non-applicable vr
    /// (i.e. for items and delimiters).
    pub header: DataElementHeader,
    /// The starting position of the element's data value,
    /// relative to the beginning of the file.
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
        let len = self.header.len() as u64;
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

    fn len(&self) -> u32 {
        self.header.len()
    }
}
