//! This module contains the concept of a DICOM data dictionary.
//!
//! The standard data dictionary documentation is available in the latest
//! version of
//! [`dicom-dictionary-std`](https://docs.rs/dicom-dictionary-std).
//!
//! **Note:** The link above always points to the latest release.
//! For older releases, select the appropriate documentation version tag.

mod data_element;
pub mod stub;
mod uid;

pub use data_element::{
    DataDictionary, DataDictionaryEntry, DataDictionaryEntryBuf, DataDictionaryEntryRef, TagByName,
    TagRange, VirtualVr,
};

pub use uid::{UidDictionary, UidDictionaryEntry, UidDictionaryEntryRef, UidType};
