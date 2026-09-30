//! Test suite for Encapsulated Uncompressed Explicit VR Little Endian data reading and writing

mod adapters;

use adapters::TestDataObject;
use dicom_core::value::PixelFragmentSequence;
use dicom_encoding::{
    Codec,
    adapters::{EncodeOptions, PixelDataReader, PixelDataWriter},
};
use dicom_transfer_syntax_registry::entries::ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN;

fn gen_rgb_samples(rows: u16, columns: u16) -> Vec<u8> {
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

    samples
}

/// writing from Explicit VR Little Endian
/// to Encapsulated Uncompressed Explicit VR Little Endian
/// should not affect the pixel data value samples themselves
#[test]
fn write_and_read_frames() {
    let rows: u16 = 128;
    let columns: u16 = 256;
    let samples = gen_rgb_samples(rows, columns);

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

    // fetch adapter
    let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) =
        ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN.codec()
    else {
        panic!("Encapsulated Uncompressed Explicit VR Little Endian pixel data adapters not found")
    };

    let mut encoded = vec![];

    let _ops = writer
        .encode_frame(&obj, 0, EncodeOptions::default(), &mut encoded)
        .expect("Image Frame encoding failed");

    // encoded samples should be the same
    assert_eq!(&samples, &encoded, "pixel data mismatch");

    // instantiate new object representing the compressed version

    let obj = TestDataObject {
        // Encapsulated Uncompressed Explicit VR Little Endian
        ts_uid: "1.2.840.10008.1.2.1.98".to_string(),
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

    // should yield exactly the same data again
    assert_eq!(samples, decoded, "pixel data mismatch");
}

/// An object encoded to Encapsulated Uncompressed Explicit VR Little Endian
/// produces a suitable basic offset table.
#[test]
fn encode_whole_object_offset_table() {
    let rows = 256;
    let columns = 256;

    let mut samples = Vec::new();
    // generate 5 frames
    for _ in 0..5 {
        samples.extend(gen_rgb_samples(rows, columns));
    }
    let samples = samples;

    let frame_size = rows as u32 * columns as u32 * 3;

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
        number_of_frames: 5,
        flat_pixel_data: Some(samples.clone()),
        pixel_data_sequence: None,
    };

    // fetch adapter
    let Codec::EncapsulatedPixelData(_, Some(writer)) =
        ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN.codec()
    else {
        panic!("Encapsulated Uncompressed Explicit VR Little Endian pixel data adapters not found")
    };

    let mut encoded = Vec::new();
    let mut offset_table = Vec::new();

    let _ops = writer
        .encode(
            &obj,
            EncodeOptions::default(),
            &mut encoded,
            &mut offset_table,
        )
        .expect("image encoding failed");

    // expect 4 fragments, 1 per frame
    assert_eq!(encoded.len(), 5);

    // expect these items in offset table
    // (easy to calculate, since pixel data samples are unaffected)
    assert_eq!(
        &offset_table,
        &[
            // fragment 0
            0,
            // fragment 1
            frame_size + 8,
            // fragment 2
            (frame_size + 8) * 2,
            // fragment 3
            (frame_size + 8) * 3,
            // fragment 4
            (frame_size + 8) * 4,
        ]
    );
}
