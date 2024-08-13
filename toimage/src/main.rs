//! A CLI tool for converting a DICOM image file
//! into a general purpose image file (e.g. PNG).
use std::{borrow::Cow, path::PathBuf, str::FromStr};

use clap::Parser;
use dicom_core::prelude::*;
use dicom_dictionary_std::{tags, uids};
use dicom_object::{open_file, FileDicomObject, InMemDicomObject};
use dicom_pixeldata::{ConvertOptions, PixelDecoder};
use snafu::{OptionExt, Report, ResultExt, Snafu, Whatever};
use tracing::{error, warn, Level};

/// Convert DICOM files into image files
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// A directory or multiple paths to the DICOM files to convert
    #[arg(required(true))]
    files: Vec<PathBuf>,

    /// Parse the given directory recursively
    #[arg(short = 'r', long = "recursive")]
    recursive: bool,

    /// Path to the output image, including file extension
    /// (replaces input extension with `.png` by default)
    #[arg(short = 'o', long = "out")]
    output: Option<PathBuf>,

    /// Path to the output directory in bulk conversion mode,
    /// conflicts with `output`
    #[arg(short = 'd', long = "outdir", conflicts_with = "output")]
    outdir: Option<PathBuf>,

    /// Extension when converting multiple files
    /// (default is to replace input extension with `.png`)
    #[arg(short = 'e', long = "ext", conflicts_with = "output")]
    ext: Option<String>,

    /// Frame number (0-indexed)
    #[arg(short = 'F', long = "frame", default_value = "0")]
    frame_number: u32,

    #[clap(flatten)]
    image_options: ImageOptions,

    /// Stop on the first failed conversion
    #[arg(long)]
    fail_first: bool,

    /// Print more information about the image and the output file
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

/// Options related to image output and conversion steps
#[derive(Debug, Copy, Clone, Parser)]
struct ImageOptions {
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
    /// Decode all pixel data frames instead of just the one intended
    #[arg(hide(true), long)]
    decode_all: bool,
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
    /// No files given
    NoFiles,
    /// Read dir error
    ReadDir { source: std::io::Error },
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
            Error::NoFiles => -8,
            Error::ReadDir { .. } => -9,
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
        files,
        recursive,
        outdir,
        output,
        ext,
        frame_number,
        image_options,
        fail_first,
        verbose,
    } = args;

    if files.is_empty() {
        return Err(Error::NoFiles);
    };

    if files.len() == 1 {
        let file = &files[0];
        if file.is_dir() {
            // single directory
            let dicoms: Vec<(FileDicomObject<InMemDicomObject>, PathBuf)> =
                collect_dicom_files(file, recursive)?;

            if dicoms.is_empty() {
                return Err(Error::NoFiles);
            }

            for file in dicoms.iter() {
                let output = build_output_path(
                    false,
                    file.1.clone(),
                    outdir.clone(),
                    ext.clone(),
                    image_options.unwrap,
                );

                convert_single_file(&file.0, false, output, frame_number, image_options, verbose)
                    .or_else(|e| {
                    if fail_first {
                        Err(e)
                    } else {
                        let report = Report::from_error(e);
                        error!("Converting {}: {}", file.1.display(), report);
                        Ok(())
                    }
                })?;
            }
        } else {
            // single DICOM file
            let dcm = open_file(file).with_context(|_| ReadFileSnafu { path: file.clone() })?;

            let output_is_set = output.is_some();
            let output = build_output_path(
                output_is_set,
                output.unwrap_or(files[0].clone()),
                outdir.clone(),
                ext.clone(),
                image_options.unwrap,
            );

            convert_single_file(
                &dcm,
                output_is_set,
                output,
                frame_number,
                image_options,
                verbose,
            )?;
        }
    } else {
        // multiple DICOM files
        for file in files.iter() {
            let dicom_file =
                match open_file(file).with_context(|_| ReadFileSnafu { path: file.clone() }) {
                    Ok(file) => file,
                    Err(e) => {
                        if fail_first {
                            return Err(e);
                        } else {
                            error!("{}", Report::from_error(e));
                            continue;
                        }
                    }
                };

            let output = build_output_path(
                false,
                file.clone(),
                outdir.clone(),
                ext.clone(),
                image_options.unwrap,
            );

            convert_single_file(
                &dicom_file,
                false,
                output,
                frame_number,
                image_options,
                verbose,
            )
            .or_else(|e| {
                if fail_first {
                    Err(e)
                } else {
                    let report = Report::from_error(e);
                    error!("Converting {}: {}", file.display(), report);
                    Ok(())
                }
            })?;
        }
    }

    Ok(())
}

fn build_output_path(
    output_is_set: bool,
    mut output: PathBuf,
    outdir: Option<PathBuf>,
    ext: Option<String>,
    unwrap: bool,
) -> PathBuf {
    // check if there is a .dcm extension, otherwise, add it
    if output.extension() != Some("dcm".as_ref()) && !output_is_set {
        let pathstr = output.to_str().unwrap();
        // it is impossible to use set_extension here since dicom file names commonly have dots in
        // them which would be interpreted as file extensions
        output = PathBuf::from_str(&format!("{}.dcm", pathstr)).unwrap();
    }

    if let Some(outdir) = outdir {
        output = outdir.join(output.file_name().unwrap());
    }

    if !unwrap && !output_is_set {
        if let Some(extension) = ext {
            output.set_extension(extension);
        } else {
            output.set_extension("png");
        }
    }

    output
}

fn convert_single_file(
    file: &FileDicomObject<InMemDicomObject>,
    output_is_set: bool,
    mut output: PathBuf,
    frame_number: u32,
    image_options: ImageOptions,
    verbose: bool,
) -> Result<(), Error> {
    let ImageOptions {
        force_8bit,
        force_16bit,
        unwrap,
        decode_all,
    } = image_options;

    if unwrap {
        if !output_is_set {
            match file.meta().transfer_syntax() {
                uids::JPEG_BASELINE8_BIT
                | uids::JPEG_EXTENDED12_BIT
                | uids::JPEG_LOSSLESS
                | uids::JPEG_LOSSLESS_SV1 => {
                    output.set_extension("jpg");
                }
                uids::JPEG2000
                | uids::JPEG2000MC
                | uids::JPEG2000MC_LOSSLESS
                | uids::JPEG2000_LOSSLESS => {
                    output.set_extension("jp2");
                }
                _ => {
                    output.set_extension("data");
                }
            }
        }

        let pixeldata = file.get(tags::PIXEL_DATA).with_context(|| {
            error!("{}: DICOM file has no pixel data", output.display());
            MissingPropertySnafu { name: "PixelData" }
        })?;

        let out_data = match pixeldata.value() {
            DicomValue::PixelSequence(seq) => {
                let number_of_frames = match file.get(tags::NUMBER_OF_FRAMES) {
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
                            .with_context(|| {
                                error!(
                                    "{}: Frame number {} is out of range",
                                    output.display(),
                                    frame_number
                                );
                                FrameOutOfBoundsSnafu { frame_number }
                            })?;

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
                    file.get(tag)
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
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::write(output, out_data).context(SaveDataSnafu)?;
    } else {
        let pixel = if decode_all {
            file.decode_pixel_data().context(DecodePixelDataSnafu)?
        } else {
            file.decode_pixel_data_frame(frame_number)
                .context(DecodePixelDataSnafu)?
        };

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

        // the effective frame number
        let frame_num = if decode_all { frame_number } else { 0 };
        let image = pixel
            .to_dynamic_image_with_options(frame_num, &options)
            .context(ConvertImageSnafu)?;

        std::fs::create_dir_all(output.parent().unwrap()).unwrap();

        image.save(&output).context(SaveImageSnafu)?;

        if verbose {
            println!("Image saved to {}", output.display());
        }
    }

    Ok(())
}

fn collect_dicom_files(
    file: &PathBuf,
    recursive: bool,
) -> Result<Vec<(FileDicomObject<InMemDicomObject>, PathBuf)>, Error> {
    let mut dicoms = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(file).with_context(|_| ReadDirSnafu)?;
    entries.for_each(|entry| match entry {
        Ok(entry) => {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                let obj = match open_file(&path) {
                    Ok(obj) => obj,
                    Err(e) => {
                        warn!("Error reading file {:?}: {}", path, e);
                        return;
                    }
                };
                dicoms.push((obj, path));
            }
        }
        Err(e) => {
            error!("Error reading directory: {}", e);
        }
    });
    if recursive {
        dirs.iter()
            .for_each(|dir| match collect_dicom_files(dir, recursive) {
                Ok(mut d) => dicoms.append(&mut d),
                Err(e) => error!("Error reading directory {:?}: {}", dir, e),
            });
    }
    Ok(dicoms)
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
