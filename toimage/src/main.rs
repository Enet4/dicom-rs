//! A CLI tool for converting a DICOM image file
//! into a general purpose image file (e.g. PNG).
use std::path::PathBuf;

use clap::Parser;
use dicom_object::open_file;
use dicom_pixeldata::{ConvertOptions, PixelDecoder};
use snafu::{Report, ResultExt, Whatever};
use tracing::{error, Level};

/// Convert a DICOM file into an image
#[derive(Debug, Parser)]
struct App {
    /// Path to the DICOM file to convert
    file: PathBuf,

    /// Path to the output image
    /// (default is to replace input extension with `.png`)
    #[arg(short = 'o', long = "out")]
    output: Option<PathBuf>,

    /// Frame number (0-indexed)
    #[arg(short = 'F', long = "frame", default_value = "0")]
    frame_number: u32,

    /// Force output bit depth to 8 bits per sample
    #[arg(long = "8bit", conflicts_with = "force_16bit")]
    force_8bit: bool,

    /// Force output bit depth to 16 bits per sample
    #[arg(long = "16bit", conflicts_with = "force_8bit")]
    force_16bit: bool,

    /// Print more information about the image and the output file
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

fn main() {
    let App {
        file,
        output,
        frame_number,
        verbose,
        force_8bit,
        force_16bit,
    } = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
            .finish(),
    )
    .whatever_context("Could not set up global logging subscriber")
    .unwrap_or_else(|e: Whatever| {
        eprintln!("[ERROR] {}", Report::from_error(e));
    });

    let output = output.unwrap_or_else(|| {
        let mut path = file.clone();
        path.set_extension("png");
        path
    });

    let obj = open_file(&file).unwrap_or_else(|e| {
        error!("{}", Report::from_error(e));
        std::process::exit(-1);
    });

    let pixel = obj.decode_pixel_data().unwrap_or_else(|e| {
        error!("{}", Report::from_error(e));
        std::process::exit(-2);
    });

    if verbose {
        println!(
            "{}x{}x{} image, {}-bit",
            pixel.columns(),
            pixel.rows(),
            pixel.samples_per_pixel(),
            pixel.bits_stored()
        );
    }

    let mut options = ConvertOptions::new();

    if force_16bit {
        options = options.force_16bit();
    } else if force_8bit {
        options = options.force_8bit();
    }

    let image = pixel
        .to_dynamic_image_with_options(frame_number, &options)
        .unwrap_or_else(|e| {
            error!("{}", Report::from_error(e));
            std::process::exit(-3);
        });

    image.save(&output).unwrap_or_else(|e| {
        error!("{}", Report::from_error(e));
        std::process::exit(-4);
    });

    if verbose {
        println!("Image saved to {}", output.display());
    }
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
