//! DICOM association module
//!
//! This module contains utilities for establishing associations
//! between DICOM nodes via TCP/IP.
//!
//! As an association requester, often as a service class user (SCU),
//! a new association can be started
//! via the [`ClientAssociationOptions`] type.
//! The minimum required properties are the accepted abstract syntaxes
//! and the TCP socket address to the target node.
//!
//! As an association acceptor,
//! usually taking the role of a service class provider (SCP),
//! a newly created [TCP stream][1] can be passed to
//! a previously prepared [`ServerAssociationOptions`].
//!
//! [1]: std::net::TcpStream
pub mod client;
pub mod server;
mod uid;

pub(crate) mod pdata;

pub use client::{ClientAssociation, ClientAssociationOptions};
#[cfg(feature = "async")]
pub use pdata::AsyncPDataWriter as PDataWriter;
pub use pdata::PDataReader;
#[cfg(not(feature = "async"))]
pub use pdata::PDataWriter;
pub use server::{ServerAssociation, ServerAssociationOptions};
