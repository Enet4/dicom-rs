//! This crate serves as a parent for library crates in the DICOM-rs project.
//!
//! For an idiomatic API to reading and writing DICOM data, please see
//! the [`object`](crate::object) module.
pub use dicom_core as core;
pub use dicom_dictionary_std as dictionary_std;
pub use dicom_encoding as encoding;
pub use dicom_object as object;
pub use dicom_parser as parser;
#[cfg(feature = "pixeldata")]
pub use dicom_pixeldata as pixeldata;
pub use dicom_transfer_syntax_registry as transfer_syntax;
#[cfg(feature = "ul")]
pub use dicom_ul as ul;

// re-export dicom_value macro
pub use dicom_core::dicom_value;
