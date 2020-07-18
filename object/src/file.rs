use crate::{DefaultDicomObject, Result};
use std::io::Read;
use std::path::Path;

/// Create a DICOM object by reading from a byte source.
///
/// This function assumes the standard file encoding structure without the
/// preamble: file meta group, followed by the rest of the data set.
pub fn from_reader<F>(file: F) -> Result<DefaultDicomObject>
where
    F: Read,
{
    DefaultDicomObject::from_reader(file)
}

/// Create a DICOM object by reading from a file.
///
/// This function assumes the standard file encoding structure: 128-byte
/// preamble, file meta group, and the rest of the data set.
pub fn open_file<P>(path: P) -> Result<DefaultDicomObject>
where
    P: AsRef<Path>,
{
    DefaultDicomObject::open_file(path)
}
