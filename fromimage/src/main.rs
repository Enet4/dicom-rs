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

use dicom::{
    core::{value::PrimitiveValue, DataElement, VR},
    dictionary_std::tags,
    object::{open_file, FileMetaTableBuilder},
};
use image::GenericImageView;
use snafu::ErrorCompat;
use structopt::StructOpt;

/// Convert and replace a DICOM file's image with another image
#[derive(Debug, StructOpt)]
struct App {
    /// Path to the base DICOM file to read
    dcm_file: PathBuf,
    /// Path to the image file to replace the DICOM file
    img_file: PathBuf,
    /// Path to the output image
    /// (default is to replace input extension with `.new.dcm`)
    #[structopt(short = "o", long = "out")]
    output: Option<PathBuf>,
    /// Print more information about the image and the output file
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
}

fn report<E: 'static>(err: &E)
where
    E: std::error::Error,
{
    eprintln!("[ERROR] {}", err);
    if let Some(source) = err.source() {
        eprintln!();
        eprintln!("Caused by:");
        for (i, e) in std::iter::successors(Some(source), |e| e.source()).enumerate() {
            eprintln!("   {}: {}", i, e);
        }
    }
}

fn report_backtrace<E: 'static>(err: &E)
where
    E: std::error::Error,
    E: ErrorCompat,
{
    let env_backtrace = std::env::var("RUST_BACKTRACE").unwrap_or_default();
    let env_lib_backtrace = std::env::var("RUST_LIB_BACKTRACE").unwrap_or_default();
    if env_lib_backtrace == "1" || (env_backtrace == "1" && env_lib_backtrace != "0") {
        if let Some(backtrace) = ErrorCompat::backtrace(&err) {
            eprintln!();
            eprintln!("Backtrace:");
            eprintln!("{}", backtrace);
        }
    }
}

fn report_with_backtrace<E: 'static>(err: E)
where
    E: std::error::Error,
    E: ErrorCompat,
{
    report(&err);
    report_backtrace(&err);
}

fn main() {
    let App {
        dcm_file,
        img_file,
        output,
        verbose,
    } = App::from_args();

    let output = output.unwrap_or_else(|| {
        let mut path = dcm_file.clone();
        path.set_extension("new.dcm");
        path
    });

    let mut obj = open_file(&dcm_file).unwrap_or_else(|e| {
        report_with_backtrace(e);
        std::process::exit(-1);
    });

    let img = image::open(img_file).unwrap_or_else(|e| {
        report(&e);
        std::process::exit(-1);
    });

    let (pi, spp, bits_stored, pixeldata): (&str, u16, u16, Vec<u8>) = match img.color() {
        image::ColorType::L8 => ("MONOCHROME2", 1, 8, img.to_bytes()),
        image::ColorType::L16 => ("MONOCHROME2", 1, 16, img.to_bytes()),
        image::ColorType::Rgb8 => ("RGB", 3, 8, img.to_bytes()),
        image::ColorType::Bgr8 => ("RGB", 3, 8, img.to_rgb8().into_raw()),
        _ => {
            eprintln!("Unsupported image format {:?}", img.color());
            std::process::exit(-2);
        }
    };

    if verbose {
        println!("{}x{} {:?} image", img.width(), img.height(), img.color());
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
        PrimitiveValue::from(img.width() as u16),
    ));
    obj.put(DataElement::new(
        tags::ROWS,
        VR::US,
        PrimitiveValue::from(img.height() as u16),
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

    for tag in [
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

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        if bits_stored == 8 { VR::OB } else { VR::OW },
        PrimitiveValue::from(pixeldata),
    ));

    let class_uid = obj.meta().media_storage_sop_class_uid.clone();

    let obj = obj.into_inner().with_meta(FileMetaTableBuilder::new()
            .transfer_syntax("1.2.840.10008.1.2.1")
            .media_storage_sop_class_uid(class_uid)
    ).unwrap_or_else(|e| {
        report_with_backtrace(e);
        std::process::exit(-3);
    });

    obj.write_to_file(&output).unwrap_or_else(|e| {
        report_with_backtrace(e);
        std::process::exit(-4);
    });

    if verbose {
        println!("DICOM file saved to {}", output.display());
    }
}
