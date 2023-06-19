//! This crate implements standard dictionaries:
//!
//! - [`data_element`]: Co containing all information about the
//!   DICOM attributes specified in the standard according to DICOM PS3.6,
//!   and it will be used by default in most other abstractions available.
//!   When not using private tags, this dictionary should suffice.
//!
//! Each dictionary is provided as a singleton
//! behind a unit type for efficiency and ease of use.

pub mod tags;
pub mod data_element;

pub use data_element::{StandardDataDictionary, StandardDataDictionaryRegistry};
