//! Independent test for submission of a dummy TS implementation
//! withour adapters.
//!
//! Only applicable to the inventory-based registry.
#![cfg(feature = "inventory-registry")]

use dicom_encoding::{
    submit_ele_transfer_syntax, Codec, TransferSyntaxIndex,
};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

// install this dummy as a private transfer syntax
submit_ele_transfer_syntax!(
    "1.2.840.10008.1.999.9999.99999",
    "Dummy Little Endian",
    Codec::EncapsulatedPixelData
);

#[test]
fn contains_stub_ts() {
    // contains our stub TS, not fully fully supported,
    // but enough to read some datasets
    let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.999.9999.99999");
    assert!(ts.is_some());
    let ts = ts.unwrap();
    assert_eq!(ts.uid(), "1.2.840.10008.1.999.9999.99999");
    assert_eq!(ts.name(), "Dummy Little Endian");
    assert!(!ts.fully_supported());
    assert!(ts.unsupported_pixel_encapsulation());
    // can obtain a data set decoder
    assert!(ts.decoder().is_some());
}
