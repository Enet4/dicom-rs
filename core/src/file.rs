use std::io::{Read, Seek, Write};
use std::fs::File;
use std::path::Path;
use error::Result;
use object::DicomObject;
use DefaultDicomObject;

pub fn from_file<'s, F: 's + Read + Seek>(file: F) -> Result<DefaultDicomObject> {
    unimplemented!()
}


pub fn from_path<'s, P: AsRef<Path>>(path: P) -> Result<DefaultDicomObject> {
    let file = File::open(path)?;
    from_file(file)
}


pub fn to_file<F: Write, D: DicomObject>(obj: &D, to: F) -> Result<()> {
    unimplemented!()
}