//! Independent test for testing that submitting a TS in a separate crate work.
use dicom_encoding::submit_transfer_syntax;
use dicom_encoding::transfer_syntax::{Codec, DataRWAdapter, Endianness, TransferSyntax, TransferSyntaxIndex};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use std::io::{Read, Write};

/// this would, in theory, provide a dataset adapter
#[derive(Debug)]
struct DummyCodecAdapter;

impl<R, W> DataRWAdapter<R, W> for DummyCodecAdapter {
    type Reader = Box<dyn Read>;
    type Writer = Box<dyn Write>;

    fn adapt_reader(&self, _reader: R) -> Self::Reader
    where
        R: Read,
    {
        unimplemented!()
    }

    fn adapt_writer(&self, _writer: W) -> Self::Writer
    where
        W: Write,
    {
        unimplemented!()
    }
}

// install this dummy as a private transfer syntax
submit_transfer_syntax! {
    TransferSyntax::new(
        "1.2.840.10008.9999.9999",
        "Dummy Explicit VR Little Endian",
        Endianness::Little,
        true,
        Codec::Dataset(DummyCodecAdapter),
    )
}

#[test]
fn contains_dummy_ts() {
    // contains our dummy TS, and claims to be fully supported
    let ts = TransferSyntaxRegistry.get("1.2.840.10008.9999.9999");
    assert!(ts.is_some());
    let ts = ts.unwrap();
    assert_eq!(ts.uid(), "1.2.840.10008.9999.9999");
    assert_eq!(ts.name(), "Dummy Explicit VR Little Endian");
    assert!(ts.fully_supported());
}
