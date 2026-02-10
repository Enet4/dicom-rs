//! Support for JPEG 2000 image decoding.

use dicom_encoding::adapters::{decode_error, DecodeResult, PixelDataObject, PixelDataReader};
use dicom_encoding::snafu::prelude::*;
use jpeg2k::Image;
use tracing::warn;

// Check jpeg2k backend conflicts
#[cfg(all(feature = "openjp2", feature = "openjpeg-sys"))]
compile_error!(
    "feature \"openjp2\" and feature \"openjpeg-sys\" cannot be enabled at the same time"
);

/// Pixel data adapter for transfer syntaxes based on JPEG 2000.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Jpeg2000Adapter;

impl PixelDataReader for Jpeg2000Adapter {
    /// Decode a single frame in JPEG 2000 from a DICOM object.
    fn decode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        dst: &mut Vec<u8>,
    ) -> DecodeResult<()> {
        let cols = src
            .cols()
            .context(decode_error::MissingAttributeSnafu { name: "Columns" })?;
        let rows = src
            .rows()
            .context(decode_error::MissingAttributeSnafu { name: "Rows" })?;
        let samples_per_pixel =
            src.samples_per_pixel()
                .context(decode_error::MissingAttributeSnafu {
                    name: "SamplesPerPixel",
                })?;
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

        let bytes_per_sample = bits_allocated / 8;

        // `stride` it the total number of bytes for each sample plane
        let stride: usize = bytes_per_sample as usize * cols as usize * rows as usize;
        dst.reserve_exact(samples_per_pixel as usize * stride);
        let base_offset = dst.len();
        dst.resize(base_offset + (samples_per_pixel as usize * stride), 0);

        let frame_data = src
            .frame_pixel_data(frame)
            .whatever_context("Missing frame pixeldata")?;
        let image = Image::from_bytes(&frame_data).whatever_context("jpeg2k decoder failure")?;

        // Note: we cannot use `get_pixels`
        // because the current implementation narrows the data
        // down to 8 bits per sample
        let components = image.components();

        // write each component into the destination buffer
        for (component_i, component) in components.iter().enumerate() {
            if component_i > samples_per_pixel as usize {
                warn!(
                    "JPEG 2000 image has more components than expected ({} > {})",
                    component_i, samples_per_pixel
                );
                break;
            }

            // write in standard layout
            for (i, sample) in component.data().iter().enumerate() {
                let offset = base_offset
                    + i * samples_per_pixel as usize * bytes_per_sample as usize
                    + component_i * bytes_per_sample as usize;
                dst[offset..offset + bytes_per_sample as usize]
                    .copy_from_slice(&sample.to_le_bytes()[..bytes_per_sample as usize]);
            }
        }

        Ok(())
    }
}
