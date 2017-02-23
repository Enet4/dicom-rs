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
//! let obj = DicomLoader::load(File::new("0001.dcm")).unwrap();
//! let patient_name = obj.element("PatientName").as_str().unwrap();
//! let modality = obj.element("Modality").as_str().unwrap();
//! let pixel_data = obj.pixel_data().unwrap();
//! ```

#[macro_use]
extern crate lazy_static;
extern crate byteorder;
extern crate encoding;
#[macro_use]
extern crate quick_error;
extern crate chrono;
extern crate itertools;

pub mod attribute;
pub mod data;
pub mod error;
pub mod iterator;
pub mod meta;
pub mod object;
pub mod parser;
pub mod transfer_syntax;

pub use attribute::value::DicomValue;
pub use attribute::VR;
pub use data::DataElement as DicomElement;
pub use object::DicomObject;
pub use object::LazyDicomObject;
pub use parser::DicomParser;

mod util;
