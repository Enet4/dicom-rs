//! Test suite for JPEG 2000 pixel data reading and writing
#![cfg(any(feature = "openjpeg-sys", feature = "openjp2"))]

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
use dicom_transfer_syntax_registry::entries::{
    JPEG_2000_IMAGE_COMPRESSION, JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY,
};

fn read_data_piece(test_file: impl AsRef<Path>, offset: u64, length: usize) -> Vec<u8> {
    let mut file = File::open(test_file).unwrap();
    // single fragment found in file data offset 0x6b6, 3314 bytes
    let mut buf = vec![0; length];
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.read_exact(&mut buf).unwrap();
    buf
}

fn check_w_monochrome_pixel(pixels: &[u8], columns: u16, x: u16, y: u16, expected_pixel: u16) {
    let i = (y as usize * columns as usize + x as usize) * 2;
    if i + 1 >= pixels.len() {
        panic!("pixel index {} at ({}, {}) is out of bounds", i, x, y);
    }
    let got = u16::from_le_bytes([pixels[i], pixels[i + 1]]);
    assert_eq!(
        got, expected_pixel,
        "pixel mismatch at ({}, {}): {:?} vs {:?}",
        x, y, got, expected_pixel
    );
}

fn check_w_monochrome_pixel_approx(
    data: &[u8],
    columns: u16,
    x: u16,
    y: u16,
    pixel: u16,
    margin: u16,
) {
    let i = (y as usize * columns as usize + x as usize) * 2;
    let sample = u16::from_le_bytes([data[i], data[i + 1]]);

    assert!(
        sample.abs_diff(pixel) <= margin,
        "sample error at ({}, {}): {} vs {}",
        x,
        y,
        sample,
        pixel
    );
}

#[test]
fn read_jpeg_2000_1() {
    let test_file = dicom_test_files::path("WG04/J2KI/NM1_J2KI").unwrap();

    // manually fetch the pixel data fragment from the file

    // single fragment found in file data offset 0x0bea, 250 bytes
    let buf = read_data_piece(test_file, 0x0bea, 250);

    let cols = 256;

    // create test object
    let obj = TestDataObject {
        ts_uid: "1.2.840.10008.1.2.4.91".to_string(),
        rows: 1024,
        columns: cols,
        bits_allocated: 16,
        bits_stored: 16,
        samples_per_pixel: 1,
        photometric_interpretation: "MONOCHROME2",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![buf])),
    };

    // fetch decoder

    let Codec::EncapsulatedPixelData(Some(adapter), _) = JPEG_2000_IMAGE_COMPRESSION.codec() else {
        panic!("JPEG 2000 pixel data reader not found")
    };

    let mut dest = vec![];

    adapter
        .decode_frame(&obj, 0, &mut dest)
        .expect("frame decoding failed");

    // inspect the result

    assert_eq!(dest.len(), 1024 * 256 * 2);

    let err_margin = 256;

    // check a few known pixels

    // 0, 0
    check_w_monochrome_pixel_approx(&dest, cols, 0, 0, 0, err_margin);
    // 64, 154
    check_w_monochrome_pixel_approx(&dest, cols, 64, 154, 0, err_margin);
    // 135, 145
    check_w_monochrome_pixel_approx(&dest, cols, 135, 145, 168, err_margin);
    // 80, 188
    check_w_monochrome_pixel_approx(&dest, cols, 80, 188, 9, err_margin);
    // 136, 416
    check_w_monochrome_pixel_approx(&dest, cols, 136, 416, 245, err_margin);
}

#[test]
fn read_jpeg_2000_lossless_1() {
    let test_file = dicom_test_files::path("WG04/J2KR/NM1_J2KR").unwrap();

    // manually fetch the pixel data fragment from the file

    // single fragment found in file data offset b82, 65536 bytes
    let buf = read_data_piece(test_file, 0x0b82, 65536);

    let cols = 256;

    // create test object
    let obj = TestDataObject {
        ts_uid: "1.2.840.10008.1.2.4.90".to_string(),
        rows: 1024,
        columns: cols,
        bits_allocated: 16,
        bits_stored: 16,
        samples_per_pixel: 1,
        photometric_interpretation: "MONOCHROME2",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![buf])),
    };

    // fetch decoder

    let Codec::EncapsulatedPixelData(Some(adapter), _) =
        JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.codec()
    else {
        panic!("JPEG 2000 pixel data reader not found")
    };

    let mut dest = vec![];

    adapter
        .decode_frame(&obj, 0, &mut dest)
        .expect("frame decoding failed");

    // inspect the result

    assert_eq!(dest.len(), 1024 * 256 * 2);

    // check a few known pixels

    // 0, 0
    check_w_monochrome_pixel(&dest, cols, 0, 0, 0);
    // 64, 154
    check_w_monochrome_pixel(&dest, cols, 64, 154, 0);
    // 135, 145
    check_w_monochrome_pixel(&dest, cols, 135, 145, 168);
    // 80, 188
    check_w_monochrome_pixel(&dest, cols, 80, 188, 9);
    // 136, 416
    check_w_monochrome_pixel(&dest, cols, 136, 416, 245);
}

/// writing to JPEG 2000 and back should yield approximately the same pixel data
#[test]
fn write_and_read_jpeg_2000() {
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

    // fetch decoder and encoder
    let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) =
        JPEG_2000_IMAGE_COMPRESSION.codec()
    else {
        panic!("JPEG 2000 pixel data adapters not found")
    };

    // request enough quality to admit some loss, but not too much
    let mut options = EncodeOptions::default();
    options.quality = Some(85);

    let mut encoded = vec![];

    let _ops = writer
        .encode_frame(&obj, 0, options, &mut encoded)
        .expect("frame encoding failed");

    // instantiate new object representing the compressed version

    let obj = TestDataObject {
        // JPEG 2000
        ts_uid: "1.2.840.10008.1.2.4.91".to_string(),
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
        .expect("frame decoding failed");

    // inspect the result
    assert_eq!(samples.len(), decoded.len(), "pixel data length mismatch");

    // traverse all pixels, compare with error margin
    let err_margin = 4;

    for (src_sample, decoded_sample) in samples.iter().copied().zip(decoded.iter().copied()) {
        assert!(
            src_sample.abs_diff(decoded_sample) <= err_margin,
            "pixel sample mismatch: {} vs {}",
            src_sample,
            decoded_sample
        );
    }
}
