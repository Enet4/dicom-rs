//! A CLI tool for inspecting the contents of a DICOM file
//! by printing it in a human readable format.
use dicom::object::open_file;
use dicom_dump::{ColorMode, DumpOptions};
use snafu::ErrorCompat;
use std::io::ErrorKind;
use std::path::PathBuf;
use structopt::StructOpt;

/// Exit code for when an error emerged while reading the DICOM file.
const ERROR_READ: i32 = -2;
/// Exit code for when an error emerged while dumping the file.
const ERROR_PRINT: i32 = -3;

#[cfg(windows)]
fn os_compatibility() -> Result<(), ()> {
    control::set_virtual_terminal(true)
}

#[cfg(not(windows))]
fn os_compatibility() -> Result<(), ()> {
    Ok(())
}

/// Dump the contents of DICOM files
#[derive(Debug, StructOpt)]
struct App {
    /// The DICOM file(s) to read
    files: Vec<PathBuf>,
    /// whether text value width limit is disabled
    /// (limited to `width` by default)
    #[structopt(long = "no-text-limit")]
    no_text_limit: bool,
    /// the width of the display
    /// (default is to check automatically)
    #[structopt(short = "w", long = "width")]
    width: Option<u32>,
    /// color mode
    #[structopt(long = "color", default_value = "auto")]
    color: ColorMode,
}

fn main() {
    os_compatibility().unwrap_or_else(|_| {
        eprintln!("Error setting OS compatibility for colored output");
    });

    let App {
        files: filenames,
        no_text_limit,
        width,
        color,
    } = App::from_args();

    let width = width
        .or_else(|| term_size::dimensions().map(|(width, _)| width as u32))
        .unwrap_or(120);

    let mut options = DumpOptions::new();

    for filename in filenames {
        let obj = open_file(filename).unwrap_or_else(|e| {
            report(e);
            std::process::exit(ERROR_READ);
        });
        
        match options
            .no_text_limit(no_text_limit)
            .width(width)
            .color_mode(color)
            .dump_file(&obj)
        {
            Err(ref e) if e.kind() == ErrorKind::BrokenPipe => {
                // handle broken pipe separately with a no-op
            }
            Err(e) => {
                eprintln!("[ERROR] {}", e);
                std::process::exit(ERROR_PRINT);
            }
            _ => {} // all good
        }
    }
}

fn report<E: 'static>(err: E)
where
    E: std::error::Error,
    E: ErrorCompat,
{
    eprintln!("[ERROR] {}", err);
    if let Some(source) = err.source() {
        eprintln!();
        eprintln!("Caused by:");
        for (i, e) in std::iter::successors(Some(source), |e| e.source()).enumerate() {
            eprintln!("   {}: {}", i, e);
        }
    }

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
