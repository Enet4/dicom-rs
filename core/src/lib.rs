#![crate_type = "lib"]
#![deny(missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts, unsafe_code, unstable_features,
        unused_import_braces)]
#![warn(missing_docs, unused_qualifications)]

//! This is a library for basic DICOM content reading and writing.
//! 
//! ## Example
//!
//! The following code does not depict the current functionalities, and the API
//! is subject to change.
//! 
//! ```ignore
//! # extern crate dicom_core;
//! # use dicom_core::Result;
//! # fn foo() -> Result<()> {
//! let obj = try!(DicomLoader::load(File::new("0001.dcm")));
//! let patient_name = try!(try!(obj.element_by_name("PatientName")).as_str());
//! let modality = try!(try!(obj.element_by_name("Modality")).as_str());
//! let pixel_data = try!(obj.pixel_data());
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
pub mod meta;
pub mod object;
pub mod parser;
pub mod transfer_syntax;

pub use data::value::DicomValue;
pub use data::VR;
pub use data::DataElement as DicomElement;
pub use object::DicomObject;
pub use error::{Error, Result};

mod util;
