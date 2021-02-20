//! This crates contains the types and methods needed to interact
//! with DICOM nodes through the upper layer protocol.
//!
//! This crate can be used as a base
//! for finite-state machines and higher-level helpers,
//! enabling the creation of concrete service class users (SCUs)
//! and service class providers (SCPs).
//!
//! - The [`pdu`](crate::pdu) module
//! provides data structures representing _protocol data units_,
//! which are passed around as part of the DICOM network communication support.
//! - The [`association`](crate::association) module
//! comprises abstractions for establishing and negotiating associations
//! between application entities,
//! via the upper layer protocol by TCP.

pub mod association;
pub mod pdu;

// re-exports

pub use association::client::{ClientAssociation, ClientAssociationOptions};
pub use association::server::{ServerAssociation, ServerAssociationOptions};
pub use pdu::reader::read_pdu;
pub use pdu::writer::write_pdu;
pub use pdu::Pdu;
