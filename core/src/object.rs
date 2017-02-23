//! This module contains the high-level DICOM abstraction trait.
//! At this level, objects are comparable to a lazy dictionary of elements,
//! in which some of them can be DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM objects.
use std::ops::DerefMut;
use std::io::Seek;
use std::iter::Iterator;
use std::fmt::Debug;
use std::fmt;
use data::Header;
use data::decode::{BasicDecode, Decode};
use parser::DicomParser;
use data::text::TextCodec;
use std::collections::HashMap;
use error::{Result, Error};
use attribute::VR;
use attribute::tag::Tag;
use attribute::value::DicomValue;
use iterator::DicomElementMarker;
use util::ReadSeek;

/// An enum type for an entry reference to an object, which can be
/// a primitive element or another complex value.
#[derive(Debug)]
pub enum ObjectEntry<'a> {
    //    type Item: ;
    //    type SequenceIt: Iterator;
    Value(&'a DicomValue),
    Item(&'a DicomObject), //    Sequence(Box<Iterator<Item=Self::Item> + 'a>),
}

/// An enum type for an entry reference to an object, which can be
/// a primitive element or another complex value.
#[derive(Debug)]
pub enum ObjectEntryMut<'a> {
    //    type Item: ;
    //    type SequenceIt: Iterator;
    Element(&'a mut DicomValue),
    Item(&'a mut DicomObject), //    Sequence(Box<Iterator<Item=Self::Item> + 'a>),
}

/// Trait type for a DICOM object.
/// This is a high-level abstraction where an object is accessed and
/// manipulated as dictionary of entries indexed by tags, which in
/// turn may contain a DICOM object.
///
pub trait DicomObject: Debug {
    /// Retrieve a particular DICOM element.
    fn get<'a>(&'a mut self, tag: Tag) -> Result<ObjectEntry<'a>>;
}

/// Data type for a lazily loaded DICOM object builder.
pub struct LazyDicomObject<'s, D, BD, S: ?Sized + 's, DS: ?Sized + 's, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + ReadSeek,
          DS: ReadSeek,
          TC: TextCodec
{
    parser: DicomParser<'s, D, BD, S, DS, TC>,
    entries: HashMap<Tag, LazyDataElement>,
}

impl<'s, D, BD, S: ?Sized + 's, DS: ?Sized + 's, TC> Debug for LazyDicomObject<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + ReadSeek,
          DS: ReadSeek,
          TC: TextCodec
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "LazyDicomObject{{parser: {:?}, entries{:?}}}",
               &self.parser,
               &self.entries)
    }
}

impl<'s, D, BD, S: ?Sized + 's, DS: ?Sized + 's, TC> LazyDicomObject<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + ReadSeek,
          DS: ReadSeek,
          TC: TextCodec
{
    /// create a new lazy DICOM object from an element marker iterator.
    pub fn from_iter<T>(iter: T,
                        parser: DicomParser<'s, D, BD, S, DS, TC>)
                        -> Result<LazyDicomObject<'s, D, BD, S, DS, TC>>
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

impl<'s, D, BD, S: ?Sized + 's, DS: ?Sized, TC> DicomObject for LazyDicomObject<'s, D, BD, S, DS, TC>
    where D: Decode<Source = DS>,
          BD: BasicDecode<Source = DS>,
          S: DerefMut<Target = DS> + ReadSeek,
          DS: ReadSeek,
          TC: TextCodec
{

    fn get(&mut self, tag: Tag) -> Result<ObjectEntry> {

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
        self.marker.tag()
    }

    /// Retrieve the element's value representation, which can be unknown if
    /// not applicable.
    pub fn vr(&self) -> VR {
        self.marker.vr()
    }

    /// Retrieve the value data's length as specified by the data element.
    /// According to the standard, this can be 0xFFFFFFFFu32 if the length is undefined,
    /// which can be the case for sequence elements.
    pub fn len(&self) -> u32 {
        self.marker.len()
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
