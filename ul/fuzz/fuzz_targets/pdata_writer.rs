#![no_main]
use dicom_ul::association::pdata::PDataWriter;
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|input: (u8, u32, Vec<u8>)| {
    let (presentation_context_id, max_pdu_length, data) = input;
    if data.is_empty() {
        return;
    }

    let mut out = Vec::new();
    let mut writer = PDataWriter::new(&mut out, presentation_context_id, max_pdu_length);

    let _ = writer.write_all(&data);
    let _ = writer.finish();
});
