//! Test suite for JPEG pixel data reading and writing
#![cfg(feature = "jpeg")]

mod adapters;

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use adapters::TestDataObject;
use dicom_core::value::PixelFragmentSequence;
use dicom_encoding::{
    adapters::{EncodeOptions, PixelDataReader, PixelDataWriter},
    Codec,
};
use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;

fn read_data_piece(test_file: impl AsRef<Path>, offset: u64, length: usize) -> Vec<u8> {
    let mut file = File::open(test_file).unwrap();
    // single fragment found in file data offset 0x6b6, 3314 bytes
    let mut buf = vec![0; length];
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.read_exact(&mut buf).unwrap();
    buf
}

fn check_rgb_pixel(pixels: &[u8], columns: u16, x: u16, y: u16, expected_pixel: [u8; 3]) {
    let i = (y as usize * columns as usize + x as usize) * 3;
    let got = [pixels[i], pixels[i + 1], pixels[i + 2]];
    assert_eq!(
        got, expected_pixel,
        "pixel mismatch at ({}, {}): {:?} vs {:?}",
        x, y, got, expected_pixel
    );
}

fn check_rgb_pixel_approx(pixels: &[u8], columns: u16, x: u16, y: u16, pixel: [u8; 3], margin: u8) {
    let i = (y as usize * columns as usize + x as usize) * 3;

    // check each component separately
    assert!(
        pixels[i].abs_diff(pixel[0]) <= margin,
        "R channel error: {} vs {}",
        pixels[i],
        pixel[0]
    );
    assert!(
        pixels[i + 1].abs_diff(pixel[1]) <= margin,
        "G channel error: {} vs {}",
        pixels[i + 1],
        pixel[1]
    );
    assert!(
        pixels[i + 2].abs_diff(pixel[2]) <= margin,
        "B channel error: {} vs {}",
        pixels[i + 2],
        pixel[2]
    );
}

#[test]
fn read_jpeg_baseline_1() {
    let test_file = dicom_test_files::path("pydicom/SC_rgb_jpeg_lossy_gdcm.dcm").unwrap();

    // manually fetch the pixel data fragment from the file

    // single fragment found in file data offset 0x6b8, 3314 bytes
    let buf = read_data_piece(test_file, 0x6b8, 3314);

    // create test object
    let obj = TestDataObject {
        // JPEG baseline (Process 1)
        ts_uid: "1.2.840.10008.1.2.4.50".to_string(),
        rows: 100,
        columns: 100,
        bits_allocated: 8,
        bits_stored: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![buf])),
    };

    // instantiate JpegAdapter and call decode_frame

    let Codec::EncapsulatedPixelData(Some(adapter), _) = JPEG_BASELINE.codec() else {
        panic!("JPEG pixel data reader not found")
    };

    let mut dest = vec![];

    adapter
        .decode_frame(&obj, 0, &mut dest)
        .expect("JPEG frame decoding failed");

    // inspect the result

    assert_eq!(dest.len(), 30_000);

    let err_margin = 7;

    // check a few known pixels

    // 0, 0
    check_rgb_pixel_approx(&dest, 100, 0, 0, [254, 0, 0], err_margin);
    // 50, 50
    check_rgb_pixel_approx(&dest, 100, 50, 50, [124, 124, 255], err_margin);
    // 75, 75
    check_rgb_pixel_approx(&dest, 100, 75, 75, [64, 64, 64], err_margin);
    // 16, 49
    check_rgb_pixel_approx(&dest, 100, 16, 49, [4, 4, 226], err_margin);
}

#[test]
fn read_jpeg_lossless_1() {
    let test_file = dicom_test_files::path("pydicom/SC_rgb_jpeg_gdcm.dcm").unwrap();

    // manually fetch the pixel data fragment from the file

    // single fragment found in file data offset 0x538, 3860 bytes
    let buf = read_data_piece(test_file, 0x538, 3860);

    // create test object
    let obj = TestDataObject {
        // JPEG baseline (Process 1)
        ts_uid: "1.2.840.10008.1.2.4.70".to_string(),
        rows: 100,
        columns: 100,
        bits_allocated: 8,
        bits_stored: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![buf])),
    };

    // instantiate JpegAdapter and call decode_frame

    let Codec::EncapsulatedPixelData(Some(adapter), _) = JPEG_BASELINE.codec() else {
        panic!("JPEG pixel data reader not found")
    };

    let mut dest = vec![];

    adapter
        .decode_frame(&obj, 0, &mut dest)
        .expect("JPEG frame decoding failed");

    // inspect the result

    assert_eq!(dest.len(), 30_000);

    // check a few known pixels

    // 0, 0
    check_rgb_pixel(&dest, 100, 0, 0, [255, 0, 0]);
    // 50, 50
    check_rgb_pixel(&dest, 100, 50, 50, [128, 128, 255]);
    // 75, 75
    check_rgb_pixel(&dest, 100, 75, 75, [64, 64, 64]);
    // 16, 49
    check_rgb_pixel(&dest, 100, 16, 49, [0, 0, 255]);
}

/// writing to JPEG and back should yield approximately the same pixel data
#[test]
fn write_and_read_jpeg_baseline() {
    let rows: u16 = 256;
    let columns: u16 = 512;

    // build some random RGB image
    let mut samples = vec![0; rows as usize * columns as usize * 3];

    // use linear congruence to make RGB noise
    let mut seed = 0xcfcf_acab_u32;
    let mut gen_sample = || {
        let r = 4_294_967_291_u32;
        let b = 67291_u32;
        seed = seed.wrapping_mul(r).wrapping_add(b);
        // grab a portion from the seed
        (seed >> 7) as u8
    };

    let slab = 8;
    for y in (0..rows as usize).step_by(slab) {
        let scan_r = gen_sample();
        let scan_g = gen_sample();
        let scan_b = gen_sample();

        for x in 0..columns as usize {
            for k in 0..slab {
                let offset = ((y + k) * columns as usize + x) * 3;
                samples[offset] = scan_r;
                samples[offset + 1] = scan_g;
                samples[offset + 2] = scan_b;
            }
        }
    }

    // create test object of native encoding
    let obj = TestDataObject {
        // Explicit VR Little Endian
        ts_uid: "1.2.840.10008.1.2.1".to_string(),
        rows,
        columns,
        bits_allocated: 8,
        bits_stored: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 1,
        flat_pixel_data: Some(samples.clone()),
        pixel_data_sequence: None,
    };

    // instantiate JpegAdapter and call encode_frame

    let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) = JPEG_BASELINE.codec() else {
        panic!("JPEG pixel data adapters not found")
    };

    // request higher quality to reduce loss
    let mut options = EncodeOptions::default();
    options.quality = Some(95);

    let mut encoded = vec![];

    let _ops = writer
        .encode_frame(&obj, 0, options, &mut encoded)
        .expect("JPEG frame encoding failed");

    // instantiate new object representing the compressed version

    let obj = TestDataObject {
        // JPEG baseline (Process 1)
        ts_uid: "1.2.840.10008.1.2.4.50".to_string(),
        rows,
        columns,
        bits_allocated: 8,
        bits_stored: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![encoded])),
    };

    // decode frame
    let mut decoded = vec![];

    reader
        .decode_frame(&obj, 0, &mut decoded)
        .expect("JPEG frame decoding failed");

    // inspect the result
    assert_eq!(samples.len(), decoded.len(), "pixel data length mismatch");

    // traverse all pixels, compare with error margin
    let err_margin = 7;

    for (src_sample, decoded_sample) in samples.iter().copied().zip(decoded.iter().copied()) {
        assert!(
            src_sample.abs_diff(decoded_sample) <= err_margin,
            "pixel sample mismatch: {} vs {}",
            src_sample,
            decoded_sample
        );
    }
}
