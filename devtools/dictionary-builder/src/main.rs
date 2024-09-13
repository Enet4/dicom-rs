//! A simple application that downloads the data dictionary and creates code or
//! data to reproduce it in the core library.
//!
//! ### How to use
//!
//! Run the application with one of the following subcommands:
//!
//! - **`data-element`** or **`tags`**: DICOM data element dictionary
//! - **`uid`** or **`uids`**: DICOM unique identifiers dictionary
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
mod uids;

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
    #[clap(name("uids"))]
    Uid(uids::UidApp),
}

fn main() {
    match App::parse() {
        App {
            command: BuilderSubcommand::DataElement(app),
        } => tags::run(app),
        App {
            command: BuilderSubcommand::Uid(app),
        } => uids::run(app),
    }
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::App;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        App::command().debug_assert();
    }
}
