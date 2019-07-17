//! This crate provides interfaces and data structures for reading and writing
//! data in accordance to the DICOM standard, at different layers of
//! abstraction.
//! For the time being, all APIs are based on synchronous I/O.
#![recursion_limit="72"]

pub mod dataset;
pub mod decode;
pub mod encode;
pub mod parser;
pub mod text;
pub mod error;
pub mod transfer_syntax;

mod util;

pub use parser::{DicomParser, Parse, DynamicDicomParser};
