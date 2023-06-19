//! This crate implements standard DICOM dictionaries and constants.
//!
//! ## Run-time dictinaries
//!
//! The following modules provide definitions for dictionaries
//! which can be queried during a program's lifetime:
//!  
//! - [`data_element`]: Contains all information about the
//!   DICOM attributes specified in the standard according to ,
//!   and it will be used by default in most other abstractions available.
//!   When not using private tags, this dictionary should suffice.
//! - `sop_class` (requires Cargo feature **sop-class**):
//!   Contains information about DICOM Service-Object Pair (SOP) classes
//!   and their respective unique identifiers,
//!   according to [DICOM PS3.6].
//! 
//! The records in these dictionaries are typically collected
//! from [DICOM PS3.6] directly,
//! but they may be obtained through other sources.
//! Each dictionary is provided as a singleton
//! behind a unit type for efficiency and ease of use.
//!
//! [DICOM PS3.6]: https://dicom.nema.org/medical/dicom/current/output/chtml/part06/ps3.6.html
//!
//! ## Constants
//! 
//! The following modules contain constant declarations,
//! which perform an equivalent mapping at compile time,
//! thus without incurring a look-up cost:
//!
//! - [`tags`], which map an attribute alias to a DICOM tag
//! - [`uids`], for various normative DICOM unique identifiers
pub mod data_element;

#[cfg(feature = "sop-class")]
pub mod sop_class;
pub mod tags;
pub mod uids;

pub use data_element::{StandardDataDictionary, StandardDataDictionaryRegistry};
#[cfg(feature = "sop-class")]
pub use sop_class::StandardSopClassDictionary;
