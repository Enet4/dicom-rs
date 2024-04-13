//! Support for RLE Lossless image decoding.
//!
//! implementation taken from Pydicom:
//! <https://github.com/pydicom/pydicom/blob/master/pydicom/pixel_data_handlers/rle_handler.py>
//!
//! Copyright 2008-2021 pydicom authors.
//!
//! License: <https://github.com/pydicom/pydicom/blob/master/LICENSE>
use byteordered::byteorder::{ByteOrder, LittleEndian};

use dicom_encoding::adapters::{decode_error, DecodeResult, PixelDataObject, PixelDataReader};
use dicom_encoding::snafu::prelude::*;
use std::io::{self, Read, Seek};

/// Pixel data adapter for the RLE Lossless transfer syntax.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RleLosslessAdapter;

/// Pixel data decoder for RLE Lossless (UID `1.2.840.10008.1.2.5`)
impl PixelDataReader for RleLosslessAdapter {
    /// Decode the DICOM image from RLE Lossless completely.
    ///
    /// See <https://dicom.nema.org/medical/dicom/2023e/output/chtml/part05/chapter_G.html>
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
        // For RLE the number of fragments = number of frames
        // therefore, we can fetch the fragments one by one
        let nr_frames =
            src.number_of_fragments()
                .whatever_context("Invalid pixel data, no fragments found")? as usize;
        let bytes_per_sample = (bits_allocated / 8) as usize;
        let samples_per_pixel = samples_per_pixel as usize;
        // `stride` is the total number of bytes for each sample plane
        let stride = bytes_per_sample * cols as usize * rows as usize;
        let frame_size = stride * samples_per_pixel;
        // extend `dst` to make room for decoded pixel data
        let base_offset = dst.len();
        dst.resize(base_offset + frame_size * nr_frames, 0);

        // RLE encoded data is ordered like this (for 16-bit, 3 sample):
        //  Segment: 0     | 1     | 2     | 3     | 4     | 5
        //           R MSB | R LSB | G MSB | G LSB | B MSB | B LSB
        //  A segment contains only the MSB or LSB parts of all the sample pixels

        // As currently required,
        // we need to rearrange the pixel data to standard planar configuration.
        // (and use little endian byte ordering):
        //    Pixel 1                             | ... Pixel N
        //    Red         Green       Blue        | ...
        //    LSB R MSB R LSB G MSB G LSB B MSB B | ...

        for i in 0..nr_frames {
            let fragment = &src
                .fragment(i)
                .whatever_context("No pixel data found for frame")?;
            let mut offsets = read_rle_header(fragment);
            offsets.push(fragment.len() as u32);

            for sample_number in 0..samples_per_pixel {
                for byte_offset in (0..bytes_per_sample).rev() {
                    // ii is 1, 0, 3, 2, 5, 4 for the example above
                    // This is where the segment order correction occurs
                    let ii = sample_number * bytes_per_sample + byte_offset;
                    let segment = &fragment[offsets[ii] as usize..offsets[ii + 1] as usize];
                    let buff = io::Cursor::new(segment);
                    let (_, decoder) = PackBitsReader::new(buff, segment.len())
                        .whatever_context("Failed to read RLE segments")?;
                    let mut decoded_segment = Vec::with_capacity(rows as usize * cols as usize);
                    decoder
                        .take(rows as u64 * cols as u64)
                        .read_to_end(&mut decoded_segment)
                        .unwrap();

                    // Interleave pixels as described in the example above.
                    // in 16-bit, this is:
                    // MSB R channel: 1,  7, 13, ...
                    // LSB R channel: 0,  6, 12, ...
                    // MSB G channel: 3,  9, 15, ...
                    // LSB G channel: 2,  8, 14, ...
                    // MSB G channel: 5, 11, 17, ...
                    // LSB G channel: 4, 10, 16, ...
                    let frame_start = i * frame_size;
                    let start = frame_start +  if samples_per_pixel == 3 {
                        sample_number * bytes_per_sample + byte_offset
                    } else {
                        sample_number * bytes_per_sample + samples_per_pixel - byte_offset
                    };

                    let end = (i + 1) * frame_size;
                    for (decoded_index, dst_index) in (start..end)
                        .step_by(bytes_per_sample * samples_per_pixel)
                        .enumerate()
                    {
                        dst[base_offset + dst_index] = decoded_segment[decoded_index];
                    }
                }
            }
        }
        Ok(())
    }

    /// Decode a singe frame of the DICOM image from RLE Lossless.
    ///
    /// See <https://dicom.nema.org/medical/dicom/2023e/output/chtml/part05/chapter_G.html>
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

        if bits_allocated != 8 && bits_allocated != 16 {
            whatever!("BitsAllocated other than 8 or 16 is not supported");
        }
        // For RLE the number of fragments = number of frames
        // therefore, we can fetch the fragments one by one
        let nr_frames =
            src.number_of_fragments()
                .whatever_context("Invalid pixel data, no fragments found")? as usize;
        ensure!(
            nr_frames > frame as usize,
            decode_error::FrameRangeOutOfBoundsSnafu
        );

        let bytes_per_sample = (bits_allocated / 8) as usize;
        let samples_per_pixel = samples_per_pixel as usize;
        // `stride` is the total number of bytes for each sample plane
        let stride = bytes_per_sample * cols as usize * rows as usize;
        let frame_size = stride * samples_per_pixel;
        // extend `dst` to make room for decoded pixel data
        let base_offset = dst.len();
        dst.resize(base_offset + frame_size, 0);

        // RLE encoded data is ordered like this (for 16-bit, 3 sample):
        //  Segment: 0     | 1     | 2     | 3     | 4     | 5
        //           R MSB | R LSB | G MSB | G LSB | B MSB | B LSB
        //  A segment contains only the MSB or LSB parts of all the sample pixels

        // As currently required,
        // we need to rearrange the pixel data to standard planar configuration.
        // (and use little endian byte ordering):
        //    Pixel 1                             | ... Pixel N
        //    Red         Green       Blue        | ...
        //    LSB R MSB R LSB G MSB G LSB B MSB B | ...

        let fragment = &src
            .fragment(frame as usize)
            .whatever_context("No pixel data found for frame")?;
        let mut offsets = read_rle_header(fragment);
        offsets.push(fragment.len() as u32);

        for sample_number in 0..samples_per_pixel {
            for byte_offset in (0..bytes_per_sample).rev() {
                // ii is 1, 0, 3, 2, 5, 4 for the example above
                // This is where the segment order correction occurs
                let ii = sample_number * bytes_per_sample + byte_offset;
                let segment = &fragment[offsets[ii] as usize..offsets[ii + 1] as usize];
                let buff = io::Cursor::new(segment);
                let (_, decoder) = PackBitsReader::new(buff, segment.len())
                    .map_err(|e| Box::new(e) as Box<_>)
                    .whatever_context("Failed to read RLE segments")?;
                let mut decoded_segment = Vec::with_capacity(rows as usize * cols as usize);
                decoder
                    .take(rows as u64 * cols as u64)
                    .read_to_end(&mut decoded_segment)
                    .unwrap();

                // Interleave pixels as described in the example above.
                let start = if samples_per_pixel == 3 {
                    sample_number * bytes_per_sample + byte_offset
                } else {
                    sample_number * bytes_per_sample + samples_per_pixel - byte_offset
                };

                let end = frame_size;
                for (decoded_index, dst_index) in (start..end)
                    .step_by(bytes_per_sample * samples_per_pixel)
                    .enumerate()
                {
                    dst[base_offset + dst_index] = decoded_segment[decoded_index];
                }
            }
        }
        Ok(())
    }
}

// TODO(#125) implement `encode`

// Read the RLE header and return the offsets
fn read_rle_header(fragment: &[u8]) -> Vec<u32> {
    let nr_segments = LittleEndian::read_u32(&fragment[0..4]);
    let mut offsets = vec![0; nr_segments as usize];
    LittleEndian::read_u32_into(&fragment[4..4 * (nr_segments + 1) as usize], &mut offsets);
    offsets
}

/// PackBits Reader from the image-tiff crate
/// Copyright 2018-2021 PistonDevelopers.
/// License: <https://github.com/image-rs/image-tiff/blob/master/LICENSE>
/// From: https://github.com/image-rs/image-tiff/blob/master/src/decoder/stream.rs
#[derive(Debug)]
struct PackBitsReader {
    buffer: io::Cursor<Vec<u8>>,
}

impl PackBitsReader {
    /// Wraps a reader
    pub fn new<R: Read + Seek>(
        mut reader: R,
        length: usize,
    ) -> io::Result<(usize, PackBitsReader)> {
        let mut buffer = Vec::new();
        let mut header: [u8; 1] = [0];
        let mut data: [u8; 1] = [0];

        let mut bytes_read = 0;
        while bytes_read < length {
            reader.read_exact(&mut header)?;
            bytes_read += 1;

            let h = header[0] as i8;
            if (-127..=-1).contains(&h) {
                let new_len = buffer.len() + (1 - h as isize) as usize;
                reader.read_exact(&mut data)?;
                buffer.resize(new_len, data[0]);
                bytes_read += 1;
            } else if h >= 0 {
                let num_vals = h as usize + 1;
                io::copy(&mut reader.by_ref().take(num_vals as u64), &mut buffer)?;
                bytes_read += num_vals;
            } else {
                // h = -128 is a no-op.
            }
        }

        Ok((
            buffer.len(),
            PackBitsReader {
                buffer: io::Cursor::new(buffer),
            },
        ))
    }
}

impl Read for PackBitsReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.buffer.read(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_packbits() {
        let encoded = vec![
            0xFE, 0xAA, 0x02, 0x80, 0x00, 0x2A, 0xFD, 0xAA, 0x03, 0x80, 0x00, 0x2A, 0x22, 0xF7,
            0xAA,
        ];
        let encoded_len = encoded.len();

        let buff = io::Cursor::new(encoded);
        let (_, mut decoder) = PackBitsReader::new(buff, encoded_len).unwrap();

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        let expected = vec![
            0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0xAA, 0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0x22,
            0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        ];
        assert_eq!(decoded, expected);
    }
}
