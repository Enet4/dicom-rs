//! This module contains the high-level DICOM abstraction trait.
//! These traits should be preferred when dealing with a variety of DICOM objects.
use error::Result;
use attribute::value::DicomValue;
use data_element::DataElementHeader;

/// Trait type for a high-level abstraction of DICOM object.
/// At this level, objects are comparable to a lazy dictionary of elements,
/// in which some of them can be DICOM objects themselves.
pub trait DicomObject {

    /// Retrieve a particular DICOM element.
    fn get<T: Into<Option<(u16, u16)>>>(&self, tag: T) -> Result<(DataElementHeader, DicomValue)>;

}


