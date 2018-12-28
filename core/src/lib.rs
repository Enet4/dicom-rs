#![crate_type = "lib"]
#![deny(
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features
)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    unused_qualifications,
    unused_import_braces
)]
#![recursion_limit = "60"]

//! This is the core DICOM library, containing various concepts, data structures
//! and traits specific to DICOM content.
//!

extern crate chrono;
extern crate itertools;
#[macro_use]
extern crate quick_error;
extern crate smallvec;

pub mod dictionary;
pub mod error;
pub mod header;
pub mod value;

pub use dictionary::DataDictionary;
pub use error::{Error, Result};
pub use header::{DataElement, DataElementHeader, Length, Tag, VR};
pub use value::{PrimitiveValue, Value as DicomValue};

mod util;
