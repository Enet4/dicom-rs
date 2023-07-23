//! Independent test for submission of a dummy TS implementation
//! to replace a built-in stub.
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

// pretend to implement JPIP Referenced Deflate,
// which is in the registry by default,
// but not fully supported
submit_transfer_syntax! {
    TransferSyntax::new(
        "1.2.840.10008.1.2.4.95",
        "JPIP Referenced Deflate (Override)",
        Endianness::Little,
        true,
        Codec::Dataset::<_, NeverPixelAdapter, NeverPixelAdapter>(Some(DummyCodecAdapter))
    )
}

#[test]
fn contains_dummy_ts() {
    // contains our dummy TS, and claims to be fully supported
    let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.4.95");
    assert!(ts.is_some());
    let ts = ts.unwrap();
    assert_eq!(ts.uid(), "1.2.840.10008.1.2.4.95");
    assert_eq!(ts.name(), "JPIP Referenced Deflate (Override)");
    assert!(ts.is_fully_supported());
    assert!(ts.can_decode_dataset());
    assert!(ts.can_decode_all());
}
