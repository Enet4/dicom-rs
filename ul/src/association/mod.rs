//! DICOM association module
//!
//! This module contains utilities for establishing associations
//! between DICOM nodes via TCP/IP.

use std::net::TcpStream;

use snafu::{Snafu, ResultExt};

use crate::pdu::{Pdu, reader::read_pdu, writer::write_pdu};

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



#[derive(Debug)]
pub struct Association {
    /// The accorded abstract syntax UID
    abstract_syntax_uid: String,
    /// The accorded transfer syntax UID
    transfer_syntax_uid: String,
    /// The maximum PDU length
    max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: TcpStream,
}

impl Association {
    /// Send a PDU message to the other intervenient.
    pub fn send(&mut self, msg: &Pdu) -> Result<()> {
        write_pdu(&mut self.socket, &msg).context(Send)
    }

    /// Read a PDU message from the other intervenient.
    pub fn receive(&mut self) -> Result<Pdu> {
        read_pdu(&mut self.socket, self.max_pdu_length).context(Receive)
    }

}
