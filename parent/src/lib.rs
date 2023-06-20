//! # DICOM-rs library
//! 
//! This crate serves as a parent for library crates in the DICOM-rs project.
//! 
//! This library aggregates the key modules
//! 
//! that you are likely to require when building DICOM compliant systems.
//! These modules are also available as crates
//! which can be fetched independently,
//! in complement or as an alternative to using the `dicom` crate.
//! When adding a new dependency in the DICOM-rs umbrella,
//! they will generally have the `dicom-` prefix.
//! For instance, the module `object`
//! lives in the crate named [`dicom-object`][1].
//! 
//! [1]: https://docs.rs/dicom-object
//! 
//! ## Basic 
//! 
//! - For an idiomatic API to reading and writing DICOM data
//!   from files or other sources,
//!   see the [`object`] module.
//! - To print human readable summaries of a DICOM object,
//!   see the [`dump`] module.
//! - The [`pixeldata`] module helps you convert pixel data
//!   into images or multi-dimensional arrays.
//! - The [`core`] crate contains most of the data types
//!   that the other crates rely on,
//!   including types for DICOM Tags ([`Tag`](dicom_core::Tag)),
//!   value representations ([`VR`](dicom_core::VR)),
//!   and in-memory representations of [DICOM values](dicom_core::DicomValue),
//!   contained in [data elements](dicom_core::DataElement).
//!   For convenience, the [`dicom_value!`] macro
//!   has been re-exported here as well.
//! - The DICOM standard data dictionary is in [`dictionary_std`],
//!   which not only provides a singleton to a standard DICOM tag index
//!   that can be queried at run-time,
//!   it also provides constants for known tags
//!   in the [`tags`][dictionary_std::tags] module.
//! - In the event that you need to get
//!   the global registry of known transfer syntaxes,
//!   [`transfer_syntax`] a re-export of the `dicom-transfer-syntax-registry` crate.
//!   Moreover, [inventory-based transfer syntax registry][ts]
//!   is enabled by default
//!   (see the link for more information).
//!
//! [ts]: dicom_encoding::transfer_syntax
//!
//! ## Advanced
//!
//! - To write DICOM network application entity software,
//!   see the [`ul`] module for PDU reading/writing
//!   and a DICOM association API.
//! - If you are writing or declaring your own transfer syntax,
//!   you will need to take the [`encoding`] module
//!   and build your own [`TransferSyntax`](encoding::TransferSyntax) implementation.
//! - [`parser`] contains the mid-level abstractions for
//!   reading and writing DICOM data sets.
//!   It might only be truly needed if
//!   the `object` API is unfit or too inefficient for a certain task.
//! 

pub use dicom_core as core;
pub use dicom_dictionary_std as dictionary_std;
pub use dicom_dump as dump;
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
