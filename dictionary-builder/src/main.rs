//! A simple application that downloads the data dictionary
//! from the latest DICOM standard found online, then creates
//! code or data to reproduce it in the core library.
//!
//! This is a work in progress. It can already retrieve attributes with
//! very specific tags, but might skip some patterns found in the standard
//! (such as (60xx,3000), which is for overlay data). A better way to handle
//! these cases is due.
//!
//! ### How to use
//!
//! Simply run the application. It will automatically retrieve the dictionary
//! from the official DICOM website and store the result in "entries.rs".
//! Future versions will enable different kinds of outputs.
//!
//! ```none
//! dicom-dictionary-builder --help
//! ```

mod data_dict;
mod uid_dict;

use structopt::StructOpt;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, StructOpt)]
pub enum DicomDict {
    /// Build the standard data dictionary
    DataDict(DataDict),
    /// Build the normative UID dictionary
    NormativeUidDict(UidDict),
}

/// url to PS3.6 XML file
pub const DEFAULT_LOCATION: &str =
    "http://dicom.nema.org/medical/dicom/current/source/docbook/part06/part06.xml";

/// Dictionary output format
#[derive(Debug)]
pub enum OutputFormat {
    /// Output to a Rust file, retaining entries as a static vector of records.
    Rs,
    /// Output to a JSON file.
    Json,
}

impl FromStr for OutputFormat {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rs" => Ok(OutputFormat::Rs),
            "json" => Ok(OutputFormat::Json),
            _ => Err("Illegal output format"),
        }
    }
}

/// DICOM data dictionary builder
#[derive(Debug, StructOpt)]
pub struct DataDict {
    /// Where to fetch the dictionary from
    #[structopt(
        name = "FROM",
        default_value = "http://dicom.nema.org/medical/dicom/current/source/docbook/part06/part06.xml"
    )]
    from: String,
    /// The path to the output file
    #[structopt(name = "OUTPUT", short = "o", parse(from_os_str))]
    output: Option<PathBuf>,

    /// The output format
    #[structopt(short = "f", long = "format", default_value = "rs")]
    format: OutputFormat,

    /// whether to ignore retired attributes
    ignore_retired: bool,
}

/// DICOM normative UID dictionary builder
#[derive(Debug, StructOpt)]
pub struct UidDict {
    /// Where to fetch the dictionary from
    #[structopt(
        name = "FROM",
        default_value = "http://dicom.nema.org/medical/dicom/current/source/docbook/part06/part06.xml"
    )]
    from: String,
    /// The path to the output file
    #[structopt(name = "OUTPUT", short = "o", parse(from_os_str))]
    output: Option<PathBuf>,

    /// The output format
    #[structopt(short = "f", long = "format", default_value = "rs")]
    format: OutputFormat,

    /// whether to ignore retired UIDs
    ignore_retired: bool,
}

fn main() {
    match DicomDict::from_args() {
        DicomDict::DataDict(args) => data_dict::run(args).unwrap(),
        DicomDict::NormativeUidDict(args) => uid_dict::run(args).unwrap(),
    }
}
