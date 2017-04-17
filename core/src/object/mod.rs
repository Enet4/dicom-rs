//! This module contains the high-level DICOM abstraction trait.
//! At this level, objects are comparable to a lazy dictionary of elements,
//! in which some of them can be DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM objects.
use data::Header;
use error::Result;
use data::Tag;

pub mod mem;
pub mod lazy;
pub mod pixeldata;

use self::pixeldata::PixelData;

/// Trait type for a DICOM object.
/// This is a high-level abstraction where an object is accessed and
/// manipulated as dictionary of entries indexed by tags, which in
/// turn may contain a DICOM object.
///
pub trait DicomObject {
    type Element: Header; // TODO change constraint
    type Sequence; // TODO add constraint

    /// Retrieve a particular DICOM element by its tag.
    fn element(&self, tag: Tag) -> Result<&Self::Element>;

    /// Retrieve a particular DICOM element by its name.
    fn element_by_name(&self, name: &str) -> Result<&Self::Element>;

    /// Retrieve the object's pixel data.
    fn pixel_data<PV, PX: PixelData<PV>>(&self) -> Result<PX>;

    // TODO moar
}
