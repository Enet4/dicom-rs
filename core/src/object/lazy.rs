use super::DicomObject;
use data::dataset::DicomElementMarker;
use data::parser::DynamicDicomParser;
use data::value::Value;
use data::Header;
use data::{DataElement, Length, Tag, VR};
use dictionary::{DataDictionary, DictionaryEntry};
use error::{Error, Result};
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::rc::Rc;
use util::ReadSeek;

/// Data type for a lazily loaded DICOM object builder.
pub struct LazyDataSequence<S, P, D> {
    dict: D,
    source: RefCell<S>,
    parser: P,
    seq: Vec<LazyDataElement>,
}

type LazyObj = DataElement<()>;

impl<S, P, D> Debug for LazyDataSequence<S, P, D>
where
    D: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // ignore parent to avoid cycles
        f.debug_struct("LazyDataSequence")
            .field("entries", &self.seq)
            .finish()
    }
}

/// Data type for a lazily loaded DICOM object builder.
pub struct LazyDicomObject<S, P, D> {
    dict: D,
    source: RefCell<S>,
    parser: P,
    entries: RefCell<HashMap<Tag, LazyDataElement>>,
}

impl<S, P, D> Debug for LazyDicomObject<S, P, D>
where
    P: Debug,
    D: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LazyDicomObject")
            .field("parser", &self.parser)
            .field("entries", &self.entries)
            .finish()
    }
}

impl<'s, S: 's, D: 's> DicomObject for &'s LazyDicomObject<S, DynamicDicomParser, D>
where
    S: ReadSeek,
    D: DataDictionary,
{
    type Element = Ref<'s, LazyDataElement>;

    fn element(&self, tag: Tag) -> Result<Self::Element> {
        {
            let borrow = self.entries.borrow();
            if !borrow.contains_key(&tag) {
                return Err(Error::NoSuchDataElement);
            }
            let e = Ref::map(borrow, |m| m.get(&tag).expect("Element should exist"));
            if e.is_loaded() {
                return Ok(e);
            }
        }
        {
            let mut borrow = self.entries.borrow_mut();
            let e = borrow.get_mut(&tag).expect("Element should exist");
            let v: Value<_> = self.load_value(&e.marker).unwrap();
            let data = e.value_mut();
            unimplemented!() // TODO
        }
        Ok(Ref::map(self.entries.borrow(), |m| {
            m.get(&tag).expect("Element should exist")
        }))
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        let tag = self.lookup_name(name)?;
        self.element(tag)
    }
}

impl<'s, S: 's, D> LazyDicomObject<S, DynamicDicomParser, D>
where
    S: ReadSeek,
    D: DataDictionary,
{
    fn lookup_name(&self, name: &str) -> Result<Tag> {
        self.dict
            .by_name(name)
            .ok_or(Error::NoSuchAttributeName)
            .map(|e| e.tag())
    }

    fn load_value(&self, marker: &DicomElementMarker) -> Result<Value<LazyObj>> {
        let mut borrow = self.source.borrow_mut();
        marker.move_to_start(&mut *borrow)?;
        unimplemented!()
    }
}

/// A data element containing the value only after the first read.
/// This element makes no further assumptions of where the
/// element really comes from, and cannot retrieve the value by itself.
#[derive(Debug, Clone, PartialEq)]
pub struct LazyDataElement {
    marker: DicomElementMarker,
    value: Option<()>,
}

impl Header for LazyDataElement {
    fn tag(&self) -> Tag {
        self.marker.tag()
    }
    fn len(&self) -> Length {
        self.marker.len()
    }
}

impl<'a> Header for &'a LazyDataElement {
    fn tag(&self) -> Tag {
        (**self).tag()
    }
    fn len(&self) -> Length {
        (**self).len()
    }
}

impl<'s> Header for Ref<'s, LazyDataElement> {
    fn tag(&self) -> Tag {
        (**self).tag()
    }
    fn len(&self) -> Length {
        (**self).len()
    }
}

impl Header for Rc<LazyDataElement> {
    fn tag(&self) -> Tag {
        (**self).tag()
    }
    fn len(&self) -> Length {
        (**self).len()
    }
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
    /// According to the standard, this can be undefined,
    /// which can be the case for sequence elements.
    pub fn len(&self) -> Length {
        self.marker.len()
    }

    /// Whether the value data length is known and is exactly zero. 
    pub fn is_empty(&self) -> bool {
        self.marker.len().get() == Some(0)
    }

    // TODO lazy value evaluation
    /// Getter for this element's cached data value.
    /// It will only hold a value once explicitly read.
    pub fn value(&self) -> Option<&()> {
        self.value.as_ref()
    }

    // TODO lazy value evaluation
    /// Mutable getter for this element's cached data container.
    pub fn value_mut(&mut self) -> &mut Option<()> {
        &mut self.value
    }

    /// Check whether the value is locally cached.
    pub fn is_loaded(&self) -> bool {
        self.value.is_some()
    }

    /// Free the cached data container.
    pub fn clear_value(&mut self) {
        self.value = None;
    }
}
