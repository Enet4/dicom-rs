//! Independent test for submission of a dummy TS implementation
//! with a dummy data set adapter.
//!
//! Only applicable to the inventory-based registry.
#![cfg(feature = "inventory-registry")]

use dicom_encoding::{
    submit_transfer_syntax, Codec, DataRWAdapter, Endianness, NeverPixelAdapter, TransferSyntax,
    TransferSyntaxIndex,
};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use std::io::{Read, Write};

/// this would, in theory, provide a dataset adapter
#[derive(Debug)]
struct DummyCodecAdapter;

impl<R: 'static, W: 'static> DataRWAdapter<R, W> for DummyCodecAdapter {
    type Reader = Box<dyn Read>;
    type Writer = Box<dyn Write>;

    fn adapt_reader(&self, reader: R) -> Self::Reader
    where
        R: Read,
    {
        Box::new(reader) as Box<_>
    }

    fn adapt_writer(&self, writer: W) -> Self::Writer
    where
        W: Write,
    {
        Box::new(writer) as Box<_>
    }
}

// install this dummy as a private transfer syntax
submit_transfer_syntax! {
    TransferSyntax::new(
        "1.2.840.10008.9999.9999.1",
        "Dummy Explicit VR Little Endian",
        Endianness::Little,
        true,
        Codec::Dataset::<_, NeverPixelAdapter>(DummyCodecAdapter)
    )
}

#[test]
fn contains_dummy_ts() {
    // contains our dummy TS, and claims to be fully supported
    let ts = TransferSyntaxRegistry.get("1.2.840.10008.9999.9999.1");
    assert!(ts.is_some());
    let ts = ts.unwrap();
    assert_eq!(ts.uid(), "1.2.840.10008.9999.9999.1");
    assert_eq!(ts.name(), "Dummy Explicit VR Little Endian");
    assert!(ts.fully_supported());
}
