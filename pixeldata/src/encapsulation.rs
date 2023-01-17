//! DICOM Pixel encapsulation
//!
//! This module implements encapsulation for pixel data.
use dicom_core::value::{Value, C};
use dicom_core::DataDictionary;
use dicom_object::mem::InMemFragment;
use dicom_object::InMemDicomObject;
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("More than 1 fragment per frame is invalid for multi frame pixel data"))]
    FragmentedMultiframe,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The encapsulated pixel data is composed of the offset table and fragments.
///
/// Each element of the offset table are byte offsets to the first byte of the item tag of the first
/// fragment of each frame.
///
/// For single frame, the image can be split into any amount of fragments. For multi frame files, it
/// has to be 1 fragment per frame.
///
/// The simplest way to encapsulate the data would be to use `encapsulate`
///
/// # Example
///
/// ```no_run
/// use dicom_core::DataElement;
/// use dicom_core::VR::OB;
/// use dicom_dictionary_std::tags::PIXEL_DATA;
/// use dicom_pixeldata::encapsulation::encapsulate;
///
/// # let frames: Vec<Vec<u8>> = vec![vec![]];
/// let encapsulated_pixels = encapsulate(frames, 0);
///
/// # let mut dcm = dicom_object::InMemDicomObject::new_empty();
/// dcm.put(DataElement::new(PIXEL_DATA, OB, encapsulated_pixels));
/// ```
///
/// When dealing with big multi frame images, to save memory each frame can be processed individually
///
/// # Example
///
/// ```no_run
/// use dicom_core::DataElement;
/// use dicom_core::VR::OB;
/// use dicom_dictionary_std::tags::PIXEL_DATA;
/// use dicom_pixeldata::encapsulation::EncapsulatedPixels;
///
/// # let frames: Vec<Vec<u8>> = vec![vec![]];
/// let mut encapsulated_pixels = EncapsulatedPixels::default();
/// let encapsulated_data = frames
///     .into_iter()
///     .for_each(|encoded_frame| {
///         encapsulated_pixels.add_frame(encoded_frame, 0)
///             .expect("For multi frame data only 1 fragment per frame is allowed");
///     });
///
/// # let mut dcm = dicom_object::InMemDicomObject::new_empty();
/// dcm.put(DataElement::new(PIXEL_DATA, OB, encapsulated_pixels));
/// ```
/// Internally all the both ways use `fragment_frame`, it can be used if more control is required:
///
/// # Example
///
/// ```no_run
/// use rayon::prelude::*;
/// use dicom_core::DataElement;
/// use dicom_core::VR::OB;
/// use dicom_dictionary_std::tags::PIXEL_DATA;
/// use dicom_pixeldata::encapsulation::*;
///
/// # let frames: Vec<Vec<u8>> = vec![vec![]];
/// let mut encapsulated_pixels = EncapsulatedPixels::default();
/// let encapsulated_data: Vec<FrameFragments> = frames
///     .into_par_iter()
///     .map(|encoded_frame| fragment_frame(encoded_frame, 0))
///     .collect();
///
/// let encapsulated_pixels = EncapsulatedPixels::from_frame_fragments(encapsulated_data)
///     .expect("For multi frame data only 1 fragment per frame is allowed");
///
/// # let mut dcm = dicom_object::InMemDicomObject::new_empty();
/// dcm.put(DataElement::new(PIXEL_DATA, OB, encapsulated_pixels));
/// ```
#[derive(Debug, Default)]
pub struct EncapsulatedPixels {
    offset_table: C<u32>,
    fragments: C<Vec<u8>>,
}

/// This represents a single fragmented frame. It can contain 1 or more fragments.
/// Usually it is created by `fragment_frame`
#[derive(Debug)]
pub struct FrameFragments {
    fragments: Vec<Vec<u8>>,
}

impl EncapsulatedPixels {
    /// Add a single frame
    pub fn add_frame(&mut self, data: Vec<u8>, fragment_size: u32) -> Result<()>{
        let number_of_fragments = self.fragments.len();
        if number_of_fragments > 0 {
            let offsets_size = self.offset_table.len();
            if number_of_fragments > offsets_size && fragment_size > 1 {
                return Err(Error::FragmentedMultiframe);
            }
        }

        let fragments = fragment_frame(data, fragment_size);
        let frame_offset = fragments.len();
        for fragment in fragments.fragments {
            self.fragments.push(fragment.to_vec());
        }
        self.add_offset(frame_offset);

        Ok(())
    }

    /// Create EncapsulatedPixels from a list of FrameFragments
    pub fn from_frame_fragments(frames: Vec<FrameFragments>) -> Result<Self> {
        let mut offset_table = C::with_capacity(frames.len() + 1);
        offset_table.push(0u32);
        let mut current_offset = 0u32;

        let mut fragments = Vec::new();
        let is_multiframe = frames.len() > 1;

        for mut frame in frames {
            if frame.is_multiframe() && is_multiframe {
                return Err(Error::FragmentedMultiframe);
            }

            let offset = frame.len();
            offset_table.push(current_offset + offset);
            current_offset += offset;

            fragments.append(&mut frame.fragments);
        }

        Ok(EncapsulatedPixels {
            offset_table,
            fragments: fragments.into(),
        })
    }

    /// Add an offset to the offsets table
    fn add_offset(&mut self, offset: u32) {
        let last = match self.offset_table.last() {
            Some(el) => *el,
            None => {
                self.offset_table.push(0u32);
                0u32
            }
        };

        self.offset_table.push(last + offset);
    }
}

impl<D> From<EncapsulatedPixels> for Value<InMemDicomObject<D>, InMemFragment>
where
    D: DataDictionary + Clone,
{
    fn from(value: EncapsulatedPixels) -> Self {
        let offset_table = if value.offset_table.len() > 1 {
            let ot_size = value.offset_table.len() - 1;
            let mut ot = C::with_capacity(ot_size);
            for v in 0..ot_size {
                ot.push(value.offset_table[v]);
            }
            ot
        } else {
            value.offset_table
        };

        Value::PixelSequence {
            offset_table,
            fragments: value.fragments,
        }
    }
}

impl FrameFragments {
    pub fn is_empty(&self) -> bool {
        self.fragments.len() == 0
    }

    pub fn is_multiframe(&self) -> bool {
        self.fragments.len() > 1
    }

    pub fn len(&self) -> u32 {
        self.fragments
            .iter()
            .fold(0u32, |acc, fragment| acc + fragment.len() as u32 + 8u32)
    }
}

/// Create the fragments for a single frame. It returns a list with the fragments.
pub fn fragment_frame(data: Vec<u8>, fragment_size: u32) -> FrameFragments {
    let fragment_size: u32 = if fragment_size == 0 {
        data.len() as u32
    } else {
        fragment_size
    };

    let fragment_size = if fragment_size % 2 == 0 {
        fragment_size
    } else {
        fragment_size + 1
    };

    let number_of_fragments = (data.len() as f32 / fragment_size as f32).ceil() as u32;

    // Calculate the encapsulated size. If necessary pad the vector with zeroes so all the
    // chunks have the same fragment_size
    let mut data = data;
    let encapsulated_size = (fragment_size * number_of_fragments) as usize;
    if encapsulated_size > data.len() {
        data.resize(encapsulated_size, 0);
    }

    let fragments = data
        .chunks_exact(fragment_size as usize)
        .map(|fragment| fragment.to_vec())
        .collect::<Vec<Vec<u8>>>();

    FrameFragments { fragments }
}

/// Encapsulate the pixel data of the frames. If fragment_size > 0 it will use 1 fragment per frame.
/// This parameter is ignored for multi frame data, as 1 fragment per frame is required.
pub fn encapsulate(frames: Vec<Vec<u8>>, fragment_size: u32) -> EncapsulatedPixels {
    let fragment_size = if frames.len() > 1 { 0 } else { fragment_size };
    let mut encapsulated_data = EncapsulatedPixels::default();

    for frame in frames {
        encapsulated_data.add_frame(frame, fragment_size)
            .expect("For multi frame data only 1 fragment per frame is allowed");
    }

    encapsulated_data
}

#[cfg(test)]
mod tests {
    use crate::encapsulation::{encapsulate, fragment_frame, EncapsulatedPixels};

    #[test]
    fn test_add_frame() {
        let mut enc = EncapsulatedPixels::default();
        assert_eq!(enc.offset_table.len(), 0);
        assert_eq!(enc.fragments.len(), 0);

        enc.add_frame(vec![10, 20, 30], 0).unwrap();
        assert_eq!(enc.offset_table.len(), 2);
        assert_eq!(enc.fragments.len(), 1);
        assert_eq!(enc.offset_table[0], 0);
        assert_eq!(enc.offset_table[1], 12);

        enc.add_frame(vec![10, 20, 30, 50], 0).unwrap();
        assert_eq!(enc.offset_table.len(), 3);
        assert_eq!(enc.fragments.len(), 2);
        assert_eq!(enc.offset_table[2], 24);
    }

    #[test]
    fn test_encapsulated_pixels() {
        let enc = encapsulate(vec![vec![20, 30, 40], vec![50, 60, 70, 80]], 0);
        assert_eq!(enc.offset_table.len(), 3);
        assert_eq!(enc.fragments.len(), 2);
        assert_eq!(enc.fragments[0].len(), 4);
        assert_eq!(enc.fragments[1].len(), 4);

        let enc = encapsulate(vec![vec![20, 30, 40]], 1);
        assert_eq!(enc.offset_table.len(), 2);
        assert_eq!(enc.fragments.len(), 2);
        assert_eq!(enc.fragments[0].len(), 2);
        assert_eq!(enc.fragments[1].len(), 2);

        let enc = encapsulate(vec![vec![20, 30, 40], vec![50, 60, 70, 80]], 2);
        assert_eq!(enc.offset_table.len(), 3);
        assert_eq!(enc.fragments.len(), 2);
        assert_eq!(enc.fragments[0].len(), 4);
        assert_eq!(enc.fragments[1].len(), 4);
    }

    #[test]
    fn test_fragment_frame() {
        let fragment = fragment_frame(vec![150, 164, 200], 0);
        assert_eq!(fragment.fragments.len(), 1, "1 fragment should be present");
        assert_eq!(
            fragment.fragments[0].len(),
            4,
            "The fragment size should be 4"
        );
        assert_eq!(
            fragment.fragments[0],
            vec![150, 164, 200, 0],
            "The data should be 0 padded"
        );

        let fragment = fragment_frame(vec![150, 164, 200, 222], 4);
        assert_eq!(fragment.fragments.len(), 1, "1 fragment should be present");
        assert_eq!(
            fragment.fragments[0].len(),
            4,
            "The fragment size should be 4"
        );
        assert_eq!(
            fragment.fragments[0],
            vec![150, 164, 200, 222],
            "The data should be what was sent"
        );

        let fragment = fragment_frame(vec![150, 164, 200, 222], 2);
        assert_eq!(fragment.fragments.len(), 2, "2 fragments should be present");
        assert_eq!(fragment.fragments[0].len(), 2);
        assert_eq!(fragment.fragments[1].len(), 2);
        assert_eq!(fragment.fragments[0], vec![150, 164]);
        assert_eq!(fragment.fragments[1], vec![200, 222]);

        let fragment = fragment_frame(vec![150, 164, 200], 1);
        assert_eq!(
            fragment.fragments.len(),
            2,
            "2 fragments should be present as fragment_size < 2"
        );
        assert_eq!(fragment.fragments[0].len(), 2);
        assert_eq!(fragment.fragments[0], vec![150, 164]);
        assert_eq!(fragment.fragments[1].len(), 2);
        assert_eq!(fragment.fragments[1], vec![200, 0]);

        let fragment = fragment_frame(vec![150, 164, 200, 222], 1);
        assert_eq!(
            fragment.fragments.len(),
            2,
            "2 fragments should be present as fragment_size < 2"
        );
        assert_eq!(fragment.fragments[0].len(), 2);
        assert_eq!(fragment.fragments[0], vec![150, 164]);
        assert_eq!(fragment.fragments[1].len(), 2);
        assert_eq!(fragment.fragments[1], vec![200, 222]);
    }
}
