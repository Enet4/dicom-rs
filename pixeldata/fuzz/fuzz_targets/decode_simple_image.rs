#![no_main]
use byteorder::{LittleEndian as LE, ReadBytesExt};
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::{FileDicomObject, FileMetaTableBuilder, InMemDicomObject};
use dicom_pixeldata::PixelDecoder;
use libfuzzer_sys::fuzz_target;
use std::{error::Error, io::Read};

fuzz_target!(|data: &[u8]| {
    let _ = fuzz(data);
});

/// Obtain a simple DICOM file with an image from reading some raw bytes
/// in a non-standard format.
/// This simplifies the fuzz test payload to
/// obtain coverage over pixel data decoding sooner.
///
/// For the time being,
/// it always returns native pixel data objects in Explicit VR Little Endian.
fn build_simple_image(data: &[u8]) -> Result<FileDicomObject<InMemDicomObject>, Box<dyn Error>> {
    let mut obj = InMemDicomObject::new_empty();

    let reader = &mut (&data[..]);

    let rows = reader.read_u16::<LE>()?;
    let cols = reader.read_u16::<LE>()?;
    let spp: u16 = if reader.read_u8()? < 0x80 { 1 } else { 3 };

    obj.put(DataElement::new(
        tags::ROWS,
        VR::US,
        PrimitiveValue::from(rows),
    ));

    obj.put(DataElement::new(
        tags::COLUMNS,
        VR::US,
        PrimitiveValue::from(cols),
    ));

    obj.put(DataElement::new(
        tags::SAMPLES_PER_PIXEL,
        VR::US,
        PrimitiveValue::from(spp),
    ));

    if spp > 1 {
        obj.put_element(DataElement::new(
            tags::PLANAR_CONFIGURATION,
            VR::US,
            PrimitiveValue::from(0),
        ));
    }

    let pi = if spp == 3 { "RGB" } else { "MONOCHROME2" };
    obj.put(DataElement::new(
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        PrimitiveValue::from(pi),
    ));

    let bits_allocated = if reader.read_u8()? < 0x80 { 8 } else { 16 };
    let bits_stored = bits_allocated;
    let high_bit = bits_stored - 1;

    obj.put(DataElement::new(
        tags::BITS_ALLOCATED,
        VR::US,
        PrimitiveValue::from(bits_allocated),
    ));
    obj.put(DataElement::new(
        tags::BITS_STORED,
        VR::US,
        PrimitiveValue::from(bits_stored),
    ));
    obj.put(DataElement::new(
        tags::HIGH_BIT,
        VR::US,
        PrimitiveValue::from(high_bit),
    ));
    let pixel_representation = reader.read_u8()? >> 7;
    obj.put(DataElement::new(
        tags::PIXEL_REPRESENTATION,
        VR::US,
        PrimitiveValue::from(pixel_representation),
    ));

    if spp == 1 {
        let rescale_intercept = reader.read_f64::<LE>()?;
        let rescale_slope = reader.read_f64::<LE>()?;

        obj.put(DataElement::new(
            tags::RESCALE_INTERCEPT,
            VR::DS,
            PrimitiveValue::from(rescale_intercept.to_string()),
        ));
        obj.put(DataElement::new(
            tags::RESCALE_SLOPE,
            VR::DS,
            PrimitiveValue::from(rescale_slope.to_string()),
        ));
    }

    obj.put(DataElement::new(
        tags::VOILUT_FUNCTION,
        VR::CS,
        PrimitiveValue::from("IDENTITY"),
    ));

    // finally, grab some pixel data

    let size = rows as u64 * cols as u64 * spp as u64 * (bits_allocated / 8) as u64;

    let mut pixeldata = vec![];
    Read::take(reader, size).read_to_end(&mut pixeldata)?;

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        if bits_allocated == 16 { VR::OW } else { VR::OB },
        PrimitiveValue::from(pixeldata),
    ));

    let meta = FileMetaTableBuilder::new()
        .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
        .media_storage_sop_instance_uid("2.25.221743183549175336412959299516406387775")
        .transfer_syntax("1.2.840.10008.1.2.1");

    Ok(obj.with_meta(meta)?)
}

fn fuzz(data: &[u8]) -> Result<(), Box<dyn Error>> {
    // deserialize random bytes
    let obj = build_simple_image(data)?;

    // decode them as an image
    let decoded = obj.decode_pixel_data()?;

    // turn into native pixel data vector
    let pixels: Vec<u16> = decoded.to_vec()?;

    // assert that the vector length matches the expected number of samples
    let size =
        decoded.rows() as u64 * decoded.columns() as u64 * decoded.samples_per_pixel() as u64;

    assert_eq!(pixels.len() as u64, size);

    Ok(())
}
