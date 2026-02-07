//! Support for deflated image frame compression via pixel data adapter.

use dicom_core::{
    ops::{AttributeAction, AttributeOp},
    PrimitiveValue, Tag,
};
use dicom_encoding::{
    adapters::{
        DecodeResult, EncodeOptions, EncodeResult, PixelDataObject, PixelDataReader, PixelDataWriter, decode_error, encode_error
    },
    snafu::{OptionExt, ResultExt},
};

use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};

/// Adapter for [Deflated Image Frame Compression][1]
/// [1]: https://dicom.nema.org/medical/dicom/2025c/output/chtml/part05/sect_10.20.html
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeflatedImageFrameAdapter;

impl PixelDataReader for DeflatedImageFrameAdapter {
    fn decode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> DecodeResult<()> {
        // decod each fragment into the output vector
        let pixeldata = src
            .raw_pixel_data()
            .context(decode_error::MissingAttributeSnafu { name: "Pixel Data" })?;

        if pixeldata.fragments.is_empty() {
            return Ok(());
        }

        // set up decoder with the first frame
        let mut it = pixeldata.fragments.iter();
        let mut decoder = DeflateDecoder::new(
            &it.next().expect("fragments should not be empty")[..]
        );
        std::io::copy(&mut decoder, dst)
            .whatever_context("failed to deflate frame")?;
        for fragment in it {
            decoder.reset(&fragment[..]);
            std::io::copy(&mut decoder, dst)
                .whatever_context("failed to deflate frame")?;
        }

        Ok(())
    }

    fn decode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        dst: &mut Vec<u8>,
    ) -> DecodeResult<()> {

        // just copy the specific fragment into the output vector
        let pixeldata = src
            .raw_pixel_data()
            .context(decode_error::MissingAttributeSnafu { name: "Pixel Data" })?;

        let fragment = pixeldata
            .fragments
            .get(frame as usize)
            .context(decode_error::FrameRangeOutOfBoundsSnafu)?;

        let mut decoder = DeflateDecoder::new(&fragment[..]);
        std::io::copy(&mut decoder, dst)
            .whatever_context("failed to deflate frame")?;

        Ok(())
    }
}

impl PixelDataWriter for DeflatedImageFrameAdapter {
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        options: EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<AttributeOp>> {
        use std::io::Write;

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

        let len_before = dst.len();

        // Deflate the data to the output
        let compression = match options.effort {
            None => Compression::default(),
            Some(0) | Some(1) => Compression::fast(),
            // map 0..=100 to 0..=9
            Some(e) => Compression::new((e.min(100) / 11) as u32),
        };
        let mut encoder = DeflateEncoder::new(&mut *dst, compression);
        encoder.write_all(frame_data)
            .whatever_context("failed to encode deflated data")?;
        encoder.finish()
            .whatever_context("failed to finish deflated data encoding")?;

        if dst.len() % 2 == 1 {
            // add null byte to maintain even length
            dst.push(0);
        }

        let fragment_len = dst.len() - len_before;

        // provide attribute changes
        Ok(vec![
            // Encapsulated Pixel Data Value Total Length
            AttributeOp::new(
                Tag(0x7FE0, 0x0003),
                AttributeAction::Set(PrimitiveValue::from(fragment_len as u64)),
            ),
        ])
    }
}
