//! This module contains the high-level DICOM abstraction trait.
//! These traits should be preferred when dealing with a variety of DICOM objects.
use data_element::{Header, DataElement, DataElementHeader, SequenceItemHeader};
use data_element::decode::Decode;
use data_element::text::{SpecificCharacterSet, TextCodec};
use std::io::{Read, Seek, SeekFrom};
use std::iter::{Iterator, FromIterator};
use util::SeekInterval;
use parser::DicomParser;
use std::collections::HashMap;
use error::{Result, Error};
use attribute::ValueRepresentation;
use attribute::value::DicomValue;
use transfer_syntax::TransferSyntax;

/// Trait type for a high-level abstraction of DICOM object.
/// At this level, objects are comparable to a lazy dictionary of elements,
/// in which some of them can be DICOM objects themselves.
pub trait DicomObject {

    /// Retrieve a particular DICOM element.
    fn get<T: Into<(u16, u16)>>(&self, tag: T) -> Result<DataElement>;

}

/// An iterator for DICOM object elements.
#[derive(Debug)]
pub struct DicomElementIterator<'s, S: Read + Seek + ?Sized + 's> {
    parser: DicomParser<'s, S>,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
}

impl<'s, S: Read + Seek + ?Sized + 's> DicomElementIterator<'s, S> {
    /// Create a new iterator with the given random access source,
    /// while considering the given decoder and text codec.
    pub fn new(mut source: &'s mut S,
               decoder: Box<Decode<Source = S> + 's>,
               text: Box<TextCodec>)
               -> DicomElementIterator<'s, S> {
        DicomElementIterator {
            parser: DicomParser::new(source, decoder, text),
            depth: 0,
            in_sequence: false,
            hard_break: false,
        }
    }

    /// Create a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(mut source: &'s mut S,
                    ts: TransferSyntax,
                    cs: SpecificCharacterSet)
                    -> Result<DicomElementIterator<'s, S>> {
        let decoder: Box<Decode<Source = S>> = try!(ts.get_decoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax));
        let text = try!(cs.get_codec()
            .ok_or_else(|| Error::UnsupportedCharacterSet));

        Ok(DicomElementIterator {
            parser: DicomParser::new(source, decoder, text),
            depth: 0,
            in_sequence: false,
            hard_break: false,
        })
    }

    fn save_element(&mut self, header: DataElementHeader) -> Result<DicomElementMarker> {
        match self.parser.borrow_source().seek(SeekFrom::Current(0)) {
            Ok(pos) => {
                Ok(DicomElementMarker {
                    header: LazyDicomElementHeader::Data(header),
                    pos: pos,
                })
            }
            Err(e) => {
                self.hard_break = true;
                Err(Error::from(e))
            }
        }
    }

    fn save_item(&mut self, header: SequenceItemHeader) -> Result<DicomElementMarker> {
        match self.parser.borrow_source().seek(SeekFrom::Current(0)) {
            Ok(pos) => {
                Ok(DicomElementMarker {
                    header: LazyDicomElementHeader::Item(header),
                    pos: pos,
                })
            }
            Err(e) => {
                self.hard_break = true;
                Err(Error::from(e))
            }
        }
    }
}

impl<'a, S: Read + Seek + ?Sized + 'a> Iterator for DicomElementIterator<'a, S> {
    type Item = Result<DicomElementMarker>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hard_break {
            return None;
        }

        if self.in_sequence {
            match self.parser.decode_item_header() {
                Ok(header) => {
                    match header {
                        header @ SequenceItemHeader::Item { .. } => {
                            self.in_sequence = false;
                            Some(self.save_item(header))
                        }
                        SequenceItemHeader::ItemDelimiter => {
                            self.in_sequence = true;
                            Some(self.save_item(header))
                        }
                        SequenceItemHeader::SequenceDelimiter => {
                            self.depth -= 1;
                            self.in_sequence = false;
                            Some(self.save_item(header))
                        }
                    }
                }
                Err(e) => {
                    self.hard_break = true;
                    Some(Err(Error::from(e)))
                }
            }

        } else {
            match self.parser.decode_header() {
                Ok(header) => {
                    // check if SQ
                    if header.vr() == ValueRepresentation::SQ {
                        self.in_sequence = true;
                        self.depth += 1;
                    }
                    Some(self.save_element(header))
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
    /// The header, kept in memory.
    header: LazyDicomElementHeader,
    /// The starting position of the element's data value,
    /// relative to the beginning of the file.
    pos: u64,
}

impl DicomElementMarker {
    /// Obtain an interval of the raw data associated to this element's data value.
    pub fn get_data_stream<'s, S: Read + Seek + ?Sized + 's>(&self,
                                                             source: &'s mut S)
                                                             -> Result<SeekInterval<'s, S>> {
        let len = self.header.len();
        let interval = try!(SeekInterval::new(source, len));
        Ok(interval)
    }
}

impl Header for DicomElementMarker {
    fn tag(&self) -> (u16, u16) {
        self.header.tag()
    }

    fn len(&self) -> u32 {
        self.header.len()
    }
}

/// A data type for a DICOM element header.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LazyDicomElementHeader {
    /// A regular data element was read.
    Data(DataElementHeader),
    /// A sequence item was read. Item delimiters and sequence
    /// delimiters also apply to this variant.
    Item(SequenceItemHeader),
}

impl Header for LazyDicomElementHeader {
    fn tag(&self) -> (u16, u16) {
        match *self {
            LazyDicomElementHeader::Data(h) => h.tag(),
            LazyDicomElementHeader::Item(h) => h.tag(),
        }
    }

    fn len(&self) -> u32 {
        match *self {
            LazyDicomElementHeader::Data(h) => h.len(),
            LazyDicomElementHeader::Item(h) => h.len(),
        }
    }
}

/// Data type for a lazily loaded DICOM object builder.
#[derive(Debug)]
pub struct LazyDicomObject<'s, S: Read + Seek + ?Sized + 's> {
    parser: DicomParser<'s, S>,
    entries: HashMap<(u16, u16), LazyDataElement>,
}

impl<'s, S: Read + Seek + ?Sized + 's> LazyDicomObject<'s, S> {

    /// create a new lazy DICOM object from an element marker iterator.
    pub fn from_iter<T>(col: T, parser: DicomParser<'s, S>) -> Result<LazyDicomObject<'s, S>>
        where T: IntoIterator<Item = Result<DicomElementMarker>>
    {
        // create iterator of Result<(tag, LazyDataElement)>
        let iter = col.into_iter().map(|res| (res.map(|e| (e.tag(), LazyDataElement::new(e)))));
        let entries: HashMap<(u16, u16), LazyDataElement> = try!(FromIterator::from_iter(iter));

        Ok(LazyDicomObject {
            parser: parser,
            entries: entries,
        })
    }
}

impl<'s, S: Read + Seek + ?Sized + 's> DicomObject for LazyDicomObject<'s, S> {
    fn get<T: Into<(u16, u16)>>(&self, tag: T) -> Result<DataElement> {
        let tag: (u16, u16) = tag.into();

        // ???
        let value = self.entries.get(&tag).ok_or_else(|| Error::NoSuchDataElement);

        unimplemented!() // TODO
    }
}

#[derive(Debug)]
/// A data element containing the value only after the first read.
/// This element makes no further assumptions of where the
/// element really comes from, and cannot retrieve the value by itself.
pub struct LazyDataElement {
    marker: DicomElementMarker,
    value: Option<DicomValue>,
}

impl LazyDataElement {

    /// Create a new lazy element with the given marker.
    pub fn new(marker: DicomElementMarker) -> LazyDataElement {
        LazyDataElement {
            marker: marker,
            value: None,
        }
    }

    /// Retrieve the element's tag as a `(group, element)` tuple.
    pub fn tag(&self) -> (u16, u16) {
        self.marker.header.tag()
    }

    /// Retrieve the element's value representation, which can be unknown or
    /// not applicable.
    pub fn vr(&self) -> Option<ValueRepresentation> {
        match self.marker.header {
            LazyDicomElementHeader::Data(h) => Some(h.vr()),
            _ => None,
        }
    }

    /// Retrieve the value data's length as specified by the data element.
    /// According to the standard, this can be 0xFFFFFFFFu32 if the length is undefined,
    /// which can be the case for sequence elements.
    pub fn len(&self) -> u32 {
        self.marker.header.len()
    }

    /// Getter for this element's cached data value.
    /// It will only hold a value once explicitly read.
    pub fn value(&self) -> &Option<DicomValue> {
        &self.value
    }

    /// Mutable getter for this element's cached data value.
    pub fn value_mut(&mut self) -> &mut Option<DicomValue> {
        &mut self.value
    }
}
