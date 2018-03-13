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
//! ```compile
//! use dicom_core::from_file;
//! # use dicom_core::Result;
//! # fn foo() -> Result<()> {
//! let obj = from_file("0001.dcm")?;
//! let patient_name = obj.get_element_by_name("PatientName")?.as_str()?;
//! let modality = obj.get_element_by_name("Modality")?.as_str()?;
//! let pixel_data = obj.get_pixel_data()?;
//! # Ok(())
//! # }
//! ```

extern crate byteorder;
extern crate chrono;
extern crate encoding;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate quick_error;

pub mod data;
pub mod dictionary;
pub mod error;
pub mod file;
pub mod loader;
pub mod meta;
pub mod object;
pub mod transfer_syntax;

pub use data::value::{Value as DicomValue, PrimitiveValue};
pub use data::VR;
pub use data::DataElement as DicomElement;
pub use dictionary::{DataDictionary, StandardDataDictionary};
pub use object::DicomObject;
pub use error::{Error, Result};

pub use object::mem::InMemDicomObject;

mod util;

type DefaultDicomObject = InMemDicomObject<&'static StandardDataDictionary>;

pub use file::{from_file, from_stream, to_file};
