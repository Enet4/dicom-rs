//! This crates contains the types and methods needed to interact with the DICOM upper-layer protocol.
//!
//! It is very low level and not usable as is.
//!
//! Eventually, a finite-state-machine and higher-level SCU/SCP helpers will be added that will make
//! interacting with these types more idiomatic and friendly.

#[cfg(test)]
#[macro_use]
extern crate matches;

pub mod error;
pub mod pdu;
