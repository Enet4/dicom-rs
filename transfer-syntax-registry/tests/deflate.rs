use std::{fs::File, io::BufReader};

use dicom_core::Tag;
use dicom_object::OpenFileOptions;

#[test]
fn test_read_data_deflated() {
    let path =
        dicom_test_files::path("pydicom/image_dfl.dcm").expect("test DICOM file should exist");
    let source = BufReader::new(File::open(path).unwrap());

    // should read preamble even though it's from a reader
    let object = OpenFileOptions::new()
        .from_reader(source)
        .expect("Should read from source successfully");

    // inspect some attributes

    // SOP Instance UID
    assert_eq!(
        object.get(Tag(0x0008, 0x0018)).unwrap().to_str().unwrap(),
        "1.3.6.1.4.1.5962.1.1.0.0.0.977067309.6001.0",
    );

    // photometric interpretation
    assert_eq!(
        object.get(Tag(0x0028, 0x0004)).unwrap().to_str().unwrap(),
        "MONOCHROME2",
    );

    let rows: u16 = object.get(Tag(0x0028, 0x0010)).unwrap().to_int().unwrap();
    let cols: u16 = object.get(Tag(0x0028, 0x0011)).unwrap().to_int().unwrap();
    let spp: u16 = object.get(Tag(0x0028, 0x0002)).unwrap().to_int().unwrap();
    assert_eq!((rows, cols, spp), (512, 512, 1));

    // pixel data

    let pixel_data = object.get(Tag(0x7FE0, 0x0010)).unwrap().to_bytes().unwrap();

    assert_eq!(
        pixel_data.len(),
        rows as usize * cols as usize * spp as usize,
    );

    // poke some of the pixel samples
    assert_eq!(pixel_data[0], 0xd5);
    assert_eq!(pixel_data[0x0080], 0x29);
    assert_eq!(pixel_data[0x0804], 0xff);
}

/// Encoding to deflated and back should yield the same pixel data.
#[test]
fn write_deflated() {
    let path = dicom_test_files::path("WG04/REF/US1_UNC").unwrap();

    // should read preamble even though it's from a reader
    let mut object = OpenFileOptions::new()
        .open_file(path)
        .expect("Should read test file successfully");

    // set transfer syntax to deflated
    object.meta_mut().set_transfer_syntax(
        &dicom_transfer_syntax_registry::entries::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN.erased(),
    );

    let mut buf = Vec::<u8>::new();
    object.write_all(&mut buf).unwrap();

    // check that the contents are indeed smaller than the original (902k)
    // (this US image compresses pretty well in deflate)
    assert!(buf.len() < 400_000, "Expected deflated size to be smaller, but was {} bytes", buf.len());

    // now read the deflated data back
    let object2 = OpenFileOptions::new()
        .from_reader(buf.as_slice())
        .expect("Should read deflated data back successfully");
    // check that the objects are mostly equal

    assert_eq!(object.meta().transfer_syntax(), object2.meta().transfer_syntax());
    // SOP Instance UID
    let tag = Tag(0x0008, 0x0018);
    assert_eq!(object.get(tag).unwrap(), object2.get(tag).unwrap());

    // Study Instance UID
    let tag = Tag(0x0020, 0x000D);
    assert_eq!(object.get(tag).unwrap(), object2.get(tag).unwrap());

    // pixel data should be equal
    let tag = Tag(0x7FE0, 0x0010);
    let pixel_data = object.get(tag).unwrap().to_bytes().unwrap();
    let pixel_data2 = object2.get(tag).unwrap().to_bytes().unwrap();
    assert_eq!(pixel_data, pixel_data2);
}
