//! Support for JPG image decoding.
use snafu::OptionExt;

use super::{CustomMessageSnafu, MissingAttributeSnafu};
use crate::adapters::{DecodeResult, PixelDataObject, PixelRWAdapter};
use jpeg_decoder::Decoder;
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
            return CustomMessageSnafu {
                message: "BitsAllocated other than 8 or 16 is not supported",
            }
            .fail();
        }

        let nr_frames = src.number_of_frames().unwrap_or(1) as usize;
        let nr_fragments = src.number_of_fragments().context(CustomMessageSnafu {
            message: "Invalid pixel data, no fragments found",
        })? as usize;
        if nr_frames != nr_fragments {
            return CustomMessageSnafu {
                message: "frame count differs from fragment count, Not implemented yet",
            }
            .fail();
        }
        let bytes_per_sample = bits_allocated / 8;

        // `stride` it the total number of bytes for each sample plane
        let stride: usize = (bytes_per_sample as usize * cols as usize * rows as usize).into();
        dst.resize((samples_per_pixel as usize * stride) * nr_frames, 0);

        let mut offset = 0;
        for i in 0..nr_frames {
            let fragment = &src.fragment(i).context(CustomMessageSnafu {
                message: "No pixel data found for frame",
            })?;
            let mut decoder = Decoder::new(Cursor::new(fragment));

            match decoder.decode() {
                Ok(decoded) => {
                    let decoded_len = decoded.len();
                    dst[offset..(offset + decoded_len)].copy_from_slice(&decoded);
                    offset += decoded_len
                }
                Err(_) => {
                    // TODO: Replace this with a result context error
                    // println!("{}", e);
                    return CustomMessageSnafu {
                        message: "Could not decode jpeg in frame",
                    }
                    .fail();
                }
            }
        }
        Ok(())
    }
}
