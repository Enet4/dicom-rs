use dicom_core::value::Value;
use dicom_object::open_file;
use dicom_test_files;

#[test]
fn test_ob_value_with_unknown_length() {
    let path =
        dicom_test_files::path("pydicom/JPEG2000.dcm").expect("test DICOM file should exist");
    let object = open_file(&path).unwrap();
    let element = object.element_by_name("PixelData").unwrap();

    match element.value() {
        Value::PixelSequence { fragments, .. } => {
            // check the start and end of the bytes the check it looks right
            assert_eq!(fragments.len(), 1);
            let fragment = &fragments[0];
            assert_eq!(fragment[0..2], [255, 79]);
            assert_eq!(fragment[fragment.len() - 2..fragment.len()], [255, 217]);
        },
        _ => {
            panic!("expected a byte value");
        }
    }
}
