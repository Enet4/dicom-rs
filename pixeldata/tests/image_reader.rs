//! Test module for pixel data readers (adapters).
//!
//! This test suite deliberately avoids the use of the decoded pixel data API
//! for a greater focus on covering
//! pixel data reading and writing capabilities
//! from transfer syntax implementations.

#[cfg(feature = "native")]
mod jpeg {

    // always compare with an error margin,
    // since JPEG baseline is lossy
    const L1_ERROR_THRESHOLD: u32 = 8;
    fn pixel_value_matches(got: [u8; 3], expected: [u8; 3]) {
        let error = got[0].abs_diff(expected[0]) as u32
            + got[1].abs_diff(expected[1]) as u32
            + got[2].abs_diff(expected[2]) as u32;

        assert!(
            error <= L1_ERROR_THRESHOLD,
            "pixel values {:?} does not match with expected {:?}",
            got,
            expected
        );
    }

    use std::convert::TryInto;

    use dicom_encoding::adapters::DecodeError;
    use dicom_encoding::adapters::PixelDataObject;
    use dicom_encoding::transfer_syntax::Codec;
    use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
    use dicom_object::open_file;
    use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

    #[test]
    fn can_read_jpeg() {
        let file_path = dicom_test_files::path("pydicom/color3d_jpeg_baseline.dcm").unwrap();
        let obj = open_file(file_path).unwrap();

        let ts_jpeg = TransferSyntaxRegistry
            .get("1.2.840.10008.1.2.4.50")
            .expect("Transfer syntax _JPEG Baseline_ should be registered for this test");

        // can fetch reader

        let Codec::EncapsulatedPixelData(Some(img_reader), _img_writer) = ts_jpeg.codec() else {
            panic!("Missing image reader in _JPEG Baseline_ transfer syntax impl");
        };

        // can read one frame
        let mut dst = Vec::new();
        img_reader
            .decode_frame(&obj, 0, &mut dst)
            .expect("Failed to decode frame");

        let columns = obj.cols().expect("DICOM object should have Columns");
        assert_eq!(columns, 640, "Unexpected image width");
        let rows = obj.rows().expect("DICOM object should have Columns");
        assert_eq!(rows, 480, "Unexpected image height");
        let columns = columns as usize;
        let rows = rows as usize;

        // curated expected pixel data values at the given X,Y coordinates
        // in frame 0
        let pixel_gt = [
            // pixel at (0,0): (1,1,3)
            ((0, 0), [1, 1, 3]),
            // and so on
            ((15, 19), [81, 115, 163]),
            ((70, 33), [118, 118, 118]),
            ((380, 39), [74, 166, 125]),
            // begin ultrasound region
            ((313, 190), [104, 104, 104]),
            ((350, 256), [122, 122, 122]),
            // end ultrasound region
            ((512, 460), [28, 35, 45]),
        ];

        for ((x, y), expected) in pixel_gt {
            let i = (y * columns + x) * 3;
            pixel_value_matches(dst[i..i + 3].try_into().unwrap(), expected);
        }
        let frame_size = columns * rows * 3;
        assert_eq!(dst.len(), frame_size);

        // decode a different frame, keeping the previous one in memory
        img_reader
            .decode_frame(&obj, 63, &mut dst)
            .expect("Failed to decode frame");

        // curated expected pixel data values at the given X,Y coordinates
        // in frame 63
        let pixel_gt = [
            ((0, 0), [1, 1, 3]),
            ((15, 19), [81, 115, 163]),
            ((70, 33), [118, 118, 118]),
            ((380, 39), [74, 166, 125]),
            // only the ultrasound region changes
            ((313, 190), [84, 84, 84]),
            ((350, 256), [6, 6, 6]),
            // end ultrasound region
            ((512, 460), [28, 35, 45]),
        ];
        for ((x, y), expected) in pixel_gt {
            let i = frame_size + (y * columns + x) * 3;
            pixel_value_matches(dst[i..i + 3].try_into().unwrap(), expected);
        }
        assert_eq!(dst.len(), frame_size * 2);

        // cannot read out of bounds
        assert!(matches!(
            img_reader.decode_frame(&obj, 120, &mut dst),
            Err(DecodeError::FrameRangeOutOfBounds),
        ));
    }
}
