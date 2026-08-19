//! Test module for decoding overlay planes from real DICOM files.

use dicom_object::open_file;
use dicom_pixeldata::{OverlayType, PixelDecoder};

#[test]
fn decode_overlays_from_mr_with_overlays() {
    let test_file = dicom_test_files::path("pydicom/MR-SIEMENS-DICOM-WithOverlays.dcm").unwrap();
    let obj = open_file(test_file).unwrap();

    let overlays = obj.decode_overlays().unwrap();
    assert_eq!(overlays.len(), 1);

    let plane = &overlays[0];
    assert_eq!(plane.group(), 0x6000);
    assert_eq!(plane.rows(), 484);
    assert_eq!(plane.columns(), 484);
    assert_eq!(plane.overlay_type(), OverlayType::Graphics);
    assert_eq!(plane.number_of_frames(), 1);
    assert_eq!(plane.data().len(), 484 * 484);
    // the overlay is not empty
    assert!(plane.data().iter().any(|&bit| bit != 0));

    // same plane through the single plane decoding method
    let plane_2 = obj.decode_overlay(0).unwrap().unwrap();
    assert_eq!(&plane_2, plane);
    assert!(obj.decode_overlay(1).unwrap().is_none());
}
