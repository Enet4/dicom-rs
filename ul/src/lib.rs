//! This crates contains the types and methods needed to interact
//! with DICOM nodes through the upper layer protocol.
//!
//! This crate can be used as a base
//! for finite-state machines and higher-level helpers,
//! enabling the creation of concrete service class users (SCUs)
//! and service class providers (SCPs).
//!
//! - The [`address`] module
//!   provides an abstraction for working with compound addresses
//!   referring to application entities in a network.
//! - The [`pdu`] module
//!   provides data structures representing _protocol data units_,
//!   which are passed around as part of the DICOM network communication support.
//! - The [`association`] module
//!   comprises abstractions for establishing and negotiating associations
//!   between application entities,
//!   via the upper layer protocol by TCP.
//!
//! ## Features
//! * `async`: Enables a fully async implementation of the upper layer protocol.
//!   See [`ClientAssociationOptions`] and [`ServerAssociationOptions`] for details

pub mod address;
pub mod association;
pub mod pdu;

/// The current implementation class UID generically referring to DICOM-rs.
///
/// Automatically generated as per the standard, part 5, section B.2.
///
/// This UID may change in future versions,
/// even between patch versions.
pub const IMPLEMENTATION_CLASS_UID: &str = "2.25.156227610253341005307660858504280353500";

/// The current implementation version name generically referring to DICOM-rs.
///
/// This name may change in future versions,
/// even between patch versions.
pub const IMPLEMENTATION_VERSION_NAME: &str = "DICOM-rs 0.8.0";

// re-exports

pub use address::{AeAddr, FullAeAddr};
pub use association::client::{ClientAssociation, ClientAssociationOptions};
pub use association::server::{ServerAssociation, ServerAssociationOptions};
pub use pdu::read_pdu;
pub use pdu::write_pdu;
pub use pdu::Pdu;
