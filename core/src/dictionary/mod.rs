//! This module contains the concept of a DICOM data dictionary.
//!
//! The standard data dictionary is available in the [`dicom-dictionary-std`] crate.

mod data_element;
pub mod stub;
mod uid;

pub use data_element::{
    DataDictionary, DataDictionaryEntry, DataDictionaryEntryBuf, DataDictionaryEntryRef, TagByName,
    TagRange,
};

pub use uid::{UidDictionaryEntry, UidDictionaryEntryRef, UidType};
