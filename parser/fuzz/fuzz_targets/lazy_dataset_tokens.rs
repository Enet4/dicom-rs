#![no_main]
use dicom_parser::dataset::lazy_read::LazyDataSetReader;
use dicom_transfer_syntax_registry::entries;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let ts = entries::EXPLICIT_VR_LITTLE_ENDIAN.erased();
    if let Ok(mut reader) = LazyDataSetReader::new_with_ts(Cursor::new(data), &ts) {
        while let Some(Ok(token)) = reader.advance() {
            let _ = token.into_owned();
        }
    }
    let ts = entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();
    if let Ok(mut reader) = LazyDataSetReader::new_with_ts(Cursor::new(data), &ts) {
        while let Some(Ok(token)) = reader.advance() {
            let _ = token.into_owned();
        }
    }
});
