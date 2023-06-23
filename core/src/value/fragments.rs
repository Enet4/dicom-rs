//! Handle pixel encapsulation into fragments
use crate::value::{InMemFragment, PixelFragmentSequence, C};

/// Represents the fragments of a single frame. [PixelFragmentSequence] can be generated from a list
/// of [Fragments]. In case of multi frame a list of frames composed by 1 fragment is expected.
///
/// The frames can be independently processed, so parallel execution is possible.
///
/// # Example
/// ```
/// // Frames are represented as Vec<Vec<u8>>
/// use dicom_core::{DataElement, Tag};
/// use dicom_core::header::EmptyObject;
/// use dicom_core::value::Value::PixelSequence;
/// use dicom_core::value::fragments::Fragments;
/// use dicom_core::value::InMemFragment;
/// use dicom_core::VR::OB;
///
/// // Single 512x512 frame
/// let frames = vec![vec![0; 262144]];
/// let fragments = frames
///     .into_iter()
///     .map(|frame| Fragments::new(frame, 0))
///     .collect::<Vec<Fragments>>();
///
/// let element = DataElement::new(Tag(0x7FE0, 0x0008), OB, PixelSequence::<EmptyObject, InMemFragment>(fragments.into()));
/// ```
///
/// From this last example, it is possible to extend it to implement a pipeline, and even use rayon
/// process the frames.
#[derive(Debug)]
pub struct Fragments {
    fragments: Vec<InMemFragment>,
}

impl Fragments {
    pub fn new(data: Vec<u8>, fragment_size: u32) -> Self {
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
            .collect::<Vec<InMemFragment>>();

        Fragments { fragments }
    }

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

impl From<Vec<Fragments>> for PixelFragmentSequence<Vec<u8>> {
    fn from(value: Vec<Fragments>) -> Self {
        let mut offset_table = C::with_capacity(value.len() + 1);
        offset_table.push(0u32);
        let mut current_offset = 0u32;

        let mut fragments = Vec::new();
        let is_multiframe = value.len() > 1;

        for mut frame in value {
            if frame.is_multiframe() && is_multiframe {
                panic!("More than 1 fragment per frame is invalid for multi frame pixel data");
            }

            let offset = frame.len();
            offset_table.push(current_offset + offset);
            current_offset += offset;

            fragments.append(&mut frame.fragments);
        }

        PixelFragmentSequence {
            offset_table,
            fragments: C::from_vec(fragments),
        }
    }
}
