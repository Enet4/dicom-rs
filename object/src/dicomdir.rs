use std::path::Path;

use crate::{DefaultDicomObject, OpenFileOptions, ReadError};

pub struct FileSet;

pub type Result<T, E = ReadError> = std::result::Result<T, E>;

impl FileSet {
    pub fn new<P>(path: P) -> Result<DefaultDicomObject>
    where
        P: AsRef<Path>,
    {
        OpenFileOptions::new().open_file(path)
    }
}

#[cfg(test)]
mod test {
    use super::FileSet;
    use std::env;

    #[test]
    fn test_dicom_dir() {
        let current_dir = env::current_dir().unwrap();
        println!("Current directory: {}", current_dir.display());
        let file_set = FileSet::new("src/dicomdirtests/DICOMDIR");
        println!("asdf file_set {:?}", file_set);
    }
}
