
use std::{io::BufReader, fs::File};

// use dicom_object::OpenFileOptions;
// use dicom_pixeldata::PixelDecoder;

// #[test]
// fn test_read_data_with_preamble() {
//     let path = dicom_test_files::path("pydicom/image_dfl.dcm").expect("test DICOM file should exist");
//     let source = BufReader::new(File::open(path).unwrap());

//     // should read preamble even though it's from a reader
//     let object = OpenFileOptions::new()
//         .from_reader(source)
//         .expect("Should read from source successfully");

//     let res = object.decode_pixel_data();
//     println!("{:?}", res);

// }