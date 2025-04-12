//! A CLI tool for inspecting the contents of a DICOM file
//! by printing it in a human readable format.
use clap::Parser;
use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_dump::{ColorMode, DumpFormat, DumpOptions};
use dicom_object::{file::OddLengthStrategy, OpenFileOptions, StandardDataDictionary};
use snafu::{Report, Whatever};
use std::io::{ErrorKind, IsTerminal};
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
    /// Read the file up to this tag
    #[clap(long = "until", value_parser = parse_tag)]
    read_until: Option<Tag>,
    /// Strategy for handling odd-length text values
    ///
    /// accept: Accept elements with an odd length as is,
    /// continuing data set reading normally.
    ///
    /// next_even: Assume that the real length is `length + 1`,
    /// as in the next even number.
    ///
    /// fail: Raise an error instead
    #[clap(short = 'o', long = "odd_length_strategy", value_parser = parse_strategy, default_value = "accept")]
    odd_length_strategy: OddLengthStrategy,
    /// Print text values to the end
    /// (limited to `width` by default).
    ///
    /// Does not apply if output is not a tty
    /// or if output type is json
    #[clap(long = "no-text-limit")]
    no_text_limit: bool,
    /// Print all values to the end
    /// (implies `no_text_limit`, limited to `width` by default)
    #[clap(long = "no-limit")]
    no_limit: bool,
    /// The width of the display
    /// (default is to check automatically).
    ///
    /// Does not apply if output is not a tty
    /// or if output type is json
    #[clap(short = 'w', long = "width")]
    width: Option<u32>,
    /// The color mode
    #[clap(long = "color", default_value = "auto")]
    color: ColorMode,
    /// Fail if any errors are encountered
    #[clap(long = "fail-first")]
    fail_first: bool,
    /// Output format
    #[arg(value_enum)]
    #[clap(short = 'f', long = "format", default_value = "text")]
    format: DumpFormat,
}

fn parse_strategy(s: &str) -> Result<OddLengthStrategy, &'static str> {
    match s {
        "accept" => Ok(OddLengthStrategy::Accept),
        "next_even" => Ok(OddLengthStrategy::NextEven),
        "fail" => Ok(OddLengthStrategy::Fail),
        _ => Err("invalid strategy"),
    }
}

fn parse_tag(s: &str) -> Result<Tag, &'static str> {
    use dicom_core::dictionary::DataDictionary as _;
    StandardDataDictionary.parse_tag(s).ok_or("invalid tag")
}

fn is_terminal() -> bool {
    std::io::stdout().is_terminal()
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
        read_until,
        odd_length_strategy,
        no_text_limit,
        no_limit,
        width,
        color,
        fail_first,
        format,
    } = App::parse();

    let width = width
        .or_else(|| terminal_size::terminal_size().map(|(width, _)| width.0 as u32))
        .unwrap_or(120);

    let mut options = DumpOptions::new();
    options
        .no_text_limit(no_text_limit)
        // No limit when output is not a terminal
        .no_limit(if !is_terminal() { true } else { no_limit })
        .width(width)
        .color_mode(color)
        .format(format);
    let fail_first = filenames.len() == 1 || fail_first;
    let mut errors: i32 = 0;

    for filename in &filenames {
        // Write filename to stderr to make piping easier, i.e. dicom-dump -o json file.dcm | jq
        eprintln!("{}: ", filename.display());

        let open_options = match read_until {
            Some(stop_tag) => OpenFileOptions::new().read_until(stop_tag),
            None => OpenFileOptions::new(),
        };

        let open_options = open_options.odd_length_strategy(odd_length_strategy);

        match open_options.open_file(filename) {
            Err(e) => {
                eprintln!("{}", Report::from_error(e));
                if fail_first {
                    std::process::exit(ERROR_READ);
                }
                errors += 1;
            }
            Ok(mut obj) => {
                if options.format == DumpFormat::Json {
                    // JSON output doesn't currently support encapsulated pixel data
                    if let Ok(elem) = obj.element(tags::PIXEL_DATA) {
                        if let dicom_core::value::Value::PixelSequence(_) = elem.value() {
                            eprintln!("[WARN] Encapsulated pixel data not supported in JSON output, skipping");
                            obj.remove_element(tags::PIXEL_DATA);
                        }
                    }
                }
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
