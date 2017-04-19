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
pub trait DicomObject<'s> {
    type Element: 's + Header; // TODO change constraint
    type Sequence: 's; // TODO add constraint

    /// Retrieve a particular DICOM element by its tag.
    fn element(&'s self, tag: Tag) -> Result<Self::Element>;

    /// Retrieve a particular DICOM element by its name.
    fn element_by_name(&'s self, name: &str) -> Result<Self::Element>;

    /// Retrieve the object's pixel data.
    fn pixel_data<PV, PX: PixelData<PV>>(&'s self) -> Result<PX>;

    // TODO moar
}
