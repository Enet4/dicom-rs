use std::io::Read;
use std::path::Path;
use dicom_parser::error::Result;
use crate::DefaultDicomObject;

pub fn from_reader<F>(file: F) -> Result<DefaultDicomObject>
where
    F: Read,
{
    DefaultDicomObject::from_reader(file)
}

pub fn open_file<P>(path: P) -> Result<DefaultDicomObject> 
where
    P: AsRef<Path>
{
    DefaultDicomObject::open_file(path)
}
