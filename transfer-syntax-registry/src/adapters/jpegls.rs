//! Support for JPEG-LS image decoding.

use charls::{CharLS, FrameInfo};
use dicom_core::ops::{AttributeAction, AttributeOp};
use dicom_core::Tag;
use dicom_encoding::adapters::{
    decode_error, encode_error, DecodeResult, EncodeResult, PixelDataObject, PixelDataReader,
    PixelDataWriter,
};
use dicom_encoding::snafu::prelude::*;

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

        let frame_data = src
            .frame_pixel_data(frame)
            .whatever_context("Missing frame pixeldata")?;

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

        ensure_whatever!(bits_stored != 1, "BitsStored of 1 is not supported");

        let bytes_per_sample = (bits_allocated / 8) as usize;
        let frame_size =
            cols as usize * rows as usize * samples_per_pixel as usize * bytes_per_sample;

        // identify frame data using the frame index
        let frame_data = src
            .frame_pixel_data(frame)
            .whatever_context("Missing frame pixeldata")?;

        // Encode the data
        let mut encoder = CharLS::default();

        let frame_info = FrameInfo {
            width: cols as u32,
            height: rows as u32,
            bits_per_sample: bits_stored as i32,
            component_count: samples_per_pixel as i32,
        };

        // prefer lossless encoding by default
        let mut quality = options.quality.map(|q| q.clamp(0, 100)).unwrap_or(100);

        let pmi = src.photometric_interpretation();

        if pmi == Some("PALETTE COLOR") {
            // force lossless encoding of palette color samples
            quality = 100;
        }

        // calculate the maximum acceptable error range
        // based on the requested quality and bit depth
        let near = ((1 << (bits_stored - 4)) * (100 - quality as i32) / 100).min(4096);

        let compressed_data = encoder
            .encode(frame_info, near, frame_data.as_ref())
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
                AttributeOp::new(
                    Tag(0x0028, 0x2110),
                    AttributeAction::SetIfMissing("00".into()),
                ),
            ]
        };

        if samples_per_pixel == 1 {
            // set Photometric Interpretation to MONOCHROME2
            // if it was neither of the expected 1-channel formats
            if pmi != Some("MONOCHROME1")
                && pmi != Some("MONOCHROME2")
                && pmi != Some("PALETTE COLOR")
            {
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
