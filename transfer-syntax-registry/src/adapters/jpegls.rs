//! Support for JPEG-LS image decoding.

use charls::{CharLS, FrameInfo};
use dicom_core::ops::{AttributeAction, AttributeOp};
use dicom_core::Tag;
use dicom_encoding::adapters::{
    decode_error, encode_error, DecodeResult, EncodeResult, PixelDataObject, PixelDataReader,
    PixelDataWriter,
};
use dicom_encoding::snafu::prelude::*;
use std::borrow::Cow;

/// Pixel data reader and writer for JPEG-LS transfer syntaxes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JpegLsAdapter;

/// Pixel data writer specifically for JPEG-LS lossless.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct JpegLsLosslessWriter;

impl PixelDataReader for JpegLsAdapter {
    /// Decode a single frame in JPEG-LS from a DICOM object.
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

        let mut decoded = CharLS::default()
            .decode(&frame_data)
            .map_err(|error| error.to_string())
            .with_whatever_context(|error| error.to_string())?;

        dst.append(&mut decoded);

        Ok(())
    }
}

impl PixelDataWriter for JpegLsAdapter {
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        options: dicom_encoding::adapters::EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        let cols = src
            .cols()
            .context(encode_error::MissingAttributeSnafu { name: "Columns" })?;
        let rows = src
            .rows()
            .context(encode_error::MissingAttributeSnafu { name: "Rows" })?;
        let samples_per_pixel =
            src.samples_per_pixel()
                .context(encode_error::MissingAttributeSnafu {
                    name: "SamplesPerPixel",
                })?;
        let bits_allocated = src
            .bits_allocated()
            .context(encode_error::MissingAttributeSnafu {
                name: "BitsAllocated",
            })?;
        let bits_stored = src
            .bits_stored()
            .context(encode_error::MissingAttributeSnafu { name: "BitsStored" })?;

        ensure_whatever!(
            bits_allocated == 8 || bits_allocated == 16,
            "BitsAllocated other than 8 or 16 is not supported"
        );

        ensure_whatever!(
            bits_stored != 1,
            "BitsStored of 1 is not supported"
        );

        let bytes_per_sample = (bits_allocated / 8) as usize;
        let frame_size =
            cols as usize * rows as usize * samples_per_pixel as usize * bytes_per_sample;

        // identify frame data using the frame index
        let pixeldata_uncompressed = &src
            .raw_pixel_data()
            .context(encode_error::MissingAttributeSnafu { name: "Pixel Data" })?
            .fragments[0];

        let frame_data = pixeldata_uncompressed
            .get(frame_size * frame as usize..frame_size * (frame as usize + 1))
            .whatever_context("Frame index out of bounds")?;

        // Encode the data
        let mut encoder = CharLS::default();

        let frame_info = FrameInfo {
            width: cols as u32,
            height: rows as u32,
            bits_per_sample: bits_stored as i32,
            component_count: samples_per_pixel as i32,
        };

        // prefer lossless encoding by default
        let quality = options.quality.map(|q| q.clamp(0, 100)).unwrap_or(100);

        // calculate the maximum acceptable error range
        // based on the requested quality and bit depth
        let near = ((1 << (bits_stored - 4)) * (100 - quality as i32) / 100).min(4096);

        let compressed_data = encoder
            .encode(frame_info, near, frame_data)
            .whatever_context("JPEG-LS encoding failed")?;

        dst.extend_from_slice(&compressed_data);

        let mut changes = if near > 0 {
            let compressed_frame_size = compressed_data.len();

            let compression_ratio = frame_size as f64 / compressed_frame_size as f64;
            let compression_ratio = format!("{:.6}", compression_ratio);

            // provide attribute changes
            vec![
                // lossy image compression
                AttributeOp::new(Tag(0x0028, 0x2110), AttributeAction::SetStr("01".into())),
                // lossy image compression ratio
                AttributeOp::new(
                    Tag(0x0028, 0x2112),
                    AttributeAction::PushStr(compression_ratio.into()),
                ),
            ]
        } else {
            vec![
                // lossless image compression
                AttributeOp::new(Tag(0x0028, 0x2110), AttributeAction::SetIfMissing("00".into())),
            ]
        };

        let pmi = src.photometric_interpretation();

        if samples_per_pixel == 1 {
            // set Photometric Interpretation to Monochrome2
            // if it was neither of the expected monochromes
            if pmi != Some("MONOCHROME1") && pmi != Some("MONOCHROME2") {
                changes.push(AttributeOp::new(
                    Tag(0x0028, 0x0004),
                    AttributeAction::SetStr("MONOCHROME2".into()),
                ));
            }
        } else if samples_per_pixel == 3 {
            // set Photometric Interpretation to RGB
            // if it was not already set to RGB
            if pmi != Some("RGB") {
                changes.push(AttributeOp::new(
                    Tag(0x0028, 0x0004),
                    AttributeAction::SetStr("RGB".into()),
                ));
            }
        }

        Ok(changes)
    }
}

impl PixelDataWriter for JpegLsLosslessWriter {    
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        mut options: dicom_encoding::adapters::EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        // override quality and defer to the main adapter
        options.quality = Some(100);
        JpegLsAdapter.encode_frame(src, frame, options, dst)
    }
    
    fn encode(
        &self,
        src: &dyn PixelDataObject,
        options: dicom_encoding::adapters::EncodeOptions,
        dst: &mut Vec<Vec<u8>>,
        offset_table: &mut Vec<u32>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        // override quality and defer to the main adapter
        let mut options = options;
        options.quality = Some(100);
        JpegLsAdapter.encode(src, options, dst, offset_table)
    }
}
