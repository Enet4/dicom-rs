//! Support for JPG image decoding.

use super::MissingAttributeSnafu;
use crate::adapters::{DecodeResult, PixelDataObject, PixelRWAdapter};
use jpeg_decoder::Decoder;
use snafu::{whatever, OptionExt, ResultExt};
use std::io::Cursor;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JPEGAdapter;

impl PixelRWAdapter for JPEGAdapter {
    /// Decode DICOM image data with jpeg encoding.
    fn decode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> DecodeResult<()> {
        let cols = src
            .cols()
            .context(MissingAttributeSnafu { name: "Columns" })?;
        let rows = src.rows().context(MissingAttributeSnafu { name: "Rows" })?;
        let samples_per_pixel = src.samples_per_pixel().context(MissingAttributeSnafu {
            name: "SamplesPerPixel",
        })?;
        let bits_allocated = src.bits_allocated().context(MissingAttributeSnafu {
            name: "BitsAllocated",
        })?;

        if bits_allocated != 8 && bits_allocated != 16 {
            whatever!("BitsAllocated other than 8 or 16 is not supported");
        }

        let nr_frames = src.number_of_frames().unwrap_or(1) as usize;
        let bytes_per_sample = bits_allocated / 8;

        // `stride` it the total number of bytes for each sample plane
        let stride: usize = bytes_per_sample as usize * cols as usize * rows as usize;
        dst.resize((samples_per_pixel as usize * stride) * nr_frames, 0);

        // Embedded jpegs can span multiple fragments
        // Hence we collect all fragments into single vector
        // and then just iterate a cursor for each frame
        let fragments: Vec<u8> = src
            .raw_pixel_data()
            .whatever_context("Expected to have raw pixel data available")?
            .fragments
            .into_iter()
            .flatten()
            .collect();

        let fragments_len = fragments.len() as u64;
        let mut cursor = Cursor::new(fragments);
        let mut dst_offset = 0;

        loop {
            let mut decoder = Decoder::new(&mut cursor);
            let decoded = decoder
                .decode()
                .map_err(|e| Box::new(e) as Box<_>)
                .whatever_context("JPEG decoder failure")?;

            let decoded_len = decoded.len();
            dst[dst_offset..(dst_offset + decoded_len)].copy_from_slice(&decoded);
            dst_offset += decoded_len;

            // dicom fields always have to have an even length and fill this space with padding
            // if uneven we have to move one position further to consume this padding
            if cursor.position() % 2 > 0 {
                cursor.set_position(cursor.position() + 1);
            }

            if cursor.position() >= fragments_len {
                break;
            }
        }

        Ok(())
    }
}
