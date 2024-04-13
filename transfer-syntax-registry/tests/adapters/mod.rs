//! Utility module for testing pixel data adapters.
use std::borrow::Cow;

use dicom_core::value::{InMemFragment, PixelFragmentSequence};
use dicom_encoding::adapters::{PixelDataObject, RawPixelData};

/// A test data object.
///
/// Can be used to test pixel data adapters
/// without having to open a real DICOM file using `dicom_object`.
#[derive(Debug)]
pub(crate) struct TestDataObject {
    pub ts_uid: String,
    pub rows: u16,
    pub columns: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub samples_per_pixel: u16,
    pub photometric_interpretation: &'static str,
    pub number_of_frames: u32,
    pub flat_pixel_data: Option<Vec<u8>>,
    pub pixel_data_sequence: Option<PixelFragmentSequence<InMemFragment>>,
}

impl PixelDataObject for TestDataObject {
    fn transfer_syntax_uid(&self) -> &str {
        &self.ts_uid
    }

    fn rows(&self) -> Option<u16> {
        Some(self.rows)
    }

    fn cols(&self) -> Option<u16> {
        Some(self.columns)
    }

    fn samples_per_pixel(&self) -> Option<u16> {
        Some(self.samples_per_pixel)
    }

    fn bits_allocated(&self) -> Option<u16> {
        Some(self.bits_allocated)
    }

    fn bits_stored(&self) -> Option<u16> {
        Some(self.bits_stored)
    }

    fn photometric_interpretation(&self) -> Option<&str> {
        Some(&self.photometric_interpretation)
    }

    fn number_of_frames(&self) -> Option<u32> {
        Some(self.number_of_frames)
    }

    fn number_of_fragments(&self) -> Option<u32> {
        match &self.pixel_data_sequence {
            Some(v) => Some(v.fragments().len() as u32),
            _ => None,
        }
    }

    fn fragment(&self, fragment: usize) -> Option<Cow<[u8]>> {
        match (&self.flat_pixel_data, &self.pixel_data_sequence) {
            (Some(_), Some(_)) => {
                panic!("Invalid pixel data object (both flat and fragment sequence)")
            }
            (_, Some(v)) => v
                .fragments()
                .get(fragment)
                .map(|f| Cow::Borrowed(f.as_slice())),
            (Some(v), _) => {
                if fragment == 0 {
                    Some(Cow::Borrowed(v))
                } else {
                    None
                }
            }
            (None, None) => None,
        }
    }

    fn offset_table(&self) -> Option<Cow<[u32]>> {
        match &self.pixel_data_sequence {
            Some(v) => Some(Cow::Borrowed(v.offset_table())),
            _ => None,
        }
    }

    fn raw_pixel_data(&self) -> Option<RawPixelData> {
        match (&self.flat_pixel_data, &self.pixel_data_sequence) {
            (Some(_), Some(_)) => {
                panic!("Invalid pixel data object (both flat and fragment sequence)")
            }
            (Some(v), _) => Some(RawPixelData {
                fragments: vec![v.clone()].into(),
                offset_table: Default::default(),
            }),
            (_, Some(v)) => Some(RawPixelData {
                fragments: v.fragments().into(),
                offset_table: v.offset_table().into(),
            }),
            _ => None,
        }
    }
}
