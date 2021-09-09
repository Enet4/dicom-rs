use std::{fs::File, io::BufReader};

use dicom_core::value::Value;
use dicom_encoding::text::SpecificCharacterSet;
use dicom_object::{mem::InMemDicomObject, open_file};
use dicom_test_files;

#[test]
fn test_ob_value_with_unknown_length() {
    let path =
        dicom_test_files::path("pydicom/JPEG2000.dcm").expect("test DICOM file should exist");
    let object = open_file(&path).unwrap();
    let element = object.element_by_name("PixelData").unwrap();

    match element.value() {
        Value::PixelSequence { fragments, offset_table } => {

            // check offset table
            assert_eq!(offset_table.len(), 0);

            // check if the leading and trailing bytes look right
            assert_eq!(fragments.len(), 1);
            let fragment = &fragments[0];
            assert_eq!(fragment[0..2], [255, 79]);
            assert_eq!(fragment[fragment.len() - 2..fragment.len()], [255, 217]);
        }
        value => {
            panic!("expected a pixel sequence, but got {:?}", value);
        }
    }
}

#[test]
fn test_expl_vr_le_no_meta() {
    let path = dicom_test_files::path("pydicom/ExplVR_LitEndNoMeta.dcm")
        .expect("test DICOM file should exist");
    let source = BufReader::new(File::open(path).unwrap());
    let ts = dicom_transfer_syntax_registry::entries::EXPLICIT_VR_LITTLE_ENDIAN.erased();
    let object =
        InMemDicomObject::read_dataset_with_ts_cs(source, &ts, SpecificCharacterSet::Default)
            .unwrap();

    let sop_instance_uid = object.element_by_name("SOPInstanceUID").unwrap();
    assert_eq!(sop_instance_uid.to_str().unwrap(), "1.2.333.4444.5.6.7.8",);

    let series_instance_uid = object.element_by_name("SeriesInstanceUID").unwrap();
    assert_eq!(
        series_instance_uid.to_str().unwrap(),
        "1.2.333.4444.5.6.7.8.99\0",
    );

    let frame_of_reference_uid = object.element_by_name("FrameOfReferenceUID").unwrap();
    assert_eq!(
        frame_of_reference_uid.to_str().unwrap(),
        "1.2.333.4444.5.6.7.8.9",
    );
}
