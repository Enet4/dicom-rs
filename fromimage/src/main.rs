//! A CLI tool for overriding a DICOM file's image with another one.
//!
//! This command line tool takes a base DICOM file
//! and replaces the various DICOM attributes of the [_Image Pixel_ module][1]
//! (such as Rows, Columns, PixelData, ...)
//! with those of another file.
//! The _Presentation LUT Shape_ attribute is set to `IDENTITY`.
//! Other attributes are copied as is.
//!
//! The new DICOM object is saved to a new file,
//! with the same SOP instance UID and SOP class UID as the base file,
//! encoded in Explicit VR Little Endian.
//!
//! [1]: https://dicom.nema.org/medical/dicom/current/output/chtml/part03/sect_C.7.6.3.html

use std::path::PathBuf;

use clap::Parser;
use dicom_core::{
    value::{PixelFragmentSequence, PrimitiveValue},
    DataElement, DicomValue, VR,
};
use dicom_dictionary_std::tags;
use dicom_object::{open_file, DefaultDicomObject, FileMetaTableBuilder};
use image::DynamicImage;

type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;

/// Convert and replace a DICOM file's image with another image
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// Path to the base DICOM file to read
    dcm_file: PathBuf,
    /// Path to the image file to replace the DICOM file
    img_file: PathBuf,
    /// Path to the output image
    /// (default is to replace input extension with `.new.dcm`)
    #[arg(short = 'o', long = "out")]
    output: Option<PathBuf>,
    /// Override the transfer syntax UID (pixel data is not converted)
    #[arg(long = "transfer-syntax", alias = "ts")]
    transfer_syntax: Option<String>,
    /// Encapsulate the image file raw data in a fragment sequence
    /// instead of writing native pixel data
    #[arg(long)]
    encapsulate: bool,
    /// Retain the implementation class UID and version name from base DICOM
    #[arg(long)]
    retain_implementation: bool,
    /// Print more information about the image and the output file
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

fn main() {
    tracing::subscriber::set_global_default(tracing_subscriber::FmtSubscriber::new())
        .unwrap_or_else(|e| {
            eprintln!("{}", snafu::Report::from_error(e));
        });

    let App {
        dcm_file,
        img_file,
        output,
        encapsulate,
        transfer_syntax,
        retain_implementation,
        verbose,
    } = App::parse();

    let output = output.unwrap_or_else(|| {
        let mut path = dcm_file.clone();
        path.set_extension("new.dcm");
        path
    });

    let mut obj = open_file(&dcm_file).unwrap_or_else(|e| {
        tracing::error!("{}", snafu::Report::from_error(e));
        std::process::exit(-1);
    });

    if encapsulate {
        inject_encapsulated(&mut obj, img_file, verbose)
    } else {
        inject_image(&mut obj, img_file, verbose)
    }
    .unwrap_or_else(|e| {
        tracing::error!("{}", snafu::Report::from_error(e));
        std::process::exit(-2);
    });

    let class_uid = obj.meta().media_storage_sop_class_uid.clone();

    let mut meta_builder = FileMetaTableBuilder::new()
        // currently the tool will always decode the image's pixel data,
        // so encode it as Explicit VR Little Endian
        .transfer_syntax("1.2.840.10008.1.2.1")
        .media_storage_sop_class_uid(class_uid);

    if let Some(ts) = transfer_syntax {
        meta_builder = meta_builder.transfer_syntax(ts);
    }

    // recover implementation class UID and version name from base object
    if retain_implementation {
        let implementation_class_uid = &obj.meta().implementation_class_uid;
        meta_builder = meta_builder.implementation_class_uid(implementation_class_uid);

        if let Some(implementation_version_name) = obj.meta().implementation_version_name.as_ref() {
            meta_builder = meta_builder.implementation_version_name(implementation_version_name);
        }
    }

    let obj = obj
        .into_inner()
        .with_meta(meta_builder)
        .unwrap_or_else(|e| {
            tracing::error!("{}", snafu::Report::from_error(e));
            std::process::exit(-3);
        });

    obj.write_to_file(&output).unwrap_or_else(|e| {
        tracing::error!("{}", snafu::Report::from_error(e));
        std::process::exit(-4);
    });

    if verbose {
        println!("DICOM file saved to {}", output.display());
    }
}

fn inject_image(obj: &mut DefaultDicomObject, img_file: PathBuf, verbose: bool) -> Result<()> {
    let image_reader = image::ImageReader::open(img_file).unwrap_or_else(|e| {
        tracing::error!("{}", snafu::Report::from_error(e));
        std::process::exit(-1);
    });

    let img = image_reader.decode().unwrap_or_else(|e| {
        tracing::error!("{}", snafu::Report::from_error(e));
        std::process::exit(-1);
    });

    let color = img.color();

    let bits_stored: u16 = match color {
        image::ColorType::L8 => 8,
        image::ColorType::L16 => 16,
        image::ColorType::Rgb8 => 8,
        image::ColorType::Rgb16 => 16,
        _ => {
            eprintln!("Unsupported image format {:?}", color);
            std::process::exit(-2);
        }
    };

    update_from_img(obj, &img, verbose);

    for tag in [
        tags::NUMBER_OF_FRAMES,
        tags::PIXEL_ASPECT_RATIO,
        tags::SMALLEST_IMAGE_PIXEL_VALUE,
        tags::LARGEST_IMAGE_PIXEL_VALUE,
        tags::PIXEL_PADDING_RANGE_LIMIT,
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::ICC_PROFILE,
        tags::COLOR_SPACE,
        tags::PIXEL_DATA_PROVIDER_URL,
        tags::EXTENDED_OFFSET_TABLE,
        tags::EXTENDED_OFFSET_TABLE_LENGTHS,
    ] {
        obj.remove_element(tag);
    }

    let pixeldata = img.into_bytes();

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        if bits_stored == 8 { VR::OB } else { VR::OW },
        PrimitiveValue::from(pixeldata),
    ));

    Ok(())
}

fn inject_encapsulated(
    dcm: &mut DefaultDicomObject,
    img_file: PathBuf,
    verbose: bool,
) -> Result<()> {
    let image_reader = image::ImageReader::open(&img_file).unwrap_or_else(|e| {
        tracing::error!("{}", snafu::Report::from_error(e));
        std::process::exit(-1);
    });

    // collect img file data
    let all_data = std::fs::read(img_file).unwrap_or_else(|e| {
        tracing::error!("{}", snafu::Report::from_error(e));
        std::process::exit(-2);
    });

    if let Ok(img) = image_reader.decode() {
        // insert attributes but not pixel data

        update_from_img(&mut *dcm, &img, verbose);
    }

    // insert pixel data in a sequence
    dcm.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        DicomValue::PixelSequence(PixelFragmentSequence::new_fragments(vec![all_data])),
    ));

    Ok(())
}

fn update_from_img(obj: &mut DefaultDicomObject, img: &DynamicImage, verbose: bool) {
    let width = img.width();
    let height = img.height();
    let color = img.color();

    let (pi, spp, bits_stored): (&str, u16, u16) = match color {
        image::ColorType::L8 => ("MONOCHROME2", 1, 8),
        image::ColorType::L16 => ("MONOCHROME2", 1, 16),
        image::ColorType::Rgb8 => ("RGB", 3, 8),
        image::ColorType::Rgb16 => ("RGB", 3, 16),
        _ => {
            eprintln!("Unsupported image format {:?}", color);
            std::process::exit(-2);
        }
    };

    if verbose {
        println!("{}x{} {:?} image", width, height, color);
    }

    // override attributes at DICOM object
    obj.put(DataElement::new(
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        PrimitiveValue::from(pi),
    ));

    obj.put(DataElement::new(
        tags::PRESENTATION_LUT_SHAPE,
        VR::CS,
        PrimitiveValue::from("IDENTITY"),
    ));

    obj.put(DataElement::new(
        tags::SAMPLES_PER_PIXEL,
        VR::US,
        PrimitiveValue::from(spp),
    ));

    if spp > 1 {
        obj.put(DataElement::new(
            tags::PLANAR_CONFIGURATION,
            VR::US,
            PrimitiveValue::from(0_u16),
        ));
    } else {
        obj.remove_element(tags::PLANAR_CONFIGURATION);
    }

    obj.put(DataElement::new(
        tags::COLUMNS,
        VR::US,
        PrimitiveValue::from(width as u16),
    ));
    obj.put(DataElement::new(
        tags::ROWS,
        VR::US,
        PrimitiveValue::from(height as u16),
    ));
    obj.put(DataElement::new(
        tags::BITS_ALLOCATED,
        VR::US,
        PrimitiveValue::from(bits_stored),
    ));
    obj.put(DataElement::new(
        tags::BITS_STORED,
        VR::US,
        PrimitiveValue::from(bits_stored),
    ));
    obj.put(DataElement::new(
        tags::HIGH_BIT,
        VR::US,
        PrimitiveValue::from(bits_stored - 1),
    ));

    obj.put(DataElement::new(
        tags::PIXEL_REPRESENTATION,
        VR::US,
        PrimitiveValue::from(0_u16),
    ));
}

#[cfg(test)]
mod tests {
    use crate::App;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        App::command().debug_assert();
    }
}
