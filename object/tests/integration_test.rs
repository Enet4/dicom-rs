use dicom_core::value::{PrimitiveValue, Value};
use dicom_object::open_file;
use dicom_test_files;

#[test]
fn test_ob_value_with_unknown_length() {
    let path =
        dicom_test_files::path("pydicom/JPEG2000.dcm").expect("test DICOM file should exist");
    let object = open_file(&path).unwrap();
    let element = object.element_by_name("PixelData").unwrap();

    if let Value::Primitive(PrimitiveValue::U8(bytes)) = element.value() {
        // check the start and end of the bytes the check it looks right
        assert_eq!(bytes[0..2], [0xfe, 0xff]);
        assert_eq!(bytes[bytes.len() - 2..bytes.len()], [0xff, 0xd9]);
    } else {
        panic!("expected a byte value");
    }
}
