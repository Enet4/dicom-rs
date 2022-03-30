//! Support for JPG image decoding.
use byteordered::byteorder::{ByteOrder, LittleEndian};
use snafu::{OptionExt, ResultExt};

use crate::adapters::{DecodeResult, PixelDataObject, PixelRWAdapter};
use std::io::{self, Read, Seek};

use super::{CustomMessageSnafu, CustomSnafu, MissingAttributeSnafu};

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
            return CustomMessageSnafu {
                message: "BitsAllocated other than 8 or 16 is not supported",
            }
            .fail();
        }
        // For RLE the number of fragments = number of frames
        // therefore, we can fetch the fragments one-by-one
        let nr_frames = src.number_of_fragments().context(CustomMessageSnafu {
            message: "Invalid pixel data, no fragments found",
        })? as usize;
        let bytes_per_sample = bits_allocated / 8;
        // `stride` it the total number of bytes for each sample plane
        let stride = bytes_per_sample * cols * rows;
        dst.resize((samples_per_pixel * stride) as usize * nr_frames, 0);
        todo!();
        Ok(())
    }
}
