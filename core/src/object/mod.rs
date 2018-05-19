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
use meta::DicomMetaTable;

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

    /// Retrieve a particular DICOM element by its tag.
    fn element(&self, tag: Tag) -> Result<Self::Element>;

    /// Retrieve a particular DICOM element by its name.
    fn element_by_name(&self, name: &str) -> Result<Self::Element>;

    // TODO moar
}


/** A root DICOM object contains additional meta information about the object
 * (such as the DICOM file's meta header).
 */
#[derive(Debug, Clone, PartialEq)]
pub struct RootDicomObject<T> {
    meta: DicomMetaTable,
    obj: T,
}

impl<T> RootDicomObject<T> {
    /// Retrieve the processed meta header table.
    pub fn meta(&self) -> &DicomMetaTable {
        &self.meta
    }
}

impl<T> ::std::ops::Deref for RootDicomObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<T> ::std::ops::DerefMut for RootDicomObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}

impl<T> DicomObject for RootDicomObject<T>
where
    T: DicomObject
{
    type Element = <T as DicomObject>::Element;
    
    fn element(&self, tag: Tag) -> Result<Self::Element> {
        self.obj.element(tag)
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        self.obj.element_by_name(name)
    }
}

impl<'a, T: 'a> DicomObject for &'a RootDicomObject<T>
where
    T: DicomObject
{
    type Element = <T as DicomObject>::Element;
    
    fn element(&self, tag: Tag) -> Result<Self::Element> {
        self.obj.element(tag)
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        self.obj.element_by_name(name)
    }
}
