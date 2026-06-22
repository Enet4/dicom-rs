#![no_main]
use dicom_core::value::{DicomDate, DicomDateTime, DicomTime, PersonName};
use dicom_core::PrimitiveValue;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = s.parse::<DicomDate>();
        let _ = s.parse::<DicomTime>();
        let _ = s.parse::<DicomDateTime>();
        let _ = PersonName::from_text(s).to_dicom_string();
        let pv = PrimitiveValue::from(s);
        let _ = pv.to_int::<i32>();
        let _ = pv.to_float32();
        let _ = pv.to_float64();
        let _ = pv.to_date();
        let _ = pv.to_time();
        let _ = pv.to_datetime();
    }
});
