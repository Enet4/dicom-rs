//! This module provides a mid level abstraction for reading DICOM data.
//! The structures provided here can translate a byte data source into
//! an iterator of elements, with either sequential or random access.

use std::io::{Read, Seek, SeekFrom};
use std::iter::Iterator;
use error::{Result, Error};
use data_element::{DataElement, DataElementHeader, SequenceItemHeader};
use data_element::decode::Decode;
use data_element::text::SpecificCharacterSet;
use transfer_syntax::TransferSyntax;
use data_element::text;
use attribute::ValueRepresentation;

/// An iterator for DICOM object elements.
///
#[derive(Debug)]
pub struct DicomObjectIterator<'s, S: Read + Seek + ?Sized + 's> {
    source: &'s mut S,
    decoder: Box<Decode<Source = S> + 's>,
    text: Box<text::TextCodec>,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
}

impl<'s, S: Read + Seek + ?Sized + 's> DicomObjectIterator<'s, S> {
    /// Create a new iterator with the given random access source,
    /// while considering the given decoder and text codec.
    pub fn new(mut source: &'s mut S,
               decoder: Box<Decode<Source = S> + 's>,
               text: Box<text::TextCodec>)
               -> DicomObjectIterator<'s, S> {
        DicomObjectIterator {
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
                    -> Result<DicomObjectIterator<'s, S>> {
        let decoder: Box<Decode<Source = S>> = try!(ts.get_decoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax));
        let text = try!(cs.get_codec().ok_or_else(|| Error::UnsupportedCharacterSet));

        Ok(DicomObjectIterator {
            source: source,
            decoder: decoder,
            text: text,
            depth: 0,
            in_sequence: false,
            hard_break: false,
        })
    }

    fn save_element(&mut self, header: DataElementHeader) -> Result<LazyDicomElement> {
        match self.source.seek(SeekFrom::Current(0)) {
            Ok(pos) => {
                Ok(LazyDicomElement::DataElement {
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

    fn save_item(&mut self, header: SequenceItemHeader) -> Result<LazyDicomElement> {
        match self.source.seek(SeekFrom::Current(0)) {
            Ok(pos) => {
                Ok(LazyDicomElement::SequenceItem {
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
}

impl<'a, S: Read + Seek + ?Sized + 'a> Iterator for DicomObjectIterator<'a, S> {
    type Item = Result<LazyDicomElement>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hard_break {
            return None;
        }

        if self.in_sequence {
            match self.decoder.decode_item(&mut self.source) {
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
            match self.decoder.decode(&mut self.source) {
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
/// or any other source with random access.
///
/// WIP
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LazyDicomElement {
    /// A regular data element was read.
    DataElement {
        /// The header, kept in memory.
        header: DataElementHeader,
        /// The starting position of the element's data value,
        /// relative to the beginning of the file.
        pos: u64,
    },
    /// A sequence item was read. Item delimiters and sequence
    /// delimiters also apply to this variant.
    SequenceItem {
        /// The header, kept in memory.
        header: SequenceItemHeader,
        /// The starting position of the element's data value,
        /// relative to the beginning of the file.
        pos: u64,
    },
}
