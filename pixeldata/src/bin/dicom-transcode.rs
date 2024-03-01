//! A CLI tool for transcoding a DICOM file
//! to another transfer syntax.
use clap::Parser;
use dicom_dictionary_std::uids;
use dicom_encoding::adapters::EncodeOptions;
use dicom_encoding::{TransferSyntax, TransferSyntaxIndex};
use dicom_object::open_file;
use dicom_pixeldata::Transcode;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use snafu::{OptionExt, Report, Whatever};
use std::path::PathBuf;
use tracing::Level;

/// Exit code for when an error emerged while reading the DICOM file.
const ERROR_READ: i32 = -2;
/// Exit code for when an error emerged while transcoding the file.
const ERROR_TRANSCODE: i32 = -3;
/// Exit code for when an error emerged while writing the file.
const ERROR_WRITE: i32 = -4;
/// Exit code for when an error emerged while writing the file.
const ERROR_OTHER: i32 = -128;

/// Transcode a DICOM file
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    file: PathBuf,
    /// The output file (default is to change the extension to .new.dcm)
    #[clap(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// The encoding quality (from 0 to 100)
    #[clap(long = "quality")]
    quality: Option<u8>,
    /// The encoding effort (from 0 to 100)
    #[clap(long = "effort")]
    effort: Option<u8>,

    /// Target transfer syntax
    #[clap(flatten)]
    target_ts: TargetTransferSyntax,

    /// Retain the original implementation class UID and version name
    #[clap(long)]
    retain_implementation: bool,

    /// Verbose mode
    #[clap(short = 'v', long = "verbose")]
    verbose: bool,
}

/// Specifier for the target transfer syntax
#[derive(Debug, Parser)]
#[group(required = true, multiple = false)]
struct TargetTransferSyntax {
    /// Transcode to the Transfer Syntax indicated by UID
    #[clap(long = "ts")]
    ts: Option<String>,

    /// Transcode to Explicit VR Little Endian
    #[clap(long = "expl-vr-le")]
    explicit_vr_le: bool,

    /// Transcode to Implicit VR Little Endian
    #[clap(long = "impl-vr-le")]
    implicit_vr_le: bool,

    /// Transcode to JPEG baseline (8-bit)
    #[clap(long = "jpeg-baseline")]
    jpeg_baseline: bool,
}

impl TargetTransferSyntax {
    fn resolve(&self) -> Result<&'static TransferSyntax, Whatever> {
        // explicit VR little endian
        if self.explicit_vr_le {
            return Ok(TransferSyntaxRegistry
                .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .expect("Explicit VR Little Endian is missing???"));
        }

        // implicit VR little endian
        if self.implicit_vr_le {
            return Ok(TransferSyntaxRegistry
                .get(uids::IMPLICIT_VR_LITTLE_ENDIAN)
                .expect("Implicit VR Little Endian is missing???"));
        }

        // JPEG baseline
        if self.jpeg_baseline {
            return TransferSyntaxRegistry
                .get(uids::JPEG_BASELINE8_BIT)
                .whatever_context("Missing specifier for JPEG Baseline (8-bit)");
        }

        // by TS UID
        let Some(ts) = &self.ts else {
            snafu::whatever!("No target transfer syntax specified");
        };

        TransferSyntaxRegistry
            .get(ts)
            .whatever_context("Unknown transfer syntax")
    }
}

fn main() {
    run().unwrap_or_else(|e| {
        eprintln!("{}", Report::from_error(e));
        std::process::exit(ERROR_OTHER);
    });
}

fn run() -> Result<(), Whatever> {
    let App {
        file,
        output,
        quality,
        effort,
        target_ts,
        retain_implementation,
        verbose,
    } = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
            .finish(),
    )
    .unwrap_or_else(|e| {
        eprintln!("{}", snafu::Report::from_error(e));
    });

    let output = output.unwrap_or_else(|| {
        let mut file = file.clone();
        file.set_extension("new.dcm");
        file
    });

    let mut obj = open_file(file).unwrap_or_else(|e| {
        eprintln!("{}", Report::from_error(e));
        std::process::exit(ERROR_READ);
    });

    // lookup transfer syntax
    let ts = target_ts.resolve()?;

    let mut options = EncodeOptions::default();
    options.quality = quality;
    options.effort = effort;

    obj.transcode_with_options(ts, options).unwrap_or_else(|e| {
        eprintln!("{}", Report::from_error(e));
        std::process::exit(ERROR_TRANSCODE);
    });

    // override implementation class UID and version name
    if !retain_implementation {
        obj.meta_mut().implementation_class_uid =
            dicom_object::IMPLEMENTATION_CLASS_UID.to_string();
        obj.meta_mut().implementation_version_name =
            Some(dicom_object::IMPLEMENTATION_VERSION_NAME.to_string());
    }

    // write to file
    obj.write_to_file(output).unwrap_or_else(|e| {
        eprintln!("{}", Report::from_error(e));
        std::process::exit(ERROR_WRITE);
    });

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
