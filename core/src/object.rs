//! This module contains the high-level DICOM abstraction trait.
//! At this level, objects are comparable to a lazy dictionary of elements,
//! in which some of them can be DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM objects.
use std::ops::DerefMut;
use transfer_syntax::TransferSyntax;
use data_element::{Header, DataElementHeader, SequenceItemHeader};
use data_element::decode::Decode;
use data_element::text::{SpecificCharacterSet, TextCodec};
use std::io::{Read, Seek};
use std::iter::Iterator;
use std::fmt::Debug;
use std::fmt;
use util::SeekInterval;
use parser::DicomParser;
use std::collections::HashMap;
use error::{Result, Error};
use attribute::ValueRepresentation;
use attribute::tag::Tag;
use attribute::value::DicomValue;
use util::ReadSeek;

/// An enum type for an entry reference to an object, which can be
/// a primitive element or another complex value.
#[derive(Debug)]
pub enum ObjectEntryValue<'a> {
    //    type Item: ;
    //    type SequenceIt: Iterator;
    Element(&'a DicomValue),
    Item(&'a DicomObject), //    Sequence(Box<Iterator<Item=Self::Item> + 'a>),
}

/// Trait type for a high-level abstraction of DICOM object.
pub trait DicomObject: Debug {
    /// Retrieve a particular DICOM element.
    fn get(&mut self, tag: Tag) -> Result<ObjectEntryValue>;
}

/// Trait type for a high-level abstraction of DICOM object.
///
/// This trait is for DICOM objects that are already in memory, and
/// so do not require state mutations when getting its elements.
pub trait LoadedDicomObject: Debug {
    /// Retrieve a particular DICOM element.
    fn get(&self, tag: Tag) -> Result<ObjectEntryValue>;
}

/// An iterator for DICOM object elements.
#[derive(Debug)]
pub struct DicomElementIterator<'s,
                                S: Read + ?Sized + 's,
                                DS: Read + ?Sized + 's>
    where S: DerefMut<Target = DS> {
    parser: DicomParser<'s, S, DS>,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
}

impl<'s, S: ?Sized + ReadSeek + 's> DicomElementIterator<'s, S, Read + 's>
    where S: DerefMut<Target = (Read + 's)> {

    /// Create a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(mut source: &'s mut S,
                        ts: &TransferSyntax,
                        cs: SpecificCharacterSet)
                        -> Result<DicomElementIterator<'s, S, (Read + 's)>> {
            let decoder: Box<Decode<Source = (Read + 's)> + 's> = try!(ts.get_decoder()
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
}

impl<'s, S: Read + Seek + ?Sized + 's, DS: Read + ?Sized + 's> DicomElementIterator<'s, S, DS>
    where S: DerefMut<Target = DS> {
    
    /// Create a new iterator with the given random access source,
    /// while considering the given decoder and text codec.
    pub fn new(mut source: &'s mut S,
               decoder: Box<Decode<Source = DS> + 's>,
               text: Box<TextCodec>)
               -> DicomElementIterator<'s, S, DS> {
        DicomElementIterator {
            parser: DicomParser::new(source, decoder, text),
            depth: 0,
            in_sequence: false,
            hard_break: false,
        }
    }

    fn save_element(&mut self, header: DataElementHeader) -> Result<DicomElementMarker> {
        match self.parser.get_position() {
            Ok(pos) => {
                Ok(DicomElementMarker {
                    header: header,
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
        match self.parser.get_position() {
            Ok(pos) => {
                Ok(DicomElementMarker {
                    header: From::from(header),
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

impl<'a, S: ?Sized + Read + Seek + 'a, DS: ?Sized + Read + 'a> Iterator for DicomElementIterator<'a, S, DS>
    where S: DerefMut<Target = DS> {
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
    /// The header, kept in memory. At this level, the value
    /// representation "UN" may also refer to a non-applicable vr
    /// (i.e. for items and delimiters).
    header: DataElementHeader,
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
    fn tag(&self) -> Tag {
        self.header.tag()
    }

    fn len(&self) -> u32 {
        self.header.len()
    }
}

/// Data type for a lazily loaded DICOM object builder.
pub struct LazyDicomObject<'s, S: Read + Seek + ?Sized + 's, DS: Read + Seek + ?Sized + 's>
    where S: DerefMut<Target = DS> {
    parser: DicomParser<'s, S, DS>,
    entries: HashMap<Tag, LazyDataElement>,
}

impl<'s, S: Read + Seek + ?Sized + 's, DS: Read + Seek + ?Sized + 's> Debug for LazyDicomObject<'s, S, DS>
    where S: DerefMut<Target = DS> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "LazyDicomObject{{parser: {:?}, entries{:?}}}",
               &self.parser,
               &self.entries)
    }
}

impl<'s, S: Read + Seek + ?Sized + 's, DS: Read + Seek + ?Sized + 's> LazyDicomObject<'s, S, DS>
    where S: DerefMut<Target = DS> {

    /// create a new lazy DICOM object from an element marker iterator.
    pub fn from_iter<T>(iter: T,
                        parser: DicomParser<'s, S, DS>)
                        -> Result<LazyDicomObject<'s, S, DS>>
        where T: IntoIterator<Item = Result<DicomElementMarker>>
    {
        // collect results into a hash map
        let entries = try!(iter.into_iter()
            .map(|res| res.map(|e| (e.tag(), LazyDataElement::new(e))))
            .collect());

        Ok(LazyDicomObject {
            parser: parser,
            entries: entries,
        })
    }
}

impl<'s, S: Read + Seek + ?Sized + 's, DS: Read + Seek + ?Sized> DicomObject for LazyDicomObject<'s, S, DS>
    where S: DerefMut<Target = DS> {

    fn get(&mut self, tag: Tag) -> Result<ObjectEntryValue> {

        let mut e = try!(self.entries.get_mut(&tag).ok_or_else(|| Error::NoSuchDataElement));

        // TODO

        unimplemented!()
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
    pub fn tag(&self) -> Tag {
        self.marker.header.tag()
    }

    /// Retrieve the element's value representation, which can be unknown if
    /// not applicable.
    pub fn vr(&self) -> ValueRepresentation {
        self.marker.header.vr()
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
