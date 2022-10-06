//! This crates contains the types and methods needed to interact
//! with DICOM nodes through the upper layer protocol.
//!
//! This crate can be used as a base
//! for finite-state machines and higher-level helpers,
//! enabling the creation of concrete service class users (SCUs)
//! and service class providers (SCPs).
//!
//! - The [`address`](crate::address) module
//! provides an abstraction for working with compound addresses
//! referring to application entities in a network.
//! - The [`pdu`](crate::pdu) module
//! provides data structures representing _protocol data units_,
//! which are passed around as part of the DICOM network communication support.
//! - The [`association`](crate::association) module
//! comprises abstractions for establishing and negotiating associations
//! between application entities,
//! via the upper layer protocol by TCP.

pub mod association;
pub mod pdu;
pub mod address;

/// The current implementation class UID generically referring to DICOM-rs.
///
/// Automatically generated as per the standard, part 5, section B.2.
///
/// This UID is subject to changes in future versions.
pub const IMPLEMENTATION_CLASS_UID: &str = "2.25.137038125948464847900039011591283709926";

/// The current implementation version name generically referring to DICOM-rs.
///
/// This names is subject to changes in future versions.
pub const IMPLEMENTATION_VERSION_NAME: &str = "DICOM-rs 0.3";

// re-exports

pub use association::client::{ClientAssociation, ClientAssociationOptions};
pub use association::server::{ServerAssociation, ServerAssociationOptions};
pub use pdu::reader::read_pdu;
pub use pdu::writer::write_pdu;
pub use pdu::Pdu;
pub use address::{AeAddr, FullAeAddr};
