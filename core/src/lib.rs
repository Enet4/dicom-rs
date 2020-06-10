#![crate_type = "lib"]
#![deny(trivial_numeric_casts, unsafe_code, unstable_features)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    unused_qualifications,
    unused_import_braces
)]
#![recursion_limit = "60"]

//! This is the core library of DICOM-rs containing various concepts,
//! data structures and traits specific to DICOM content.
//!
//! The current structure of this crate is as follows:
//!
//! - [`header`] comprises various data types for DICOM element header,
//!   including common definitions for DICOM tags and value representations.
//! - [`dictionary`] describes common behavior of DICOM data dictionaries,
//!   which translate attribute names and/or tags to a dictionary entry
//!   containing relevant information about the attribute.
//! - [`value`] holds definitions for values in standard DICOM elements,
//!   with the awareness of multiplicity, representation,
//!   and the possible presence of sequences.
//! - [`error`] contains crate-level error and result types.
//! 
//! [`dictionary`]: ./dictionary/index.html
//! [`error`]: ./error/index.html
//! [`header`]: ./header/index.html
//! [`value`]: ./value/index.html

pub mod dictionary;
pub mod error;
pub mod header;
pub mod value;

pub use dictionary::DataDictionary;
pub use error::{Error, Result};
pub use header::{DataElement, DataElementHeader, Length, Tag, VR};
pub use value::{PrimitiveValue, Value as DicomValue};

// re-export crates that are part of the public API
pub use chrono;
pub use smallvec;

mod util;
