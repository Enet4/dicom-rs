#![crate_type = "lib"]
#![deny(trivial_casts, trivial_numeric_casts, unsafe_code, unstable_features)]
#![warn(missing_debug_implementations, missing_docs, unused_qualifications, unused_import_braces)]

//! This is a library for basic DICOM content reading and writing.
//!
//! ## Example
//!
//! The following code does not depict the current functionalities, and the API
//! is subject to change.
//!
//! ```ignore
//! # use dicom_core::{load_from_path, Result};
//! # fn foo() -> Result<()> {
//! let obj = load_from_path("0001.dcm")?;
//! let patient_name = obj.element_by_name("PatientName")?.as_str()?;
//! let modality = obj.element_by_name("Modality")?.as_str()?;
//! let pixel_data = obj.pixel_data()?;
//! # Ok(())
//! # }
//! ```

#[macro_use]
extern crate lazy_static;
extern crate byteorder;
extern crate encoding;
#[macro_use]
extern crate quick_error;
extern crate chrono;
extern crate itertools;

pub mod data;
pub mod dictionary;
pub mod error;
pub mod iterator;
pub mod loader;
pub mod meta;
pub mod object;
pub mod transfer_syntax;

pub use data::value::DicomValue;
pub use data::VR;
pub use data::DataElement as DicomElement;
pub use dictionary::{DataDictionary, StandardDataDictionary};
pub use object::DicomObject;
pub use error::{Error, Result};

use object::mem::InMemDicomObject;
use std::io::{Read, Seek};
use std::fs::File;
use std::path::Path;

mod util;

type DefaultDicomObject<'s> = InMemDicomObject<'s, &'static StandardDataDictionary>;

pub fn load_from_file<'s, F: 's + Read + Seek>(file: F) -> Result<DefaultDicomObject<'s>> {
    unimplemented!()
}

pub fn load_from_path<'s, P: AsRef<Path>>(path: P) -> Result<DefaultDicomObject<'s>> {
    let file = File::open(path)?;
    load_from_file(file)
}
