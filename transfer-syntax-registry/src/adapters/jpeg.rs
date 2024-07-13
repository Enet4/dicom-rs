//! Support for JPG image decoding.

use dicom_core::ops::{AttributeAction, AttributeOp};
use dicom_core::{PrimitiveValue, Tag};
use dicom_encoding::adapters::{
    decode_error, encode_error, DecodeResult, EncodeOptions, EncodeResult, PixelDataObject,
    PixelDataReader, PixelDataWriter,
};
use dicom_encoding::snafu::prelude::*;
use jpeg_decoder::Decoder;
use jpeg_encoder::ColorType;
use std::borrow::Cow;
use std::io::Cursor;

/// Pixel data adapter for JPEG-based transfer syntaxes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JpegAdapter;

impl PixelDataReader for JpegAdapter {
    /// Decode DICOM image data with jpeg encoding.
    fn decode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> DecodeResult<()> {
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

        if bits_allocated != 8 && bits_allocated != 16 {
            whatever!("BitsAllocated other than 8 or 16 is not supported");
        }

        let nr_frames = src.number_of_frames().unwrap_or(1) as usize;
        let bytes_per_sample = bits_allocated / 8;

        // `stride` it the total number of bytes for each sample plane
        let stride: usize = bytes_per_sample as usize * cols as usize * rows as usize;
        let base_offset = dst.len();
        dst.resize(
            base_offset + (samples_per_pixel as usize * stride) * nr_frames,
            0,
        );

        let raw = src
            .raw_pixel_data()
            .whatever_context("Expected to have raw pixel data available")?;

        // Some embedded JPEGs might span multiple fragments.
        // Hence we collect all fragments into single vector
        // and then iterate a cursor for each frame
        // Note: not the most efficient way to do this,
        // consider optimizing later with bytes data structures
        let fragments: Vec<u8> = raw.fragments.into_iter().flatten().collect();

        let fragments_len = fragments.len() as u64;
        let mut cursor = Cursor::new(fragments);
        let mut dst_offset = base_offset;

        for i in 0..nr_frames {
            let mut decoder = Decoder::new(&mut cursor);
            let decoded = decoder
                .decode()
                .map_err(|e| Box::new(e) as Box<_>)
                .with_whatever_context(|_| format!("JPEG decoding failure on frame {}", i))?;

            let decoded_len = decoded.len();
            dst[dst_offset..(dst_offset + decoded_len)].copy_from_slice(&decoded);
            dst_offset += decoded_len;

            if next_even(cursor.position()) >= next_even(fragments_len) {
                break;
            }

            // stop if there aren't enough bytes to continue
            if cursor.position() + 2 >= fragments_len {
                break;
            }
            
            // DICOM fragments should always have an even length,
            // filling this spacing with padding if it is odd.
            // Some implementations might add this padding,
            // whereas other might not.
            // So we look for the start of the SOI marker
            // to identify whether the padding is there
            if cursor.position() % 2 > 0 {
                let Some(next_byte_1) = cursor
                    .get_ref()
                    .get(cursor.position() as usize + 1)
                    .copied()
                else {
                    // no more frames to read
                    break;
                };
                let Some(next_byte_2) = cursor
                    .get_ref()
                    .get(cursor.position() as usize + 2)
                    .copied()
                else {
                    // no more frames to read
                    break;
                };

                if [next_byte_1, next_byte_2] == [0xFF, 0xD8] {
                    // skip padding and continue
                    cursor.set_position(cursor.position() + 1);
                }
            }
        }

        Ok(())
    }

    /// Decode DICOM image data with jpeg encoding.
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
        let base_offset = dst.len();
        dst.resize(base_offset + (samples_per_pixel as usize * stride), 0);

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

        let fragme_data_len = frame_data.len() as u64;
        let mut cursor = Cursor::new(&*frame_data);
        let mut dst_offset = base_offset;

        loop {
            let mut decoder = Decoder::new(&mut cursor);
            let decoded = decoder
                .decode()
                .map_err(|e| Box::new(e) as Box<_>)
                .whatever_context("JPEG decoder failure")?;

            let decoded_len = decoded.len();
            dst[dst_offset..(dst_offset + decoded_len)].copy_from_slice(&decoded);
            dst_offset += decoded_len;

            if next_even(cursor.position()) >= next_even(fragme_data_len) {
                break;
            }

            // stop if there aren't enough bytes to continue
            if cursor.position() + 2 >= fragme_data_len {
                break;
            }

            // DICOM fragments should always have an even length,
            // filling this spacing with padding if it is odd.
            // Some implementations might add this padding,
            // whereas other might not.
            // So we look for the start of the SOI marker
            // to identify whether the padding is there
            if cursor.position() % 2 > 0 {
                let Some(next_byte_1) = cursor
                    .get_ref()
                    .get(cursor.position() as usize + 1)
                    .copied()
                else {
                    // no more frames to read
                    break;
                };
                let Some(next_byte_2) = cursor
                    .get_ref()
                    .get(cursor.position() as usize + 2)
                    .copied()
                else {
                    // no more frames to read
                    break;
                };

                if [next_byte_1, next_byte_2] == [0xFF, 0xD8] {
                    // skip padding and continue
                    cursor.set_position(cursor.position() + 1);
                }
            }
        }

        Ok(())
    }
}

impl PixelDataWriter for JpegAdapter {
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        options: EncodeOptions,
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

        let quality = options.quality.unwrap_or(85);

        let bytes_per_sample = (bits_allocated / 8) as usize;
        let frame_size =
            cols as usize * rows as usize * samples_per_pixel as usize * bytes_per_sample;

        let color_type = match samples_per_pixel {
            1 => ColorType::Luma,
            3 => ColorType::Rgb,
            _ => whatever!("Unsupported samples per pixel: {}", samples_per_pixel),
        };

        // record dst length before encoding to know full jpeg size
        let len_before = dst.len();

        // identify frame data using the frame index
        let pixeldata_uncompressed = &src
            .raw_pixel_data()
            .context(encode_error::MissingAttributeSnafu { name: "Pixel Data" })?
            .fragments[0];

        let frame_data = pixeldata_uncompressed
            .get(frame_size * frame as usize..frame_size * (frame as usize + 1))
            .whatever_context("Frame index out of bounds")?;

        let frame_data = narrow_8bit(frame_data, bits_stored)?;

        // Encode the data
        let mut encoder = jpeg_encoder::Encoder::new(&mut *dst, quality);
        encoder.set_progressive(false);
        encoder
            .encode(&frame_data, cols, rows, color_type)
            .whatever_context("JPEG encoding failed")?;

        let compressed_frame_size = dst.len() - len_before;

        let compression_ratio = frame_size as f64 / compressed_frame_size as f64;
        let compression_ratio = format!("{:.6}", compression_ratio);

        // provide attribute changes
        let mut changes = vec![
            // bits allocated
            AttributeOp::new(
                Tag(0x0028, 0x0100),
                AttributeAction::Set(PrimitiveValue::from(8_u16)),
            ),
            // bits stored
            AttributeOp::new(
                Tag(0x0028, 0x0101),
                AttributeAction::Set(PrimitiveValue::from(8_u16)),
            ),
            // high bit
            AttributeOp::new(
                Tag(0x0028, 0x0102),
                AttributeAction::Set(PrimitiveValue::from(7_u16)),
            ),
            // lossy image compression
            AttributeOp::new(Tag(0x0028, 0x2110), AttributeAction::SetStr("01".into())),
            // lossy image compression ratio
            AttributeOp::new(
                Tag(0x0028, 0x2112),
                AttributeAction::PushStr(compression_ratio.into()),
            ),
        ];

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

fn next_even(l: u64) -> u64 {
    (l + 1) & !1
}

/// reduce data precision to 8 bits if necessary
/// data loss is possible
fn narrow_8bit(frame_data: &[u8], bits_stored: u16) -> EncodeResult<Cow<[u8]>> {
    debug_assert!(bits_stored >= 8);
    match bits_stored {
        8 => Ok(Cow::Borrowed(frame_data)),
        9..=16 => {
            let mut v = Vec::with_capacity(frame_data.len() / 2);
            for chunk in frame_data.chunks(2) {
                let b = u16::from(chunk[0]) | u16::from(chunk[1]) << 8;
                v.push((b >> (bits_stored - 8)) as u8);
            }
            Ok(Cow::Owned(v))
        }
        b => whatever!("Unsupported Bits Stored {}", b),
    }
}
