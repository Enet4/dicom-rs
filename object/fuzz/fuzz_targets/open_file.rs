#![no_main]
use libfuzzer_sys::{fuzz_target, Corpus};
use std::error::Error;

fuzz_target!(|data: &[u8]| -> Corpus {
    match fuzz(data) {
        Ok(_) => Corpus::Keep,
        Err(_) => Corpus::Reject,
    }
});

fn fuzz(data: &[u8]) -> Result<(), Box<dyn Error>> {
    // deserialize random bytes
    let mut obj = dicom_object::OpenFileOptions::new()
        .read_preamble(dicom_object::file::ReadPreamble::Auto)
        .odd_length_strategy(dicom_object::file::OddLengthStrategy::Fail)
        .from_reader(data)?;

    // remove group length elements
    for g in 0..=0x07FF {
        obj.remove_element(dicom_object::Tag(g, 0x0000));
    }
    // serialize object back to bytes
    let mut bytes = Vec::new();
    obj.write_all(&mut bytes)
        .expect("writing DICOM file should always be successful");

    // deserialize back to object
    let obj2 = dicom_object::from_reader(bytes.as_slice())
        .expect("serialized object should always deserialize");

    // assert equivalence
    assert_eq!(obj, obj2);

    Ok(())
}
