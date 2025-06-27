//! Test suite for RLE lossless pixel data reading and writing
#![cfg(feature = "rle")]

mod adapters;

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use adapters::TestDataObject;
use dicom_core::value::PixelFragmentSequence;
use dicom_encoding::{adapters::PixelDataReader, Codec};
use dicom_transfer_syntax_registry::entries::RLE_LOSSLESS;

fn read_data_piece(test_file: impl AsRef<Path>, offset: u64, length: usize) -> Vec<u8> {
    let mut file = File::open(test_file).unwrap();
    let mut buf = vec![0; length];
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.read_exact(&mut buf).unwrap();
    buf
}

fn check_u16_rgb_pixel(pixels: &[u8], columns: u16, x: u16, y: u16, expected_pixel: [u16; 3]) {
    let i = (y as usize * columns as usize * 3 + x as usize * 3) * 2;
    let got = [
        u16::from_le_bytes([pixels[i], pixels[i + 1]]),
        u16::from_le_bytes([pixels[i + 2], pixels[i + 3]]),
        u16::from_le_bytes([pixels[i + 4], pixels[i + 5]]),
    ];
    assert_eq!(
        got, expected_pixel,
        "pixel sample mismatch at ({x}, {y}): {got:?} vs {expected_pixel:?}"
    );
}

fn check_i16_monochrome_pixel(pixels: &[u8], columns: u16, x: u16, y: u16, expected: i16) {
    let i = (y as usize * columns as usize + x as usize) * 2;
    let got = i16::from_le_bytes([pixels[i], pixels[i + 1]]);
    assert_eq!(
        got, expected,
        "pixel sample mismatch at ({x}, {y}): {got:?} vs {expected:?}"
    );
}

#[test]
fn read_rle_1() {
    let test_file = dicom_test_files::path("WG04/RLE/CT1_RLE").unwrap();

    // manually fetch the pixel data fragment from the file:

    // PixelData offset: 0x18f6
    // first fragment item offset: 0x1902
    // first fragment size: 4
    // second fragment item offset: 0x190e
    // second fragment size: 248330

    // single fragment found in file data offset 0x1916, 248330 bytes
    let buf = read_data_piece(test_file, 0x1916, 248330);

    // create test object
    let obj = TestDataObject {
        // RLE lossless
        ts_uid: "1.2.840.10008.1.2.5".to_string(),
        rows: 512,
        columns: 512,
        bits_allocated: 16,
        bits_stored: 16,
        samples_per_pixel: 1,
        photometric_interpretation: "MONOCHROME2",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![buf])),
    };

    // instantiate RLE lossless adapter

    let Codec::EncapsulatedPixelData(Some(adapter), _) = RLE_LOSSLESS.codec() else {
        panic!("RLE lossless pixel data reader not found")
    };

    let mut dest = vec![];

    // decode the whole image (1 frame)

    adapter
        .decode(&obj, &mut dest)
        .expect("RLE frame decoding failed");

    // inspect the result
    assert_eq!(dest.len(), 512 * 512 * 2);

    // check a few known pixels
    check_i16_monochrome_pixel(&dest, 512, 16, 16, -2_000);

    check_i16_monochrome_pixel(&dest, 512, 255, 255, 980);

    check_i16_monochrome_pixel(&dest, 512, 342, 336, 188);

    // decode a single frame
    let mut dest2 = vec![];
    adapter
        .decode_frame(&obj, 0, &mut dest2)
        .expect("RLE frame decoding failed");

    // the outcome should be the same
    assert_eq!(dest, dest2);
}

#[test]
fn read_rle_2() {
    let test_file = dicom_test_files::path("pydicom/SC_rgb_rle_16bit.dcm").unwrap();

    // manually fetch the pixel data fragment from the file:
    // PixelData offset: 0x51a
    // first fragment item offset: 0x526
    // first fragment size: 0
    // second fragment item offset: 0x52e
    // second fragment size: 1264

    // single fragment found in file data offset 0x536, 1264 bytes
    let buf = read_data_piece(test_file, 0x536, 1_264);

    // create test object
    let obj = TestDataObject {
        // RLE lossless
        ts_uid: "1.2.840.10008.1.2.5".to_string(),
        rows: 100,
        columns: 100,
        bits_allocated: 16,
        bits_stored: 16,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![buf])),
    };

    // instantiate RLE lossless adapter

    let Codec::EncapsulatedPixelData(Some(adapter), _) = RLE_LOSSLESS.codec() else {
        panic!("RLE lossless pixel data reader not found")
    };

    let mut dest = vec![];

    // decode the whole image (1 frame)

    adapter
        .decode(&obj, &mut dest)
        .expect("RLE frame decoding failed");

    // inspect the result
    assert_eq!(dest.len(), 100 * 100 * 2 * 3);

    // check a few known pixels
    check_u16_rgb_pixel(&dest, 100, 0, 0, [0xFFFF, 0, 0]);

    check_u16_rgb_pixel(&dest, 100, 99, 19, [0xFFFF, 32_896, 32_896]);

    check_u16_rgb_pixel(&dest, 100, 54, 65, [0, 0, 0]);

    check_u16_rgb_pixel(&dest, 100, 10, 95, [0xFFFF, 0xFFFF, 0xFFFF]);
}
