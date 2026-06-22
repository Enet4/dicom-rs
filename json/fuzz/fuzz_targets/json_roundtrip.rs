#![no_main]
use dicom_object::InMemDicomObject;
use libfuzzer_sys::{Corpus, fuzz_target};

fuzz_target!(|data: &[u8]| -> Corpus {
    let Ok(s) = std::str::from_utf8(data) else {
        return Corpus::Reject;
    };
    let Ok(obj) = dicom_json::from_str::<InMemDicomObject>(s) else {
        return Corpus::Reject;
    };
    let Ok(json) = dicom_json::to_string(&obj) else {
        return Corpus::Keep;
    };
    let _ = dicom_json::from_str::<InMemDicomObject>(&json);
    Corpus::Keep
});
