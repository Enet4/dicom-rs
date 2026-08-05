//! Support for DICOM overlay planes.
//!
//! Overlay planes are 1-bit deep bitmaps of graphics or regions of interest,
//! stored in the repeating groups `6000` to `601E`
//! (see [PS3.3 C.9.2][1] and [PS3.3 C.9.3][2] of the DICOM standard).
//! Since overlay data is recorded outside the pixel data,
//! it is always in native form and no pixel data codec is involved.
//!
//! Legacy overlay planes (retired in DICOM 2004)
//! which are embedded in unused bits of the pixel data samples
//! are also supported,
//! as long as the pixel data is not in an encapsulated form.
//!
//! [1]: https://dicom.nema.org/medical/dicom/current/output/chtml/part03/sect_C.9.2.html
//! [2]: https://dicom.nema.org/medical/dicom/current/output/chtml/part03/sect_C.9.3.html

use dicom_core::DataDictionary;
use dicom_object::{FileDicomObject, InMemDicomObject};

use crate::{
    FrameOutOfRangeSnafu, OverlayDataLengthSnafu, Result, UnsupportedOverlaySnafu, attribute,
};

/// The maximum number of overlay planes in a DICOM object,
/// as per the repeating groups `6000` to `601E`.
pub(crate) const MAX_OVERLAY_PLANES: u16 = 16;

/// A decoded representation of the DICOM _Overlay Type_ attribute (60xx,0040).
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
pub enum OverlayType {
    /// `G`: the overlay plane contains graphics
    Graphics,
    /// `R`: the overlay plane is a region of interest
    Roi,
}

/// A decoded DICOM overlay plane,
/// with its bitmap unpacked to one byte per overlay pixel
/// (either 0 or 1).
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayPlane {
    group: u16,
    rows: u16,
    columns: u16,
    overlay_type: OverlayType,
    origin: [i32; 2],
    frames: u32,
    image_frame_origin: u32,
    label: Option<String>,
    description: Option<String>,
    data: Vec<u8>,
}

impl OverlayPlane {
    /// Get the group number of this overlay plane
    /// (an even number between `0x6000` and `0x601E`)
    pub fn group(&self) -> u16 {
        self.group
    }

    /// Get the number of rows of the overlay bitmap
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Get the number of columns of the overlay bitmap
    pub fn columns(&self) -> u16 {
        self.columns
    }

    /// Get the overlay type (graphics or region of interest)
    pub fn overlay_type(&self) -> OverlayType {
        self.overlay_type
    }

    /// Get the overlay origin,
    /// the 1-based `[row, column]` of the image pixel
    /// under the top left pixel of the overlay bitmap.
    /// `[1, 1]` means that the overlay is aligned with the image,
    /// and values may be zero or negative.
    pub fn origin(&self) -> [i32; 2] {
        self.origin
    }

    /// Get the number of frames in the overlay (at least 1)
    pub fn number_of_frames(&self) -> u32 {
        self.frames
    }

    /// Get the 1-based number of the first image frame
    /// to which this overlay applies
    pub fn image_frame_origin(&self) -> u32 {
        self.image_frame_origin
    }

    /// Get the overlay label, if present
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Get the overlay description, if present
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Get the full unpacked overlay bitmap,
    /// one byte per overlay pixel (either 0 or 1),
    /// in row-major order,
    /// with all overlay frames in sequence
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the unpacked bitmap of a single overlay frame
    /// (0-based within the overlay),
    /// one byte per overlay pixel (either 0 or 1),
    /// in row-major order
    pub fn frame_data(&self, frame: u32) -> Result<&[u8]> {
        if frame >= self.frames {
            return FrameOutOfRangeSnafu {
                frame_number: frame,
            }
            .fail()
            .map_err(Into::into);
        }
        let frame_size = self.rows as usize * self.columns as usize;
        let offset = frame_size * frame as usize;
        Ok(&self.data[offset..offset + frame_size])
    }

    /// Get the 0-based overlay frame which applies to
    /// the given 0-based image frame,
    /// or `None` if the overlay does not apply to that image frame.
    pub fn frame_for_image_frame(&self, image_frame: u32) -> Option<u32> {
        let rel = image_frame as i64 - (self.image_frame_origin as i64 - 1);
        (rel >= 0 && rel < self.frames as i64).then_some(rel as u32)
    }

    /// Get whether the overlay bit at the given position is set,
    /// where `frame` is the 0-based overlay frame
    /// and `row` and `col` are 0-based coordinates in the overlay bitmap.
    ///
    /// Returns `false` if the position is out of bounds.
    pub fn is_set(&self, frame: u32, row: u32, col: u32) -> bool {
        if frame >= self.frames || row >= self.rows as u32 || col >= self.columns as u32 {
            return false;
        }
        let index = (frame as usize * self.rows as usize + row as usize) * self.columns as usize
            + col as usize;
        self.data[index] != 0
    }
}

/// Unpack a 1-bit packed overlay bitmap
/// into one byte (0 or 1) per pixel.
///
/// Overlay data is packed in row-major order,
/// least significant bit first within each byte,
/// with all frames in sequence without padding in between.
fn unpack_overlay_bits(packed: &[u8], num_pixels: usize) -> Vec<u8> {
    (0..num_pixels)
        .map(|i| (packed[i >> 3] >> (i & 0x7)) & 1)
        .collect()
}

/// Decode all overlay planes present in the object,
/// scanning the repeating groups `6000` to `601E`.
pub(crate) fn decode_overlays<D>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<Vec<OverlayPlane>>
where
    D: DataDictionary + Clone,
{
    (0..MAX_OVERLAY_PLANES)
        .filter_map(|i| decode_overlay_group(obj, 0x6000 + 2 * i).transpose())
        .collect()
}

/// Decode the overlay plane in the given repeating group
/// (an even number between `0x6000` and `0x601E`),
/// returning `Ok(None)` if the plane is not present.
pub(crate) fn decode_overlay_group<D>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
    group: u16,
) -> Result<Option<OverlayPlane>>
where
    D: DataDictionary + Clone,
{
    if !(0x6000..=0x601E).contains(&group) || group % 2 != 0 {
        return Ok(None);
    }

    let rows = match attribute::overlay_rows(obj, group) {
        Ok(rows) => rows,
        // the group holds no overlay plane if Overlay Data is also absent
        Err(e) => {
            return if attribute::overlay_data(obj, group)?.is_none() {
                Ok(None)
            } else {
                Err(e.into())
            };
        }
    };
    let columns = attribute::overlay_columns(obj, group)?;
    let overlay_type = match attribute::overlay_type(obj, group)?.as_str() {
        "G" => OverlayType::Graphics,
        "R" => OverlayType::Roi,
        other => {
            return UnsupportedOverlaySnafu {
                group,
                reason: format!("unknown Overlay Type `{other}`"),
            }
            .fail()
            .map_err(Into::into);
        }
    };
    let origin = attribute::overlay_origin(obj, group)?;
    let frames = attribute::number_of_frames_in_overlay(obj, group)?;
    let image_frame_origin = attribute::image_frame_origin(obj, group)?;
    let num_pixels = frames as usize * rows as usize * columns as usize;

    let data = match attribute::overlay_data(obj, group)? {
        Some(packed) => {
            // standard overlay plane recorded in Overlay Data (60xx,3000)
            let bits_allocated = attribute::overlay_bits_allocated(obj, group).unwrap_or(1);
            if bits_allocated != 1 {
                return UnsupportedOverlaySnafu {
                    group,
                    reason: format!("Overlay Bits Allocated is {bits_allocated}, expected 1"),
                }
                .fail()
                .map_err(Into::into);
            }
            let bit_position = attribute::overlay_bit_position(obj, group).unwrap_or(0);
            if bit_position != 0 {
                return UnsupportedOverlaySnafu {
                    group,
                    reason: format!("Overlay Bit Position is {bit_position}, expected 0"),
                }
                .fail()
                .map_err(Into::into);
            }

            let needed = num_pixels.div_ceil(8);
            if packed.len() < needed {
                return OverlayDataLengthSnafu {
                    group,
                    got: packed.len(),
                    needed,
                }
                .fail()
                .map_err(Into::into);
            }
            unpack_overlay_bits(&packed, num_pixels)
        }
        None => {
            // no Overlay Data: a legacy overlay plane (retired in DICOM 2004)
            // embedded in unused bits of the pixel data samples
            let frame_pixels = rows as usize * columns as usize;
            extract_embedded_overlay_bits(obj, group, num_pixels, frame_pixels, image_frame_origin)?
        }
    };

    Ok(Some(OverlayPlane {
        group,
        rows,
        columns,
        overlay_type,
        origin,
        frames,
        image_frame_origin,
        label: attribute::overlay_label(obj, group),
        description: attribute::overlay_description(obj, group),
        data,
    }))
}

/// Extract a legacy overlay plane which is embedded
/// in otherwise unused bits of the pixel data samples,
/// at the bit given by Overlay Bit Position (60xx,0102).
fn extract_embedded_overlay_bits<D>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
    group: u16,
    num_pixels: usize,
    frame_pixels: usize,
    image_frame_origin: u32,
) -> Result<Vec<u8>>
where
    D: DataDictionary + Clone,
{
    let bits_allocated = attribute::bits_allocated(obj)?;
    if bits_allocated != 8 && bits_allocated != 16 {
        return UnsupportedOverlaySnafu {
            group,
            reason: format!(
                "embedded overlay in pixel data with Bits Allocated {bits_allocated} is not supported"
            ),
        }
        .fail()
        .map_err(Into::into);
    }
    let samples_per_pixel = attribute::samples_per_pixel(obj).unwrap_or(1);
    if samples_per_pixel != 1 {
        return UnsupportedOverlaySnafu {
            group,
            reason: format!(
                "embedded overlay in pixel data with Samples per Pixel {samples_per_pixel} is not supported"
            ),
        }
        .fail()
        .map_err(Into::into);
    }
    let bit_position = attribute::overlay_bit_position(obj, group)?;
    if bit_position >= bits_allocated {
        return UnsupportedOverlaySnafu {
            group,
            reason: format!(
                "Overlay Bit Position {bit_position} is out of range of Bits Allocated {bits_allocated}"
            ),
        }
        .fail()
        .map_err(Into::into);
    }

    let pixel_data = attribute::pixel_data(obj)?;
    let samples = match pixel_data.value() {
        dicom_core::DicomValue::Primitive(p) => p.to_bytes(),
        _ => {
            return UnsupportedOverlaySnafu {
                group,
                reason: "embedded overlay in encapsulated pixel data is not supported".to_string(),
            }
            .fail()
            .map_err(Into::into);
        }
    };

    // the overlay frames are embedded in the image frames
    // starting at the image frame origin
    let bytes_per_sample = (bits_allocated / 8) as usize;
    let offset = (image_frame_origin as usize - 1) * frame_pixels * bytes_per_sample;
    let needed = offset + num_pixels * bytes_per_sample;
    if samples.len() < needed {
        return OverlayDataLengthSnafu {
            group,
            got: samples.len(),
            needed,
        }
        .fail()
        .map_err(Into::into);
    }

    // pixel data sample values are in little endian byte order
    Ok(samples[offset..needed]
        .chunks_exact(bytes_per_sample)
        .map(|sample| {
            let value = if bytes_per_sample == 2 {
                u16::from_le_bytes([sample[0], sample[1]])
            } else {
                sample[0] as u16
            };
            ((value >> bit_position) & 1) as u8
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelDecoder;
    use dicom_core::{DataElement, PrimitiveValue, Tag, VR, dicom_value};
    use dicom_dictionary_std::uids;
    use dicom_object::{DefaultDicomObject, FileDicomObject, FileMetaTableBuilder};

    fn dummy_dicom() -> DefaultDicomObject {
        FileDicomObject::new_empty_with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .media_storage_sop_class_uid(uids::SECONDARY_CAPTURE_IMAGE_STORAGE)
                .media_storage_sop_instance_uid("2.25.221545913343461877066277885826230616209")
                .build()
                .unwrap(),
        )
    }

    fn put_overlay_plane(obj: &mut DefaultDicomObject, group: u16, rows: u16, columns: u16) {
        obj.put_element(DataElement::new(
            Tag(group, 0x0010),
            VR::US,
            dicom_value!(U16, rows),
        ));
        obj.put_element(DataElement::new(
            Tag(group, 0x0011),
            VR::US,
            dicom_value!(U16, columns),
        ));
        obj.put_element(DataElement::new(
            Tag(group, 0x0040),
            VR::CS,
            PrimitiveValue::from("R"),
        ));
        obj.put_element(DataElement::new(
            Tag(group, 0x0050),
            VR::SS,
            dicom_value!(I16, [1, 1]),
        ));
        obj.put_element(DataElement::new(
            Tag(group, 0x0100),
            VR::US,
            dicom_value!(U16, 1),
        ));
        obj.put_element(DataElement::new(
            Tag(group, 0x0102),
            VR::US,
            dicom_value!(U16, 0),
        ));
        let num_bytes = (rows as usize * columns as usize).div_ceil(8).div_ceil(2) * 2;
        obj.put_element(DataElement::new(
            Tag(group, 0x3000),
            VR::OB,
            PrimitiveValue::from(vec![0b0000_0011u8; num_bytes]),
        ));
    }

    #[test]
    fn unpack_bits_is_lsb_first() {
        let unpacked = unpack_overlay_bits(&[0b0000_0011, 0b1000_0000], 16);
        let mut expected = vec![0u8; 16];
        expected[0] = 1;
        expected[1] = 1;
        expected[15] = 1;
        assert_eq!(unpacked, expected);
    }

    #[test]
    fn decode_synthesized_overlay_plane() {
        let mut obj = dummy_dicom();
        put_overlay_plane(&mut obj, 0x6000, 4, 8);
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x1500),
            VR::LO,
            PrimitiveValue::from("the label"),
        ));

        let overlays = obj.decode_overlays().unwrap();
        assert_eq!(overlays.len(), 1);
        let plane = &overlays[0];
        assert_eq!(plane.group(), 0x6000);
        assert_eq!(plane.rows(), 4);
        assert_eq!(plane.columns(), 8);
        assert_eq!(plane.overlay_type(), OverlayType::Roi);
        assert_eq!(plane.origin(), [1, 1]);
        assert_eq!(plane.number_of_frames(), 1);
        assert_eq!(plane.image_frame_origin(), 1);
        assert_eq!(plane.label(), Some("the label"));
        assert_eq!(plane.data().len(), 4 * 8);
        // each byte of `0b0000_0011` sets the first two bits of every 8 pixels,
        // so the first two pixels of each row are set (columns = 8)
        for row in 0..4 {
            for col in 0..8 {
                assert_eq!(plane.is_set(0, row, col), col < 2, "at ({row}, {col})");
            }
        }
        assert_eq!(plane.frame_data(0).unwrap(), plane.data());
        assert!(plane.frame_data(1).is_err());
    }

    #[test]
    fn decode_overlay_plane_by_index() {
        let mut obj = dummy_dicom();
        put_overlay_plane(&mut obj, 0x6002, 4, 4);

        assert!(obj.decode_overlay(0).unwrap().is_none());
        let plane = obj.decode_overlay(1).unwrap().unwrap();
        assert_eq!(plane.group(), 0x6002);
        // out-of-range index is not an error
        assert!(obj.decode_overlay(16).unwrap().is_none());

        let overlays = obj.decode_overlays().unwrap();
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].group(), 0x6002);
    }

    #[test]
    fn decode_multi_frame_overlay() {
        let mut obj = dummy_dicom();
        put_overlay_plane(&mut obj, 0x6000, 2, 8);
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x0015),
            VR::IS,
            PrimitiveValue::from("2"),
        ));
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x0051),
            VR::US,
            dicom_value!(U16, 2),
        ));
        // two frames of 2x8 pixels
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x3000),
            VR::OB,
            PrimitiveValue::from(vec![0b0000_0011u8; 4]),
        ));

        let plane = obj.decode_overlay(0).unwrap().unwrap();
        assert_eq!(plane.number_of_frames(), 2);
        assert_eq!(plane.image_frame_origin(), 2);
        assert_eq!(plane.data().len(), 2 * 2 * 8);
        assert_eq!(plane.frame_data(1).unwrap().len(), 2 * 8);

        // the overlay applies to image frames 1 and 2 (0-based)
        assert_eq!(plane.frame_for_image_frame(0), None);
        assert_eq!(plane.frame_for_image_frame(1), Some(0));
        assert_eq!(plane.frame_for_image_frame(2), Some(1));
        assert_eq!(plane.frame_for_image_frame(3), None);
    }

    #[test]
    fn overlay_data_stored_as_words_unpacks_little_endian() {
        let mut obj = dummy_dicom();
        put_overlay_plane(&mut obj, 0x6000, 2, 8);
        // one word covers 16 pixels: bits 0-7 in the low byte come first
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x3000),
            VR::OW,
            dicom_value!(U16, [0x8001]),
        ));

        let plane = obj.decode_overlay(0).unwrap().unwrap();
        assert!(plane.is_set(0, 0, 0));
        assert!(!plane.is_set(0, 0, 1));
        assert!(plane.is_set(0, 1, 7));
    }

    #[test]
    fn decode_embedded_overlay_plane() {
        let mut obj = dummy_dicom();
        // 2x4 image, 16 bits allocated, 12 bits stored
        obj.put_element(DataElement::new(
            dicom_dictionary_std::tags::ROWS,
            VR::US,
            dicom_value!(U16, 2),
        ));
        obj.put_element(DataElement::new(
            dicom_dictionary_std::tags::COLUMNS,
            VR::US,
            dicom_value!(U16, 4),
        ));
        obj.put_element(DataElement::new(
            dicom_dictionary_std::tags::BITS_ALLOCATED,
            VR::US,
            dicom_value!(U16, 16),
        ));
        obj.put_element(DataElement::new(
            dicom_dictionary_std::tags::SAMPLES_PER_PIXEL,
            VR::US,
            dicom_value!(U16, 1),
        ));
        // overlay embedded at bit 15: set on samples 0 and 5 only
        let mut samples = vec![0x0123u16; 8];
        samples[0] |= 0x8000;
        samples[5] |= 0x8000;
        obj.put_element(DataElement::new(
            dicom_dictionary_std::tags::PIXEL_DATA,
            VR::OW,
            PrimitiveValue::U16(samples.into()),
        ));

        // overlay plane in group 6000 without Overlay Data
        put_overlay_plane(&mut obj, 0x6000, 2, 4);
        obj.remove_element(Tag(0x6000, 0x3000));
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x0100),
            VR::US,
            dicom_value!(U16, 16),
        ));
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x0102),
            VR::US,
            dicom_value!(U16, 15),
        ));

        let plane = obj.decode_overlay(0).unwrap().unwrap();
        assert_eq!(plane.rows(), 2);
        assert_eq!(plane.columns(), 4);
        let mut expected = vec![0u8; 8];
        expected[0] = 1;
        expected[5] = 1;
        assert_eq!(plane.data(), &expected[..]);

        // out-of-range bit position is an error
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x0102),
            VR::US,
            dicom_value!(U16, 16),
        ));
        assert!(obj.decode_overlay(0).is_err());
    }

    #[test]
    fn decode_overlay_error_cases() {
        // overlay data too short
        let mut obj = dummy_dicom();
        put_overlay_plane(&mut obj, 0x6000, 64, 64);
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x3000),
            VR::OB,
            PrimitiveValue::from(vec![0u8; 2]),
        ));
        assert!(obj.decode_overlay(0).is_err());

        // unsupported bits allocated
        let mut obj = dummy_dicom();
        put_overlay_plane(&mut obj, 0x6000, 4, 8);
        obj.put_element(DataElement::new(
            Tag(0x6000, 0x0100),
            VR::US,
            dicom_value!(U16, 16),
        ));
        assert!(obj.decode_overlay(0).is_err());

        // no overlay data element at all: not an error, just absent
        let obj = dummy_dicom();
        assert!(obj.decode_overlay(0).unwrap().is_none());
        assert!(obj.decode_overlays().unwrap().is_empty());
    }
}
