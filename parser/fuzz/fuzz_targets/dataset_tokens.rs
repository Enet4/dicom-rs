#![no_main]
use dicom_parser::DataSetReader;
use dicom_transfer_syntax_registry::entries;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let ts = entries::EXPLICIT_VR_LITTLE_ENDIAN.erased();
    if let Ok(reader) = DataSetReader::new_with_ts(data, &ts) {
        for token in reader {
            let _ = token;
        }
    }
    let ts = entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();
    if let Ok(reader) = DataSetReader::new_with_ts(data, &ts) {
        for token in reader {
            let _ = token;
        }
    }
});
