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
    use dicom_dictionary_std::tags;
    use std::env;

    #[test]
    fn test_dicom_dir() {
        let current_dir = env::current_dir().unwrap();
        println!("Current directory: {}", current_dir.display());
        let path = "src/dicomdirtests/DICOMDIR";
        let file_set_result = FileSet::new(&path);
        let Ok(file_set) = file_set_result else {
            println!("asdf not ok");
            return;
        };
        file_set.tags().for_each(|tag| println!("{:?}", tag));
        let directory_record_sequence = file_set
            .element(tags::DIRECTORY_RECORD_SEQUENCE)
            .expect("could not get referenced_file_id")
            .items()
            .expect("could not get items of directory_record_sequence");

        let referenced_file_ids: Vec<_> = directory_record_sequence
            .iter()
            .filter_map(|item| {
                item.element(tags::REFERENCED_FILE_ID)
                    .ok()
                    .and_then(|element| element.to_str().ok())
            })
            .collect();

        println!("Referenced File IDs:");
        for file_id in &referenced_file_ids {
            println!("  {}", file_id);
        }
    }
}
