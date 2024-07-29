//! Support for JPEG 2000 image decoding.

use charls::CharLS;
use dicom_encoding::adapters::{decode_error, DecodeResult, PixelDataObject, PixelDataReader};
use dicom_encoding::snafu::prelude::*;
use std::borrow::Cow;

/// Pixel data adapter for JPEG-LS transfer syntax.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JpegLSAdapter;

impl PixelDataReader for JpegLSAdapter {
    /// Decode a single frame in JPEG 2000 from a DICOM object.
    fn decode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        dst: &mut Vec<u8>,
    ) -> DecodeResult<()> {
        let bits_allocated = src
            .bits_allocated()
            .context(decode_error::MissingAttributeSnafu {
                name: "BitsAllocated",
            })?;

        ensure_whatever!(
            bits_allocated == 8 || bits_allocated == 16,
            "BitsAllocated other than 8 or 16 is not supported"
        );

        let nr_frames = src.number_of_frames().unwrap_or(1) as usize;

        ensure!(
            nr_frames > frame as usize,
            decode_error::FrameRangeOutOfBoundsSnafu
        );

        let raw = src
            .raw_pixel_data()
            .whatever_context("Expected to have raw pixel data available")?;

        let frame_data = if raw.fragments.len() == 1 || raw.fragments.len() == nr_frames {
            // assuming 1:1 frame-to-fragment mapping
            Cow::Borrowed(
                raw.fragments
                    .get(frame as usize)
                    .with_whatever_context(|| {
                        format!("Missing fragment #{} for the frame requested", frame)
                    })?,
            )
        } else {
            // Some embedded JPEGs might span multiple fragments.
            // In this case we look up the basic offset table
            // and gather all of the frame's fragments in a single vector.
            // Note: not the most efficient way to do this,
            // consider optimizing later with byte chunk readers
            let base_offset = raw.offset_table.get(frame as usize).copied();
            let base_offset = if frame == 0 {
                base_offset.unwrap_or(0) as usize
            } else {
                base_offset
                    .with_whatever_context(|| format!("Missing offset for frame #{}", frame))?
                    as usize
            };
            let next_offset = raw.offset_table.get(frame as usize + 1);

            let mut offset = 0;
            let mut fragments = Vec::new();
            for fragment in &raw.fragments {
                // include it
                if offset >= base_offset {
                    fragments.extend_from_slice(fragment);
                }
                offset += fragment.len() + 8;
                if let Some(&next_offset) = next_offset {
                    if offset >= next_offset as usize {
                        // next fragment is for the next frame
                        break;
                    }
                }
            }

            Cow::Owned(fragments)
        };

        let mut image = CharLS::default()
            .decode(&frame_data, 0)
            .map_err(|error| error.to_string())
            .with_whatever_context(|error| error.to_string())?;

        dst.append(&mut image);

        Ok(())
    }
}
