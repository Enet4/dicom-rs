//! Test suite for Deflated Image Frame Compression pixel data reading and writing
#![cfg(feature = "deflate")]

mod adapters;

use adapters::TestDataObject;
use dicom_core::value::PixelFragmentSequence;
use dicom_encoding::{
    adapters::{EncodeOptions, PixelDataReader, PixelDataWriter},
    Codec,
};
use dicom_transfer_syntax_registry::entries::DEFLATED_IMAGE_FRAME_COMPRESSION;

/// writing to Deflated Image Frame Compression and back
/// should yield exactly the same pixel data
#[test]
fn write_and_read_deflated_frames() {
    let rows: u16 = 128;
    let columns: u16 = 256;

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

    // fetch adapters for Deflated Image Frame Compression

    let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) = DEFLATED_IMAGE_FRAME_COMPRESSION.codec() else {
        panic!("Deflated Image Frame Compression pixel data adapters not found")
    };

    let mut encoded = vec![];

    let _ops = writer
        .encode_frame(&obj, 0, EncodeOptions::default(), &mut encoded)
        .expect("Deflated Image Frame encoding failed");

    // instantiate new object representing the compressed version

    let obj = TestDataObject {
        // Deflated Image Frame Compression
        ts_uid: "1.2.840.10008.1.2.8.1".to_string(),
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
        .expect("Deflated Image Frame decoding failed");

    // compare pixels, lossless encoding should yield exactly the same data
    assert_eq!(samples, decoded, "pixel data mismatch");
}
