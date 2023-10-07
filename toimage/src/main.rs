//! A CLI tool for converting a DICOM image file
//! into a general purpose image file (e.g. PNG).
use std::{borrow::Cow, path::PathBuf};

use clap::Parser;
use dicom_core::prelude::*;
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use dicom_pixeldata::{ConvertOptions, PixelDecoder};
use snafu::{OptionExt, Report, ResultExt, Snafu, Whatever};
use tracing::{error, Level};

/// Convert a DICOM file into an image file
#[derive(Debug, Parser)]
#[command(version)]
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

    /// Output the raw pixel data instead of decoding it
    #[arg(
        long = "unwrap",
        conflicts_with = "force_8bit",
        conflicts_with = "force_16bit"
    )]
    unwrap: bool,

    /// Print more information about the image and the output file
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("could not read DICOM file {}", path.display()))]
    ReadFile {
        #[snafu(source(from(dicom_object::ReadError, Box::new)))]
        source: Box<dicom_object::ReadError>,
        path: PathBuf,
    },
    /// failed to decode pixel data
    DecodePixelData {
        #[snafu(source(from(dicom_pixeldata::Error, Box::new)))]
        source: Box<dicom_pixeldata::Error>,
    },
    /// missing offset table entry for frame #{frame_number}
    MissingOffsetEntry { frame_number: u32 },
    /// missing key property {name}
    MissingProperty { name: &'static str },
    /// property {name} contains an invalid value
    InvalidPropertyValue {
        name: &'static str,
        #[snafu(source(from(dicom_core::value::ConvertValueError, Box::new)))]
        source: Box<dicom_core::value::ConvertValueError>,
    },
    /// pixel data of frame #{frame_number} is out of bounds
    FrameOutOfBounds { frame_number: u32 },
    /// failed to convert pixel data to image
    ConvertImage {
        #[snafu(source(from(dicom_pixeldata::Error, Box::new)))]
        source: Box<dicom_pixeldata::Error>,
    },
    /// failed to save image to file
    SaveImage {
        #[snafu(source(from(dicom_pixeldata::image::ImageError, Box::new)))]
        source: Box<dicom_pixeldata::image::ImageError>,
    },
    /// failed to save pixel data to file
    SaveData { source: std::io::Error },
    /// Unexpected DICOM pixel data as data set sequence
    UnexpectedPixelData,
}

impl Error {
    fn to_exit_code(&self) -> i32 {
        match self {
            Error::ReadFile { .. } => -1,
            Error::DecodePixelData { .. }
            | Error::MissingOffsetEntry { .. }
            | Error::MissingProperty { .. }
            | Error::InvalidPropertyValue { .. }
            | Error::FrameOutOfBounds { .. } => -2,
            Error::ConvertImage { .. } => -3,
            Error::SaveData { .. } | Error::SaveImage { .. } => -4,
            Error::UnexpectedPixelData => -7,
        }
    }
}

fn main() {
    let args = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if args.verbose {
                Level::DEBUG
            } else {
                Level::INFO
            })
            .finish(),
    )
    .whatever_context("Could not set up global logging subscriber")
    .unwrap_or_else(|e: Whatever| {
        eprintln!("[ERROR] {}", Report::from_error(e));
    });

    run(args).unwrap_or_else(|e| {
        let code = e.to_exit_code();
        error!("{}", Report::from_error(e));
        std::process::exit(code);
    });
}

fn run(args: App) -> Result<(), Error> {
    let App {
        file,
        output,
        frame_number,
        force_8bit,
        force_16bit,
        unwrap,
        verbose,
    } = args;

    let obj = open_file(&file).with_context(|_| ReadFileSnafu { path: file.clone() })?;

    if unwrap {
        let output = output.unwrap_or_else(|| {
            let mut path = file.clone();

            // try to identify a better extension for this file
            // based on transfer syntax
            match obj.meta().transfer_syntax() {
                uids::JPEG_BASELINE8_BIT
                | uids::JPEG_EXTENDED12_BIT
                | uids::JPEG_LOSSLESS
                | uids::JPEG_LOSSLESS_SV1 => {
                    path.set_extension("jpg");
                }
                uids::JPEG2000
                | uids::JPEG2000MC
                | uids::JPEG2000MC_LOSSLESS
                | uids::JPEG2000_LOSSLESS => {
                    path.set_extension("jp2");
                }
                _ => {
                    path.set_extension("data");
                }
            }
            path
        });

        let pixeldata = obj.get(tags::PIXEL_DATA).unwrap_or_else(|| {
            error!("DICOM file has no pixel data");
            std::process::exit(-2);
        });

        let out_data = match pixeldata.value() {
            DicomValue::PixelSequence(seq) => {
                let number_of_frames = match obj.get(tags::NUMBER_OF_FRAMES) {
                    Some(elem) => elem.to_int::<u32>().unwrap_or_else(|e| {
                        tracing::warn!("Invalid Number of Frames: {}", e);
                        1
                    }),
                    None => 1,
                };

                if number_of_frames as usize == seq.fragments().len() {
                    // frame-to-fragment mapping is 1:1

                    // get fragment containing our frame
                    let fragment =
                        seq.fragments()
                            .get(frame_number as usize)
                            .unwrap_or_else(|| {
                                error!("Frame number {} is out of range", frame_number);
                                std::process::exit(-2);
                            });

                    Cow::Borrowed(&fragment[..])
                } else {
                    // In this case we look up the basic offset table
                    // and gather all of the frame's fragments in a single vector.
                    // Note: not the most efficient way to do this,
                    // consider optimizing later with byte chunk readers
                    let offset_table = seq.offset_table();
                    let base_offset = offset_table.get(frame_number as usize).copied();
                    let base_offset = if frame_number == 0 {
                        base_offset.unwrap_or(0) as usize
                    } else {
                        base_offset.context(MissingOffsetEntrySnafu { frame_number })? as usize
                    };
                    let next_offset = offset_table.get(frame_number as usize + 1);

                    let mut offset = 0;
                    let mut frame_data = Vec::new();
                    for fragment in seq.fragments() {
                        // include it
                        if offset >= base_offset {
                            frame_data.extend_from_slice(fragment);
                        }
                        offset += fragment.len() + 8;
                        if let Some(&next_offset) = next_offset {
                            if offset >= next_offset as usize {
                                // next fragment is for the next frame
                                break;
                            }
                        }
                    }

                    Cow::Owned(frame_data)
                }
            }
            DicomValue::Primitive(v) => {
                // grab the intended slice based on image properties

                let get_int_property = |tag, name| {
                    obj.get(tag)
                        .context(MissingPropertySnafu { name })?
                        .to_int::<usize>()
                        .context(InvalidPropertyValueSnafu { name })
                };

                let rows = get_int_property(tags::ROWS, "Rows")?;
                let columns = get_int_property(tags::COLUMNS, "Columns")?;
                let samples_per_pixel =
                    get_int_property(tags::SAMPLES_PER_PIXEL, "Samples Per Pixel")?;
                let bits_allocated = get_int_property(tags::BITS_ALLOCATED, "Bits Allocated")?;
                let frame_size = rows * columns * samples_per_pixel * ((bits_allocated + 7) / 8);

                let frame = frame_number as usize;
                let mut data = v.to_bytes();
                match &mut data {
                    Cow::Borrowed(data) => {
                        *data = data
                            .get((frame_size * frame)..(frame_size * (frame + 1)))
                            .context(FrameOutOfBoundsSnafu { frame_number })?;
                    }
                    Cow::Owned(data) => {
                        *data = data
                            .get((frame_size * frame)..(frame_size * (frame + 1)))
                            .context(FrameOutOfBoundsSnafu { frame_number })?
                            .to_vec();
                    }
                }
                data
            }
            _ => {
                return UnexpectedPixelDataSnafu.fail();
            }
        };

        std::fs::write(output, out_data).context(SaveDataSnafu)?;
    } else {
        let output = output.unwrap_or_else(|| {
            let mut path = file.clone();
            path.set_extension("png");
            path
        });

        let pixel = obj
            .decode_pixel_data_frame(frame_number)
            .context(DecodePixelDataSnafu)?;

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
            .to_dynamic_image_with_options(0, &options)
            .context(ConvertImageSnafu)?;

        image.save(&output).context(SaveImageSnafu)?;

        if verbose {
            println!("Image saved to {}", output.display());
        }
    }

    Ok(())
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
