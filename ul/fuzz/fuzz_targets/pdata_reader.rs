#![no_main]
use bytes::BytesMut;
use dicom_ul::association::pdata::PDataReader;
use dicom_ul::pdu::{MAXIMUM_PDU_SIZE, MINIMUM_PDU_SIZE};
use libfuzzer_sys::fuzz_target;
use std::io::Read;

struct Cycling<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Read for Cycling<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.data.is_empty() || buf.is_empty() {
            return Ok(0);
        }
        for slot in buf.iter_mut() {
            *slot = self.data[self.pos];
            self.pos = (self.pos + 1) % self.data.len();
        }
        Ok(buf.len())
    }
}

fuzz_target!(|input: (u32, Vec<u8>)| {
    let (max_pdu_length, data) = input;
    if data.is_empty() {
        return;
    }
    let max_pdu_length = max_pdu_length.clamp(MINIMUM_PDU_SIZE, MAXIMUM_PDU_SIZE);

    let mut stream = Cycling { data: &data, pos: 0 };
    let mut read_buffer = BytesMut::new();
    let mut reader = PDataReader::new(&mut stream, max_pdu_length, &mut read_buffer);

    let mut sink = [0u8; 4096];
    let _ = reader.read(&mut sink);
});
