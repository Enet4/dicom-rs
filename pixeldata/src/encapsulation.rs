//! DICOM Pixel encapsulation
//!
//! This module implements encapsulation for pixel data.
use dicom_core::value::fragments::Fragments;
use dicom_core::value::Value;
use std::vec;

/// Encapsulate the pixel data of a list of frames.
///
/// Check [Fragments] in case more control over the processing is required.
///
/// # Example
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
/// let pixel_data = encapsulate(frames);
/// let element = DataElement::new(tags::PIXEL_DATA, OB, pixel_data);
/// ```
pub fn encapsulate(frames: Vec<Vec<u8>>) -> Value {
    let fragments = frames
        .into_iter()
        .map(|frame| Fragments::new(frame, 0))
        .collect::<Vec<Fragments>>();

    Value::PixelSequence(fragments.into())
}

/// Encapsulate the pixel data of a single frame. If `fragment_size` is zero then `frame.len()` will
/// be used instead.
///
/// # Example
/// ```
/// use dicom_core::DataElement;
/// use dicom_core::VR::OB;
/// use dicom_dictionary_std::tags;
/// use dicom_object::InMemDicomObject;
/// use dicom_pixeldata::encapsulation::encapsulate_single_frame;
///
/// // Frames are represented as Vec<Vec<u8>>
/// // Single 512x512 frame
/// let frame = vec![0; 262144];
/// let pixel_data = encapsulate_single_frame(frame, 0);
/// let element = DataElement::new(tags::PIXEL_DATA, OB, pixel_data);
/// ```
pub fn encapsulate_single_frame(frame: Vec<u8>, fragment_size: u32) -> Value {
    let fragments = vec![Fragments::new(frame, fragment_size)];

    Value::PixelSequence(fragments.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encapsulate() {
        if let Value::PixelSequence(enc) = encapsulate(vec![vec![20, 30, 40], vec![50, 60, 70, 80]])
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

    #[test]
    fn test_encapsulate_single_framme() {
        if let Value::PixelSequence(enc) = encapsulate_single_frame(vec![20, 30, 40], 1) {
            let offset_table = enc.offset_table();
            let fragments = enc.fragments();
            assert_eq!(offset_table.len(), 2);
            assert_eq!(fragments.len(), 2);
            assert_eq!(fragments[0].len(), 2);
            assert_eq!(fragments[1].len(), 2);
        } else {
            unreachable!("encapsulate should always return a PixelSequence");
        }
    }
}
