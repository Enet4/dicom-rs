#![crate_type = "lib"]
#![deny(trivial_casts, trivial_numeric_casts, unsafe_code, unstable_features)]
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

pub mod dictionary;
pub mod error;
pub mod header;
pub mod value;

pub use dictionary::DataDictionary;
pub use error::{Error, Result};
pub use header::{DataElement, DataElementHeader, Length, Tag, VR};
pub use value::{PrimitiveValue, Value as DicomValue};
pub use util::ReadSeek;

// re-export the chrono crate, as used in the public API
pub use chrono;

mod util;
