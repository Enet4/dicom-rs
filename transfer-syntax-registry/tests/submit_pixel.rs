//! Independent test for submission of a dummy TS implementation
//! with a pixel data adapter.
//!
//! Only applicable to the inventory-based registry.
#![cfg(feature = "inventory-registry")]

use dicom_encoding::{
    adapters::{
        DecodeResult, EncodeOptions, EncodeResult, PixelDataObject, PixelDataReader,
        PixelDataWriter,
    },
    submit_transfer_syntax, Codec, NeverAdapter, TransferSyntax, TransferSyntaxIndex,
};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

/// this would, in theory, provide a pixel data adapter
#[derive(Debug)]
struct DummyPixelAdapter;

impl PixelDataReader for DummyPixelAdapter {
    fn decode(&self, _src: &dyn PixelDataObject, _dst: &mut Vec<u8>) -> DecodeResult<()> {
        panic!("Stub, not supposed to be called")
    }

    fn decode_frame(
        &self,
        _src: &dyn PixelDataObject,
        _frame: u32,
        _dst: &mut Vec<u8>,
    ) -> DecodeResult<()> {
        panic!("Stub, not supposed to be called")
    }
}

impl PixelDataWriter for DummyPixelAdapter {
    fn encode(
        &self,
        _src: &dyn PixelDataObject,
        _options: EncodeOptions,
        _dst: &mut Vec<Vec<u8>>,
        _offset_table: &mut Vec<u32>,
    ) -> EncodeResult<Vec<dicom_core::ops::AttributeOp>> {
        panic!("Stub, not supposed to be called")
    }

    fn encode_frame(
        &self,
        _src: &dyn PixelDataObject,
        _frame: u32,
        _options: EncodeOptions,
        _dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<dicom_core::ops::AttributeOp>> {
        panic!("Stub, not supposed to be called")
    }
}

// install this dummy as a private transfer syntax
submit_transfer_syntax! {
    TransferSyntax::<NeverAdapter, _, _>::new_ele(
        "1.2.840.10008.9999.9999.2",
        "Dummy Lossless",
        Codec::EncapsulatedPixelData(Some(DummyPixelAdapter), Some(DummyPixelAdapter))
    )
}

#[test]
fn contains_dummy_ts() {
    // contains our dummy TS, and claims to be fully supported
    let ts = TransferSyntaxRegistry.get("1.2.840.10008.9999.9999.2");
    assert!(ts.is_some());
    let ts = ts.unwrap();
    assert_eq!(ts.uid(), "1.2.840.10008.9999.9999.2");
    assert_eq!(ts.name(), "Dummy Lossless");
    assert!(!ts.is_codec_free());
    assert!(ts.is_fully_supported());
    assert!(ts.can_decode_dataset());
    assert!(ts.can_decode_all());
}
