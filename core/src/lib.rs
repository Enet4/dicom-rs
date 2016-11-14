#![crate_type = "lib"]
#![deny(missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts, unsafe_code, unstable_features,
        unused_import_braces)]
#![warn(missing_docs, unused_qualifications)]

//! This is a base library for dealing with DICOM information and communication.
//!
//! Sorry, no example yet!
//!

#[macro_use]
extern crate lazy_static;
extern crate byteorder;
extern crate encoding;
#[macro_use]
extern crate quick_error;
extern crate chrono;
extern crate itertools;

pub mod attribute;
pub mod error;
pub mod parser;
pub mod transfer_syntax;
pub mod data_element;
pub mod meta;
pub mod object;

pub use attribute::value::DicomValue;
pub use object::DicomObject;
pub use object::LazyDicomObject;
pub use parser::DicomParser;

mod util;
