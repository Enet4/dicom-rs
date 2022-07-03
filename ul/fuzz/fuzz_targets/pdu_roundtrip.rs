#![no_main]
use std::error::Error;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (u32, bool, &[u8])| {
    let (maxlen, strict, data) = data;
    let _ = fuzz(maxlen, strict, data);
});

fn fuzz(maxlen: u32, strict: bool, mut data: &[u8]) -> Result<(), Box<dyn Error>> {
    // deserialize random bytes
    let pdu = dicom_ul::pdu::read_pdu(&mut data, maxlen, strict)?;

    // serialize pdu back to bytes
    let mut bytes = Vec::new();
    dicom_ul::pdu::write_pdu(&mut bytes, &pdu)?;

    // deserialize back to pdu
    let pdu2 = dicom_ul::pdu::read_pdu(&mut bytes.as_slice(), maxlen, strict)
        .expect("serialized pdu should always deserialize");

    // assert equivalence
    assert_eq!(
        pdu, pdu2,
        "pdu should be equal after serializing to/from bytes"
    );

    Ok(())
}
