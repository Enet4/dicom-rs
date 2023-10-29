//! Support for encapsulated uncompressed via pixel data adapter.

use dicom_core::{
    ops::{AttributeAction, AttributeOp},
    PrimitiveValue, Tag,
};
use dicom_encoding::{
    adapters::{
        decode_error, encode_error, DecodeResult, EncodeOptions, EncodeResult, PixelDataObject,
        PixelDataReader, PixelDataWriter,
    },
    snafu::OptionExt,
};

/// Adapter for [Encapsulated Uncompressed Explicit VR Little Endian][1]
/// [1]: https://dicom.nema.org/medical/dicom/2023c/output/chtml/part05/sect_A.4.11.html
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UncompressedAdapter;

impl PixelDataReader for UncompressedAdapter {
    fn decode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> DecodeResult<()> {
        // just flatten all fragments into the output vector
        let pixeldata = src
            .raw_pixel_data()
            .context(decode_error::MissingAttributeSnafu { name: "Pixel Data" })?;

        for fragment in pixeldata.fragments {
            dst.extend_from_slice(&fragment);
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

        dst.extend_from_slice(fragment);

        Ok(())
    }
}

impl PixelDataWriter for UncompressedAdapter {
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        _options: EncodeOptions,
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

        let bytes_per_sample = (bits_allocated / 8) as usize;
        let frame_size =
            cols as usize * rows as usize * samples_per_pixel as usize * bytes_per_sample;

        // identify frame data using the frame index
        let pixeldata_uncompressed = &src
            .raw_pixel_data()
            .context(encode_error::MissingAttributeSnafu { name: "Pixel Data" })?
            .fragments[0];

        let len_before = pixeldata_uncompressed.len();

        let frame_data = pixeldata_uncompressed
            .get(frame_size * frame as usize..frame_size * (frame as usize + 1))
            .whatever_context("Frame index out of bounds")?;

        // Copy the the data to the output
        dst.extend_from_slice(frame_data);

        // provide attribute changes
        Ok(vec![
            // Encapsulated Pixel Data Value Total Length
            AttributeOp::new(
                Tag(0x7FE0, 0x0003),
                AttributeAction::Set(PrimitiveValue::from(len_before as u64)),
            ),
        ])
    }
}
