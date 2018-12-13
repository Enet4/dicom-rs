use std::io::{Read, Write};
use std::path::Path;
use dicom_parser::error::Result;
use DicomObject;
use DefaultDicomObject;

pub fn from_stream<F>(file: F) -> Result<DefaultDicomObject>
where
    F: Read,
{
    DefaultDicomObject::from_stream(file)
}

pub fn open_file<P>(path: P) -> Result<DefaultDicomObject> 
where
    P: AsRef<Path>
{
    DefaultDicomObject::open_file(path)
}

pub fn to_file<F: Write, D: DicomObject>(obj: &D, to: F) -> Result<()> {
    unimplemented!()
}
