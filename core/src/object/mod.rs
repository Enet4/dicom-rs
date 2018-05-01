//! This module contains the high-level DICOM abstraction trait.
//! At this level, objects are comparable to a lazy dictionary of elements,
//! in which some of them can be DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM objects.
//!
//! # Examples
//!
//! Fetching an element by tag:
//!
//! ```
//! # use dicom_core::object::DicomObject;
//! # use dicom_core::error::Result;
//! use dicom_core::data::Tag;
//! # fn something<T: DicomObject>(obj: T) -> Result<()> {
//! let e = obj.element(Tag(0x0002, 0x0002))?;
//! # Ok(())
//! # }
//! ```
//!
use data::Header;
use data::Tag;
use error::Result;

pub mod lazy;
pub mod mem;
pub mod pixeldata;

/// Trait type for a DICOM object.
/// This is a high-level abstraction where an object is accessed and
/// manipulated as dictionary of entries indexed by tags, which in
/// turn may contain a DICOM object.
///
/// This trait interface is experimental and prone to sudden changes.
pub trait DicomObject {
    type Element: Header; // TODO change constraint
    type Sequence; // TODO add constraint

    /// Retrieve a particular DICOM element by its tag.
    fn element(&self, tag: Tag) -> Result<Self::Element>;

    /// Retrieve a particular DICOM element by its name.
    fn element_by_name(&self, name: &str) -> Result<Self::Element>;

    // TODO moar
}

/** A generic DICOM sequence of objects of type `T`. */
#[derive(Debug, Clone, PartialEq)]
pub struct DicomSequence<T> {
    tag: Tag,
    len: u32,
    objects: Vec<T>,
}

impl<T> DicomSequence<T> {
    pub fn new(tag: Tag, len: u32, objects: Vec<T>) -> Result<Self> {
        Ok(DicomSequence { tag, len, objects })
    }

    pub fn tag(&self) -> Tag {
        self.tag
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn objects(&self) -> &[T] {
        &self.objects
    }

    pub  fn objects_mut(&mut self) -> &mut Vec<T> {
        &mut self.objects
    }
}
