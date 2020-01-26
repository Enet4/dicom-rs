//! This crate works on top of DICOM encoding primitives to provide transfer
//! syntax resolution and abstraction for parsing DICOM data sets, which
//! ultimately enables the user to perceive the DICOM object as a sequence of
//! tokens.
//!
//! For the time being, all APIs are based on synchronous I/O.
//!
//! For a more intuitive, object-oriented API, please see the `dicom-object`
//! crate.
#![recursion_limit = "80"]

pub mod dataset;
pub mod error;
pub mod stateful;

mod util;

pub use dataset::DataSetReader;
pub use stateful::decode::{DynStatefulDecoder, StatefulDecode, StatefulDecoder};
pub use stateful::encode::StatefulEncoder;
