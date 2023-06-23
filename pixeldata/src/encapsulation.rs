//! DICOM Pixel encapsulation
//!
//! This module implements encapsulation for pixel data.
use dicom_core::value::fragments::Fragments;
use dicom_core::value::Value;

/// Encapsulate the pixel data of the frames. If fragment_size > 0 it will use 1 fragment per frame.
/// This parameter is ignored for multi frame data, as 1 fragment per frame is required.
///
/// Check [Fragments] in case more control over the processing is required.
///
/// #Example
/// ```
/// use dicom_core::DataElement;
/// use dicom_core::VR::OB;
/// use dicom_dictionary_std::tags;
/// use dicom_object::InMemDicomObject;
/// use dicom_pixeldata::encapsulation::encapsulate;
///
/// // Frames are represented as Vec<Vec<u8>>
/// // Single 512x512 frame
/// let frames = vec![vec![0; 262144]];
/// let pixel_data = encapsulate(frames, 0);
/// let element = DataElement::new(tags::PIXEL_DATA, OB, pixel_data);
/// ```
pub fn encapsulate(frames: Vec<Vec<u8>>, fragment_size: u32) -> Value {
    let fragment_size = if frames.len() > 1 { 0 } else { fragment_size };

    let fragments = frames
        .into_iter()
        .map(|frame| Fragments::new(frame, fragment_size))
        .collect::<Vec<Fragments>>();

    Value::PixelSequence(fragments.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encapsulated_pixels() {
        if let Value::PixelSequence(enc) =
            encapsulate(vec![vec![20, 30, 40], vec![50, 60, 70, 80]], 0)
        {
            let offset_table = enc.offset_table();
            let fragments = enc.fragments();
            assert_eq!(offset_table.len(), 3);
            assert_eq!(fragments.len(), 2);
            assert_eq!(fragments[0].len(), 4);
            assert_eq!(fragments[1].len(), 4);
        } else {
            unreachable!("encapsulate should always return a PixelSequence");
        }

        if let Value::PixelSequence(enc) = encapsulate(vec![vec![20, 30, 40]], 1) {
            let offset_table = enc.offset_table();
            let fragments = enc.fragments();
            assert_eq!(offset_table.len(), 2);
            assert_eq!(fragments.len(), 2);
            assert_eq!(fragments[0].len(), 2);
            assert_eq!(fragments[1].len(), 2);
        } else {
            unreachable!("encapsulate should always return a PixelSequence");
        }

        if let Value::PixelSequence(enc) =
            encapsulate(vec![vec![20, 30, 40], vec![50, 60, 70, 80]], 2)
        {
            let offset_table = enc.offset_table();
            let fragments = enc.fragments();
            assert_eq!(offset_table.len(), 3);
            assert_eq!(fragments.len(), 2);
            assert_eq!(fragments[0].len(), 4);
            assert_eq!(fragments[1].len(), 4);
        } else {
            unreachable!("encapsulate should always return a PixelSequence");
        }
    }
}
