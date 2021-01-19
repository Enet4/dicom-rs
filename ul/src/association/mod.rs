//! DICOM association module
//!
//! This module contains utilities for establishing associations
//! between DICOM nodes via TCP/IP.

use std::net::TcpStream;

use snafu::{ResultExt, Snafu};

use crate::pdu::{reader::read_pdu, writer::write_pdu, Pdu};

pub mod scp;
pub mod scu;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// failed to send PDU message
    Send { source: crate::pdu::writer::Error },

    /// failed to receive PDU message
    Receive { source: crate::pdu::reader::Error },

    #[snafu(display("unexpected response `{:?}`", pdu))]
    #[non_exhaustive]
    UnexpectedResponse {
        /// the PDU obtained from the other node
        pdu: Pdu,
    },

    #[snafu(display("unknown response  `{:?}`", pdu))]
    #[non_exhaustive]
    UnknownResponse {
        /// the PDU obtained from the other node, of variant Unknown
        pdu: Pdu,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A service class user or a provider.
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
pub enum ServiceClassRole {
    /// Service Class User
    Scu,
    /// Service Class Provider
    Scp,
}

#[derive(Debug)]
pub struct Association {
    service_class_type: ServiceClassRole,
    /// The accorded abstract syntax UID
    abstract_syntax_uid: String,
    /// The accorded transfer syntax UID
    transfer_syntax_uid: String,
    /// The identifier of the accorded presentation context
    presentation_context_id: u8,
    /// The maximum PDU length
    max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: TcpStream,
}

impl Association {
    /// Retrieve the identifier of the negotiated presentation context.
    pub fn presentation_context_id(&self) -> u8 {
        self.presentation_context_id
    }

    /// Retrieve the negotiated abstract syntax UID.
    pub fn abstract_syntax_uid(&self) -> &str {
        &self.abstract_syntax_uid
    }

    /// Retrieve the negotiated transfer syntax UID.
    pub fn transfer_syntax_uid(&self) -> &str {
        &self.transfer_syntax_uid
    }

    /// Send a PDU message to the other intervenient.
    pub fn send(&mut self, msg: &Pdu) -> Result<()> {
        write_pdu(&mut self.socket, &msg).context(Send)
    }

    /// Read a PDU message from the other intervenient.
    pub fn receive(&mut self) -> Result<Pdu> {
        read_pdu(&mut self.socket, self.max_pdu_length).context(Receive)
    }

    /// Gracefully release the association.
    pub fn release(&mut self) -> Result<()> {
        write_pdu(&mut self.socket, &Pdu::ReleaseRQ).context(Send)?;

        let pdu = read_pdu(&mut self.socket, self.max_pdu_length).context(Receive)?;

        match pdu {
            Pdu::ReleaseRP => {}
            pdu @ Pdu::AbortRQ { .. }
            | pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::AssociationRQ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRQ { .. } => return UnexpectedResponse { pdu }.fail(),
            pdu @ Pdu::Unknown { .. } => return UnknownResponse { pdu }.fail(),
        }

        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        Ok(())
    }
}

impl Drop for Association {
    fn drop(&mut self) {
        if self.service_class_type == ServiceClassRole::Scu {
            let _ = self.release();
        }
    }
}
