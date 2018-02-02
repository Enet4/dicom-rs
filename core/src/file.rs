use std::io::{Read, Seek, Write};
use std::path::Path;
use error::Result;
use object::DicomObject;
use DefaultDicomObject;

pub fn from_stream<'s, F: 's + Read + Seek>(file: F) -> Result<DefaultDicomObject> {
    unimplemented!()
}

pub fn from_file<'s, P: AsRef<Path>>(path: P) -> Result<DefaultDicomObject> {
    DefaultDicomObject::from_file(path)
}

pub fn to_file<F: Write, D: DicomObject>(obj: &D, to: F) -> Result<()> {
    unimplemented!()
}
