
use std::{io::BufReader, fs::{metadata, File}};


use dicom_object::OpenFileOptions;
use dicom_pixeldata::PixelDecoder;

#[test]
fn test_read_data_deflated() {
    let path = dicom_test_files::path("pydicom/image_dfl.dcm").expect("test DICOM file should exist");
    let source = BufReader::new(File::open(path).unwrap());

    // should read preamble even though it's from a reader
    let object = OpenFileOptions::new()
        .from_reader(source)
        .expect("Should read from source successfully");

    let res = object.decode_pixel_data().unwrap();
    assert_eq!((
        res.rows() as usize * 
        res.columns() as usize *
        res.number_of_frames() as usize), res.data().len() as usize);
}

#[test]
fn write_deflated(){
    let path = dicom_test_files::path("pydicom/image_dfl.dcm").expect("test DICOM file should exist");
    let source = BufReader::new(File::open(path.clone()).unwrap());

    // should read preamble even though it's from a reader
    let object = OpenFileOptions::new()
        .from_reader(source)
        .expect("Should read from source successfully");

    let mut buf = Vec::<u8>::new();
    object.write_all(&mut buf).unwrap();
    assert_eq!(buf.len(), metadata(path).unwrap().len() as usize);
}