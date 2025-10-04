//! Support for JPEG XL image decoding and encoding.

use dicom_core::ops::{AttributeAction, AttributeOp};
use dicom_core::Tag;
use dicom_encoding::adapters::{
    decode_error, encode_error, DecodeResult, PixelDataObject, PixelDataReader, PixelDataWriter,
};
use dicom_encoding::snafu::prelude::*;
use jxl_oxide::JxlImage;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;

/// Base pixel data adapter (decoder and encoder) for transfer syntaxes based on JPEG XL.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct JpegXlAdapter;

/// Pixel data encoder specifically for JPEG XL lossless compression.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct JpegXlLosslessEncoder;

impl PixelDataReader for JpegXlAdapter {
    /// Decode a single frame in JPEG XL from a DICOM object.
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

        let raw = src
            .raw_pixel_data()
            .whatever_context("Expected to have raw pixel data available")?;

        ensure_whatever!(
            raw.fragments.len() == nr_frames,
            "Unexpected number of fragments"
        );
        let frame_data =
            // assuming 1:1 frame-to-fragment mapping
            raw.fragments
                .get(frame as usize)
                .with_whatever_context(|| {
                    format!("Missing fragment #{frame} for the frame requested")
                })?;

        let image = JxlImage::builder()
            .read(&**frame_data)
            .whatever_context("failed to read JPEG XL data")?;
        let frame = image
            .render_frame(0)
            .whatever_context("failed to render JPEG XL frame")?;

        let mut stream = frame.stream();

        // write and convert samples to the destination buffer depending on bit depth
        match bits_allocated {
            1 => {
                whatever!("Unsupported bit depth 1 by JPEG XL decoder");
            }
            8 => {
                // write directly to dst as u8 samples

                let samples_per_frame =
                    stream.channels() as usize * stream.width() as usize * stream.height() as usize;

                dst.try_reserve(samples_per_frame)
                    .whatever_context("Failed to reserve heap space for JPEG XL frame")?;

                let offset = dst.len();
                dst.resize(offset + samples_per_frame, 0);

                let count = stream.write_to_buffer(&mut dst[offset..]);
                dst.truncate(offset + count);
            }
            16 => {
                // write all u16 samples to a buffer

                let mut buffer = vec![
                    0_u16;
                    stream.channels() as usize
                        * stream.width() as usize
                        * stream.height() as usize
                ];

                let count = stream.write_to_buffer(&mut buffer);
                buffer.truncate(count);

                // pass them as bytes in native endian

                for &sample in &buffer {
                    dst.extend_from_slice(&sample.to_ne_bytes());
                }
            }
            24 => {
                // write all f32 samples to a buffer

                let mut buffer = vec![
                    0_f32;
                    stream.channels() as usize
                        * stream.width() as usize
                        * stream.height() as usize
                ];

                let count = stream.write_to_buffer(&mut buffer);
                buffer.truncate(count);

                // then convert them to 24-bit integers
                for &sample in &buffer {
                    let bytes = &((sample * 16777215.) as u32).to_ne_bytes();
                    dst.extend_from_slice(&bytes[..3]);
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }
}

impl PixelDataWriter for JpegXlAdapter {
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        options: dicom_encoding::adapters::EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> dicom_encoding::adapters::EncodeResult<Vec<AttributeOp>> {
        use zune_jpegxl::JxlSimpleEncoder;

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

        ensure_whatever!(
            bits_allocated == 8 || bits_allocated == 16,
            "BitsAllocated other than 8 or 16 is not supported"
        );

        let dicom_encoding::adapters::EncodeOptions {
            quality, effort, ..
        } = options;

        let bytes_per_sample = (bits_allocated / 8) as usize;
        let frame_size =
            cols as usize * rows as usize * samples_per_pixel as usize * bytes_per_sample;

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

        let bit_depth = match bits_allocated {
            8 => BitDepth::Eight,
            16 => BitDepth::Sixteen,
            _ => unreachable!(),
        };
        let color_space = match samples_per_pixel {
            1 => ColorSpace::Luma,
            3 => ColorSpace::RGB,
            _ => ColorSpace::Unknown,
        };
        let options = EncoderOptions::new(cols as usize, rows as usize, color_space, bit_depth);
        let mut quality = quality.unwrap_or(85);

        let pmi = src.photometric_interpretation();

        if pmi == Some("PALETTE COLOR") {
            // force lossless compression for palette color
            quality = 100;
        }

        options.set_quality(quality);
        options.set_effort(effort.map(|e| e + 27).unwrap_or(64));
        let encoder = JxlSimpleEncoder::new(frame_data, options);

        let jxl = encoder
            .encode()
            .map_err(|e| format!("{e:?}"))
            .whatever_context("Failed to encode JPEG XL data")?;

        dst.extend_from_slice(&jxl);

        // provide attribute changes
        let mut changes = if quality != 100 {
            let compressed_frame_size = dst.len() - len_before;
            let compression_ratio = frame_size as f64 / compressed_frame_size as f64;
            let compression_ratio = format!("{compression_ratio:.6}");
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
            vec![]
        };

        if samples_per_pixel == 1 {
            // set Photometric Interpretation to Monochrome2
            // if it was neither of the expected monochromes
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

impl PixelDataWriter for JpegXlLosslessEncoder {
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        options: dicom_encoding::adapters::EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> dicom_encoding::adapters::EncodeResult<Vec<AttributeOp>> {
        // override quality option and defer to JpegXlAdapter
        let mut options = options;
        options.quality = Some(100);
        JpegXlAdapter.encode_frame(src, frame, options, dst)
    }

    fn encode(
        &self,
        src: &dyn PixelDataObject,
        options: dicom_encoding::adapters::EncodeOptions,
        dst: &mut Vec<Vec<u8>>,
        offset_table: &mut Vec<u32>,
    ) -> dicom_encoding::adapters::EncodeResult<Vec<AttributeOp>> {
        // override quality option and defer to JpegXlAdapter
        let mut options = options;
        options.quality = Some(100);
        JpegXlAdapter.encode(src, options, dst, offset_table)
    }
}
