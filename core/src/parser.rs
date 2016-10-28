//! This module provides a higher level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use std::io::{Read, Seek, SeekFrom};
use std::iter::{repeat, Iterator};
use error::{Result, Error};
use data_element::{DataElementHeader, SequenceItemHeader};
use data_element::decode::Decode;
use data_element::text::SpecificCharacterSet;
use transfer_syntax::TransferSyntax;
use data_element::text;
use attribute::ValueRepresentation;
use attribute::value::DicomValue;
use std::iter::FromIterator;
use object::DicomObject;
use util::SeekInterval;
use std::collections::HashMap;

/// An iterator for DICOM object elements.
#[derive(Debug)]
pub struct DicomElementIterator<'s, S: Read + Seek + ?Sized + 's> {
    source: &'s mut S,
    decoder: Box<Decode<Source = S> + 's>,
    text: Box<text::TextCodec>,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
}

impl<'s, S: Read + Seek + ?Sized + 's> DicomElementIterator<'s, S> {
    /// Create a new iterator with the given random access source,
    /// while considering the given decoder and text codec.
    pub fn new(mut source: &'s mut S,
               decoder: Box<Decode<Source = S> + 's>,
               text: Box<text::TextCodec>)
               -> DicomElementIterator<'s, S> {
        DicomElementIterator {
            source: source,
            decoder: decoder,
            text: text,
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
            source: source,
            decoder: decoder,
            text: text,
            depth: 0,
            in_sequence: false,
            hard_break: false,
        })
    }

    fn save_element(&mut self, header: DataElementHeader) -> Result<DicomElementMarker> {
        match self.source.seek(SeekFrom::Current(0)) {
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
        match self.source.seek(SeekFrom::Current(0)) {
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
            match self.decoder.decode_item_header(&mut self.source) {
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
            match self.decoder.decode_header(&mut self.source) {
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
    pub fn get_data_stream<'s, S: Read + Seek + ?Sized + 's>(&self, source: &'s mut S) -> Result<SeekInterval<'s, S>> {
        let len = self.header.len();
        let interval = try!(SeekInterval::new(source, len));
        Ok(interval)
    }

    /// Getter for this element's tag.
    pub fn tag(&self) -> (u16, u16) {
        self.header.tag()
    }

    /// Getter for this element's length.
    pub fn len(&self) -> u32 {
        self.header.len()
    } 
}

/// A data type for a DICOM element header.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LazyDicomElementHeader {
    /// A regular data element was read.
    Data (DataElementHeader),
    /// A sequence item was read. Item delimiters and sequence
    /// delimiters also apply to this variant.
    Item (SequenceItemHeader),
}

impl LazyDicomElementHeader {

    /// Getter for this element's attribute tag.
    pub fn tag(&self) -> (u16, u16) {
        match *self {
            LazyDicomElementHeader::Data(h) => h.tag(),
            LazyDicomElementHeader::Item(h) => h.tag()
        }
    }

    /// Getter for this element's length, as specified in
    /// the DICOM element (can be 0xFFFFFFFF).
    pub fn len(&self) -> u32 {
        match *self {
            LazyDicomElementHeader::Data(h) => h.len(),
            LazyDicomElementHeader::Item(h) => h.len()
        }
    }
}

/// Data type for a lazily loaded DICOM object builder.
#[derive(Debug)]
pub struct LazyDicomObject<'s, S: Read + Seek + ?Sized + 's> {
    source: &'s mut S,
    decoder: Box<Decode<Source = S> + 's>,
    text: Box<text::TextCodec>,
    entries: HashMap<(u16, u16), LazyDataElement>,
}

impl<'s, S: Read + Seek + ?Sized + 's> LazyDicomObject<'s, S> {

    /// Eagerly read and cache the data value.
    fn read_value<'a>(&mut self, elem: &'a mut LazyDataElement) -> Result<&'a DicomValue> {
        if let LazyDicomElementHeader::Data(ref header) = elem.marker.header {
            let value = match header.vr() {
                ValueRepresentation::SQ => {
                    // sequence objects... should not work
                    unimplemented!()
                }
                ValueRepresentation::AT => {
                    // tags
                    //try!(self.source.read_exact(&mut buf));
                    let ntags = {header.len() >> 2} as usize;
                    let parts: Box<[(u16, u16)]> = try!(repeat(()).take(ntags)
                        .map(|_| self.decoder.decode_tag(self.source))
                        .collect::<Result<Vec<(u16, u16)>>>())
                        .into_boxed_slice();
                    DicomValue::Tags(parts)
                }
                ValueRepresentation::AE |
                ValueRepresentation::AS |
                ValueRepresentation::PN |
                ValueRepresentation::SH |
                ValueRepresentation::LO |
                ValueRepresentation::UI |
                ValueRepresentation::UC |
                ValueRepresentation::CS => {
                    // sequence of strings
                    let mut buf = vec![0u8 ; header.len() as usize];
                    try!(self.source.read_exact(&mut buf));
                    let parts: Box<[String]> = try!(buf[..]
                        .split(|v| *v == '\\' as u8)
                        .map(|slice| self.text.as_ref().decode(slice))
                        .collect::<Result<Vec<String>>>()).into_boxed_slice();

                    DicomValue::Strs(parts)
                }
                ValueRepresentation::UT |
                ValueRepresentation::ST |
                ValueRepresentation::UR |
                ValueRepresentation::LT => {
                    // a single string
                    let mut buf = vec![0u8 ; header.len() as usize];
                    try!(self.source.read_exact(&mut buf));
                    DicomValue::Str(try!(self.text.as_ref().decode(&buf[..])))
                }
                ValueRepresentation::UN |
                ValueRepresentation::OB => {
                    // sequence of 8-bit integers (or just byte data)
                    let mut buf = vec![0u8 ; header.len() as usize];
                    try!(self.source.read_exact(&mut buf));
                    DicomValue::U8(buf.into_boxed_slice())
                }
                ValueRepresentation::DA => {
                    // sequence of dates
                    unimplemented!()
                }
                ValueRepresentation::DT => {
                    // sequence of datetimes
                    unimplemented!()
                }
                ValueRepresentation::TM => {
                    // sequence of time instances
                    unimplemented!()
                }
                ValueRepresentation::DS |
                ValueRepresentation::FD |
                ValueRepresentation::OD => {
                    // sequence of 64-bit floats
                    unimplemented!()
                }
                ValueRepresentation::FL |
                ValueRepresentation::OF => {
                    // sequence of 32-bit floats
                    unimplemented!()
                }
                ValueRepresentation::IS |
                ValueRepresentation::SL => {
                    // sequence of 32-bit integers
                    unimplemented!()
                }
                ValueRepresentation::OL |
                ValueRepresentation::OW |
                ValueRepresentation::SS |
                ValueRepresentation::UL |
                ValueRepresentation::US  => {
                    unimplemented!()
                }
            };
            
            elem.value = Some(value);
            Ok(elem.value.as_ref().unwrap())
        } else {
            panic!("Nope!")
        }
    }

    fn read_value_preserved(&mut self, elem: &mut LazyDataElement) -> Result<()> {
        if let LazyDicomElementHeader::Data(ref header) = elem.marker.header {

            unimplemented!(); // TODO
        }

        panic!("Nope!");
    }
}

impl<'s, S: Read + Seek + ?Sized + 's> LazyDicomObject<'s, S> {
    fn from<T>(iter: T) -> Result<LazyDicomObject<'s, S>> where T: IntoIterator<Item=Result<DicomElementMarker>> {
        let entries = HashMap::<(u16, u16), LazyDataElement>::new();
        for e in iter {

        }
        unimplemented!(); // TODO
    }
}

impl<'s, S: Read + Seek + ?Sized + 's> DicomObject for LazyDicomObject<'s, S> {
    fn get<T: Into<Option<(u16, u16)>>>(&self, tag: T) -> Result<(DataElementHeader, DicomValue)> {
        let tag: (u16, u16) = try!(tag.into().ok_or(Error::NoSuchAttributeName));

        // ???
        let value = self.entries.get(&tag).ok_or_else(|| Error::NoSuchDataElement);

        unimplemented!() // TODO
    }
}

#[derive(Debug)]
pub struct LazyDataElement {
    marker: DicomElementMarker,
    value: Option<DicomValue>,
}

impl LazyDataElement {

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
    pub fn value(&self) -> &Option<DicomValue> {
        &self.value
    }

    /// Mutable getter for this element's cached data value.
    pub fn value_mut(&mut self) -> &mut Option<DicomValue> {
        &mut self.value
    }
}
