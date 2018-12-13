//! This modules contains everything needed to access and manipulate DICOM data elements.
//! It comprises a variety of basic data types, such as the DICOM attribute tag, the
//! element header, and element composite types.
#![recursion_limit="72"]

extern crate byteordered;
extern crate chrono;
extern crate encoding;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate quick_error;
extern crate dicom_core;

pub mod dataset;
pub mod decode;
pub mod encode;
pub mod parser;
pub mod text;
pub mod error;
pub mod transfer_syntax;

mod util;

pub use parser::{DicomParser, Parse, DynamicDicomParser};

#[cfg(test)]
mod tests {

}
