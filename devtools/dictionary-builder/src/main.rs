//! A simple application that downloads the data dictionary and creates code or
//! data to reproduce it in the core library.
//!
//! ### How to use
//!
//! Run the application with one of the following subcommands:
//! 
//! - **`data-elements`** or **`tags`**: DICOM data element dictionary
//! - **`sop`**: DICOM SOP class dictionary
//! 
//! It will automatically retrieve dictionary specifications
//! from a credible source and output the result as a Rust code file
//! or some other supported format.
//! Future versions may enable different kinds of outputs and dictionaries.
//!
//! Please use the `--help` flag for the full usage information.

use clap::{Parser, Subcommand};

mod common;
mod tags;
mod sop;

/// DICOM dictionary builder
#[derive(Debug, Parser)]
struct App {
    #[clap(subcommand)]
    command: BuilderSubcommand,
}

#[derive(Debug, Subcommand)]
enum BuilderSubcommand {
    #[clap(name("data-element"))]
    DataElement(tags::DataElementApp),
    #[clap(name("sop"))]
    SopClass(sop::SopClassApp),
}

fn main() {
    match App::parse() {
        App {
            command: BuilderSubcommand::DataElement(app),
        } => tags::run(app),
        App {
            command: BuilderSubcommand::SopClass(app),
        } => sop::run(app),
    }.unwrap()
}
