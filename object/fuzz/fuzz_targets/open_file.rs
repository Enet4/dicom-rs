#![no_main]
use libfuzzer_sys::fuzz_target;
use std::error::Error;

fuzz_target!(|data: &[u8]| {
    let _ = fuzz(data);
});

fn fuzz(data: &[u8]) -> Result<(), Box<dyn Error>> {
    // deserialize random bytes
    let obj = dicom_object::from_reader(data)?;

    // serialize object back to bytes
    let mut bytes = Vec::new();
    obj.write_all(&mut bytes)?;

    // deserialize back to object
    let obj2 = dicom_object::from_reader(bytes.as_slice())
        .expect("serialized object should always deserialize");

    // assert equivalence
    assert_eq!(obj, obj2);

    Ok(())
}
