//! This crate provides interfaces and data structures for reading and writing
//! data in accordance to the DICOM standard, at different layers of
//! abstraction.
//! For the time being, all APIs are based on synchronous I/O.
#![recursion_limit="72"]

extern crate byteordered;
extern crate chrono;
extern crate encoding;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate smallvec;
extern crate dicom_core;
extern crate dicom_dictionary_std;

pub mod dataset;
pub mod decode;
pub mod encode;
pub mod parser;
pub mod text;
pub mod error;
pub mod transfer_syntax;

mod util;

pub use parser::{DicomParser, Parse, DynamicDicomParser};
