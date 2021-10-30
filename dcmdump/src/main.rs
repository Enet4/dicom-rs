use dicom_dump::DumpOptions;
use dicom::object::open_file;
use std::io::ErrorKind;
use snafu::ErrorCompat;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
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

#[derive(Debug)]
struct ColoringError { }

#[derive(Clone, Copy, Debug)]
enum Coloring {
    Never,
    Auto,
    Always,
}

impl Display for ColoringError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid color mode")
    }
}

impl std::error::Error for Coloring {}

impl FromStr for Coloring {
    type Err = ColoringError;
    fn from_str(color: &str) -> Result<Self, Self::Err> {
        match color {
            "never" => Ok(Coloring::Never),
            "auto" => Ok(Coloring::Auto),
            "always" => Ok(Coloring::Always),
            _ => Err(ColoringError{})
        }
    }
}

impl std::fmt::Display for Coloring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Coloring::Never => f.write_str("never"),
            Coloring::Auto => f.write_str("auto"),
            Coloring::Always => f.write_str("always"),
        }
    }
}

/// Dump the contents of DICOM files
/// 
/// WARNING: Deprecated. Please install `dicom-dump` instead.
#[derive(Debug, StructOpt)]
struct App {
    /// The DICOM file to read
    file: PathBuf,
    /// whether text value width limit is disabled
    /// (limited to `width` by default)
    #[structopt(long = "no-text-limit")]
    no_text_limit: bool,
    /// the width of the display
    /// (default is to check automatically)
    #[structopt(short = "w", long = "width")]
    width: Option<u32>,
    /// color mode
    #[structopt(long="color", default_value = "auto")]
    color: Coloring
}

fn main() {
    os_compatibility().unwrap_or_else(|_| {
        eprintln!("Error setting OS compatibility for colored output");
    });

    let App {
        file: filename,
        no_text_limit,
        width,
        color,
    } = App::from_args();

    let obj = open_file(filename).unwrap_or_else(|e| {
        report(e);
        std::process::exit(ERROR_READ);
    });

    let width = width
        .or_else(|| term_size::dimensions().map(|(width, _)| width as u32))
        .unwrap_or(120);

    match color {
        Coloring::Never => colored::control::set_override(false),
        Coloring::Always => colored::control::set_override(true),
        _ => {}
    }

    let mut options = DumpOptions::new();
    match options
        .no_text_limit(no_text_limit)
        .width(width)
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
