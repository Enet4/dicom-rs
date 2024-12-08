//! Independent test for the precondition of `submit_replace`:
//! the transfer syntax used must be stubbed.
//!
//! Only applicable to the inventory-based registry.
#![cfg(feature = "inventory-registry")]

/// Assert that this transfer syntax is provided built-in as a stub.
///
/// This only applies if Cargo feature "deflate" is disabled.
#[cfg(not(feature = "deflate"))]
#[test]
fn registry_has_stub_ts_by_default() {
    use dicom_encoding::{Codec, TransferSyntaxIndex};
    use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
    
    // this TS is provided by default, but not fully supported
    let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.4.95");
    assert!(ts.is_some());
    let ts = ts.unwrap();
    assert_eq!(ts.uid(), "1.2.840.10008.1.2.4.95");
    assert_eq!(ts.name(), "JPIP Referenced Deflate");
    assert!(matches!(
        ts.codec(),
        Codec::Dataset(None) | Codec::EncapsulatedPixelData(None, None)
    ));

    assert_eq!(ts.can_decode_dataset(), false);
    assert_eq!(ts.can_decode_all(), false);
}
