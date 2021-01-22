//! This crates contains the types and methods needed to interact with the DICOM upper-layer protocol.
//!
//! It is very low level and not very usable as is.
//!
//! Eventually, a finite-state-machine and higher-level SCU/SCP helpers will be added that will make
//! interacting with these types more idiomatic and friendly.
//!
//! The [`pdu`](crate::pdu) module
//! provides data structures representing _protocol data units_,
//! which are passed around as part of the DICOM network communication support.

pub mod pdu;
pub mod association;
