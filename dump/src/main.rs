//! A CLI tool for inspecting the contents of a DICOM file
//! by printing it in a human readable format.
use clap::Parser;
use dicom_dump::{ColorMode, DumpOptions};
use dicom_object::open_file;
use snafu::{Report, Whatever};
use std::io::ErrorKind;
use std::path::PathBuf;

/// Exit code for when an error emerged while reading the DICOM file.
const ERROR_READ: i32 = -2;
/// Exit code for when an error emerged while dumping the file.
const ERROR_PRINT: i32 = -3;

/// Dump the contents of DICOM files
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// The DICOM file(s) to read
    #[clap(required = true)]
    files: Vec<PathBuf>,
    /// Print text values to the end
    /// (limited to `width` by default)
    #[clap(long = "no-text-limit")]
    no_text_limit: bool,
    /// Print all values to the end
    /// (implies `no_text_limit`, limited to `width` by default)
    #[clap(long = "no-limit")]
    no_limit: bool,
    /// The width of the display
    /// (default is to check automatically)
    #[clap(short = 'w', long = "width")]
    width: Option<u32>,
    /// The color mode
    #[clap(long = "color", default_value = "auto")]
    color: ColorMode,
    /// Fail if any errors are encountered
    #[clap(long = "fail-first")]
    fail_first: bool,
}

fn main() {
    run().unwrap_or_else(|e| {
        eprintln!("{}", Report::from_error(e));
        std::process::exit(-2);
    });
}

fn run() -> Result<(), Whatever> {
    let App {
        files: filenames,
        no_text_limit,
        no_limit,
        width,
        color,
        fail_first,
    } = App::parse();

    let width = width
        .or_else(|| terminal_size::terminal_size().map(|(width, _)| width.0 as u32))
        .unwrap_or(120);

    let mut options = DumpOptions::new();
    options
        .no_text_limit(no_text_limit)
        .no_limit(no_limit)
        .width(width)
        .color_mode(color);
    let fail_first = filenames.len() == 1 || fail_first;
    let mut errors: i32 = 0;

    for filename in &filenames {
        println!("{}: ", filename.display());
        match open_file(filename) {
            Err(e) => {
                eprintln!("{}", Report::from_error(e));
                if fail_first {
                    std::process::exit(ERROR_READ);
                }
                errors += 1;
            }
            Ok(obj) => {
                if let Err(ref e) = options.dump_file(&obj) {
                    if e.kind() == ErrorKind::BrokenPipe {
                        // handle broken pipe separately with a no-op
                    } else {
                        eprintln!("[ERROR] {}", Report::from_error(e));
                        if fail_first {
                            std::process::exit(ERROR_PRINT);
                        }
                    }
                    errors += 1;
                } // else all good
            }
        };
    }

    std::process::exit(errors);
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
