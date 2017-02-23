//! This module contains a mid-level abstraction for reading DICOM content sequentially.
//!
use std::io::{Read, Seek};
use std::ops::DerefMut;
use parser::DicomParser;
use std::iter::Iterator;
use transfer_syntax::{TransferSyntax, DynamicDecoder, DynamicBasicDecoder};
use data::{Header, DataElementHeader, SequenceItemHeader};
use data::decode::{BasicDecode, Decode};
use data::text::{SpecificCharacterSet, TextCodec};
use util::{ReadSeek, SeekInterval};
use error::{Result, Error};
use attribute::VR;
use attribute::tag::Tag;

/// An iterator for DICOM object elements.
#[derive(Debug)]
pub struct DicomElementIterator<'s, S: ?Sized + 's, D, BD, DS: ?Sized + 's, TC>
    where S: DerefMut<Target = DS> + ReadSeek,
          D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          DS: Read,
          TC: TextCodec
{
    parser: DicomParser<'s, D, BD, S, DS, TC>,
    depth: u32,
    in_sequence: bool,
    hard_break: bool,
}

impl<'s, S: 's + ?Sized, D, BD> DicomElementIterator<'s, S, D, BD, (Read + 's), Box<TextCodec>>
    where S: DerefMut<Target = (Read + 's)> + ReadSeek,
          D: Decode<Source = (Read + 's)>,
          BD: BasicDecode<Source = (Read + 's)>
{
    /// Create a new iterator with the given random access source,
    /// while considering the given transfer syntax and specific character set.
    pub fn new_with(mut source: &'s mut S, ts: &TransferSyntax, cs: SpecificCharacterSet)
         -> Result<DicomElementIterator<'s, S, DynamicDecoder<'s>, DynamicBasicDecoder<'s>, (Read + 's), Box<TextCodec>>> {
        let basic = ts.get_basic_decoder();
        let decoder = try!(ts.get_decoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax));
        let text = cs.get_codec().ok_or_else(|| Error::UnsupportedCharacterSet)?;

        Ok(DicomElementIterator {
            parser: DicomParser::new(source, decoder, basic, text),
            depth: 0,
            in_sequence: false,
            hard_break: false,
        })
    }
}

impl<'s, S: ?Sized + 's, D, BD, DS: ?Sized + 's, TC> DicomElementIterator<'s, S, D, BD, DS, TC>
    where S: DerefMut<Target = DS> + ReadSeek,
          D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          DS: Read,
          TC: TextCodec
{
    /// Create a new iterator with the given random access source,
    /// while considering the given decoder and text codec.
    pub fn new(mut source: &'s mut S,
               decoder: D,
               basic: BD,
               text: TC)
               -> DicomElementIterator<'s, S, D, BD, DS, TC> {
        DicomElementIterator {
            parser: DicomParser::new(source, decoder, basic, text),
            depth: 0,
            in_sequence: false,
            hard_break: false,
        }
    }

    fn create_element_marker(&mut self, header: DataElementHeader) -> Result<DicomElementMarker> {
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

    fn create_item_marker(&mut self, header: SequenceItemHeader) -> Result<DicomElementMarker> {
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

impl<'s, D, S: ?Sized + 's, BD, DS: ?Sized + Read + 's, TC> Iterator for DicomElementIterator<'s, S, D, BD, DS, TC>
    where S: DerefMut<Target = DS> + ReadSeek,
          D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          DS: Read,
          TC: TextCodec
{
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
    header: DataElementHeader,
    /// The starting position of the element's data value,
    /// relative to the beginning of the file.
    pos: u64,
}

impl DicomElementMarker {
    /// Obtain an interval of the raw data associated to this element's data value.
    pub fn get_data_stream<'s, S: ?Sized + 's>(&self,
                                               source: &'s mut S)
                                               -> Result<SeekInterval<'s, S>>
        where S: Read + Seek
    {
        let len = self.header.len();
        let interval = try!(SeekInterval::new(source, len));
        Ok(interval)
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
