//! This crate serves as a parent for library crates in the DICOM-rs project.
//! 
//! For an idiomatic API to reading and writing DICOM data, please see
//! [`dicom_object`](../dicom_object).
pub use dicom_core as core;
pub use dicom_dictionary_std as dictionary_std;
pub use dicom_parser as parser;
pub use dicom_object as object;
