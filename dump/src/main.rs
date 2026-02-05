//! A CLI tool for inspecting the contents of a DICOM file
//! by printing it in a human readable format.
use clap::Parser;
use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_dump::{ColorMode, DumpFormat, DumpOptions};
use dicom_object::{file::OddLengthStrategy, OpenFileOptions, StandardDataDictionary};
use snafu::{Report, Whatever};
use std::io::{stdout, ErrorKind, IsTerminal, Write};
use std::path::PathBuf;

#[cfg(feature = "rayon")]
use std::sync::atomic::{AtomicI32, Ordering};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

/// Exit code for when an error emerged while reading or dumping the DICOM file.
const ERROR_READ: i32 = -2;

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
    #[clap(short = 'o', long = "odd-length-strategy", value_parser = parse_strategy, default_value = "accept")]
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
    /// Number of worker threads for parallel processing.
    /// Defaults to the number of CPU cores.
    /// Set to 1 to disable parallel processing.
    #[cfg(feature = "rayon")]
    #[clap(short = 'c', long = "concurrency")]
    concurrency: Option<usize>,
    /// Process files sequentially (disables parallel processing).
    /// Useful for debugging or when output order must match input order exactly.
    #[cfg(feature = "rayon")]
    #[clap(long = "sequential")]
    sequential: bool,
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

/// Result of processing a single file
struct FileResult {
    /// The output buffer (stdout content)
    output: Vec<u8>,
    /// The error buffer (stderr content)
    error_output: Vec<u8>,
    /// Whether an error occurred
    had_error: bool,
}

/// Process a single DICOM file and return the result
fn process_file(
    filename: &PathBuf,
    options: &DumpOptions,
    read_until: Option<Tag>,
    odd_length_strategy: OddLengthStrategy,
) -> FileResult {
    let mut output = Vec::new();
    let mut error_output = Vec::new();
    let mut had_error = false;

    // Write filename to error output
    writeln!(error_output, "{}: ", filename.display()).ok();

    let open_options = match read_until {
        Some(stop_tag) => OpenFileOptions::new().read_until(stop_tag),
        None => OpenFileOptions::new(),
    };

    let open_options = open_options.odd_length_strategy(odd_length_strategy);

    match open_options.open_file(filename) {
        Err(e) => {
            writeln!(error_output, "{}", Report::from_error(e)).ok();
            had_error = true;
        }
        Ok(mut obj) => {
            if options.format == DumpFormat::Json {
                // JSON output doesn't currently support encapsulated pixel data
                if let Ok(elem) = obj.element(tags::PIXEL_DATA) {
                    if let dicom_core::value::Value::PixelSequence(_) = elem.value() {
                        writeln!(
                            error_output,
                            "[WARN] Encapsulated pixel data not supported in JSON output, skipping"
                        )
                        .ok();
                        obj.remove_element(tags::PIXEL_DATA);
                    }
                }
            }
            if let Err(ref e) = options.dump_file_to(&mut output, &obj) {
                if e.kind() == ErrorKind::BrokenPipe {
                    // handle broken pipe separately with a no-op
                } else {
                    writeln!(error_output, "[ERROR] {}", Report::from_error(e)).ok();
                }
                had_error = true;
            }
        }
    };

    FileResult {
        output,
        error_output,
        had_error,
    }
}

/// Write results to stdout/stderr, returns whether there was an error
fn write_result(result: &FileResult) -> std::io::Result<bool> {
    // Write error output to stderr
    std::io::stderr().write_all(&result.error_output)?;
    // Write main output to stdout
    stdout().write_all(&result.output)?;
    Ok(result.had_error)
}

fn run() -> Result<(), Whatever> {
    let app = App::parse();

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
        #[cfg(feature = "rayon")]
        concurrency,
        #[cfg(feature = "rayon")]
        sequential,
    } = app;

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

    // Determine if we should use parallel processing
    #[cfg(feature = "rayon")]
    let use_parallel = !sequential && filenames.len() > 1;
    #[cfg(not(feature = "rayon"))]
    let use_parallel = false;

    // Configure thread pool if using parallelism
    #[cfg(feature = "rayon")]
    if use_parallel {
        if let Some(num_threads) = concurrency {
            rayon::ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build_global()
                .ok(); // Ignore error if pool already initialized
        }
    }

    if use_parallel {
        #[cfg(feature = "rayon")]
        {
            run_parallel(&filenames, &options, read_until, odd_length_strategy, fail_first)
        }
        #[cfg(not(feature = "rayon"))]
        {
            unreachable!("Parallel processing requires rayon feature")
        }
    } else {
        run_sequential(&filenames, &options, read_until, odd_length_strategy, fail_first)
    }
}

/// Run processing sequentially (original behavior)
fn run_sequential(
    filenames: &[PathBuf],
    options: &DumpOptions,
    read_until: Option<Tag>,
    odd_length_strategy: OddLengthStrategy,
    fail_first: bool,
) -> Result<(), Whatever> {
    let mut errors: i32 = 0;

    for filename in filenames {
        let result = process_file(filename, options, read_until, odd_length_strategy);

        if write_result(&result).unwrap_or(true) {
            if fail_first {
                std::process::exit(ERROR_READ);
            }
            errors += 1;
        }
    }

    std::process::exit(errors);
}

/// Run processing in parallel using rayon
#[cfg(feature = "rayon")]
fn run_parallel(
    filenames: &[PathBuf],
    options: &DumpOptions,
    read_until: Option<Tag>,
    odd_length_strategy: OddLengthStrategy,
    fail_first: bool,
) -> Result<(), Whatever> {
    use std::sync::atomic::AtomicBool;

    let errors = AtomicI32::new(0);
    let should_stop = AtomicBool::new(false);

    // Process files in parallel and collect results
    // We use par_iter to process files concurrently (good for I/O-bound work)
    // and rayon's work-stealing handles CPU-bound work efficiently
    let results: Vec<FileResult> = filenames
        .par_iter()
        .map(|filename| {
            // Check if we should stop early (fail-first mode)
            if fail_first && should_stop.load(Ordering::Relaxed) {
                return FileResult {
                    output: Vec::new(),
                    error_output: Vec::new(),
                    had_error: false, // Don't count skipped files as errors
                };
            }

            let result = process_file(filename, options, read_until, odd_length_strategy);

            if result.had_error && fail_first {
                should_stop.store(true, Ordering::Relaxed);
            }

            result
        })
        .collect();

    // Output results in original order to maintain deterministic output
    for result in &results {
        if fail_first && should_stop.load(Ordering::Relaxed) && result.output.is_empty() {
            // Skip files that were not processed due to early termination
            continue;
        }

        if write_result(result).unwrap_or(true) {
            if fail_first {
                std::process::exit(ERROR_READ);
            }
            errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    std::process::exit(errors.load(Ordering::Relaxed));
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
