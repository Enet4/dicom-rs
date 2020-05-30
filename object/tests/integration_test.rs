use dicom_core::value::{PrimitiveValue, Value};
use dicom_object::open_file;
use dicom_test_files;

#[test]
fn test_ob_value_with_unknown_length() {
    let path =
        dicom_test_files::path("pydicom/JPEG2000.dcm").expect("test DICOM file should exist");
    let object = open_file(&path).unwrap();
    let element = object.element_by_name("PixelData").unwrap();

    println!("{:?}", element.value());
    match element.value() {
        Value::PixelSequence { fragments, .. } => {
            // check the start and end of the bytes the check it looks right
            assert_eq!(fragments[0][0..2], [0xfe, 0xff]);
            assert_eq!(fragments[0][fragments.len() - 2..fragments.len()], [0xff, 0xd9]);
        },
        _ => {
            panic!("expected a byte value");
        }
    }
}
