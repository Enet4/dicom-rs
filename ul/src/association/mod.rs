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
//!
//! [1]: std::net::TcpStream
#[cfg(test)]
mod tests;
pub mod client;
pub mod server;

mod uid;

pub(crate) mod pdata;

use std::{backtrace::Backtrace, io::{BufRead, BufReader, Cursor, Read}};

use bytes::{Buf, BytesMut};
pub use client::{ClientAssociation, ClientAssociationOptions};
#[cfg(feature = "async")]
pub use pdata::non_blocking::AsyncPDataWriter;
pub use pdata::{PDataReader, PDataWriter};
pub use server::{ServerAssociation, ServerAssociationOptions};
use snafu::{ensure, Snafu, ResultExt};

use crate::{pdu::{AssociationRJ, PresentationContextResult, ReadPduSnafu, UserVariableItem, self}, Pdu};

pub(crate) struct NegotiatedOptions{
    peer_max_pdu_length: u32,
    user_variables: Vec<UserVariableItem>,
    presentation_contexts: Vec<PresentationContextResult>,
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// missing abstract syntax to begin negotiation
    MissingAbstractSyntax { backtrace: Backtrace },

    /// could not convert to sockeDUt address
    ToAddress {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// could not connect to server
    Connect {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// Could not set tcp read timeout
    SetReadTimeout {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// Could not set tcp write timeout
    SetWriteTimeout {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// failed to send association request
    #[snafu(display("failed to send pdu: {}", source))]
    SendPdu{
        #[snafu(backtrace)]
        source: crate::pdu::WriteError,
    },

    /// failed to receive association response
    #[snafu(display("failed to receive pdu: {}", source))]
    ReceivePdu {
        #[snafu(backtrace)]
        source: crate::pdu::ReadError,
    },

    #[snafu(display("unexpected response from peer `{:?}`", pdu))]
    #[non_exhaustive]
    UnexpectedPdu {
        /// the PDU obtained from the server
        pdu: Box<Pdu>,
    },

    #[snafu(display("unknown response from peer `{:?}`", pdu))]
    #[non_exhaustive]
    UnknownPdu {
        /// the PDU obtained from the server, of variant Unknown
        pdu: Box<Pdu>,
    },

    #[snafu(display("protocol version mismatch: expected {}, got {}", expected, got))]
    ProtocolVersionMismatch {
        expected: u16,
        got: u16,
        backtrace: Backtrace,
    },

    // Association rejected by the server
    #[snafu(display("association rejected {}", association_rj.source))]
    Rejected {
        association_rj: AssociationRJ,
        backtrace: Backtrace,
    },

    /// association aborted
    Aborted { backtrace: Backtrace },

    /// no presentation contexts accepted by the server
    NoAcceptedPresentationContexts { backtrace: Backtrace },

    /// failed to send PDU message on wire
    #[non_exhaustive]
    WireSend {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// failed to read PDU message from wire
    #[non_exhaustive]
    WireRead {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// Operation timed out
    #[non_exhaustive]
    Timeout {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "PDU is too large ({} bytes) to be sent to the remote application entity",
        length
    ))]
    #[non_exhaustive]
    SendTooLongPdu { length: usize, backtrace: Backtrace },

    #[snafu(display("Connection closed by peer"))]
    ConnectionClosed,
}

/// Helper function to get a PDU from a reader.
///
/// Chunks of data are read into `read_buffer`,
/// which should be passed in subsequent calls
/// to receive more PDUs from the same stream.
pub fn read_pdu_from_wire<R>(
    reader: &mut R,
    read_buffer: &mut BytesMut,
    max_pdu_length: u32,
    strict: bool,
) -> Result<Pdu>
where
    R: Read,
{
    let mut reader = BufReader::new(reader);
    let msg = loop {
        let mut buf = Cursor::new(&read_buffer[..]);
        // try to read a PDU according to what's in the buffer
        match pdu::read_pdu(&mut buf, max_pdu_length, strict).context(ReceivePduSnafu)? {
            Some(pdu) => {
                read_buffer.advance(buf.position() as usize);
                break pdu;
            }
            None => {
                // Reset position
                buf.set_position(0)
            }
        }
        // Use BufReader to get similar behavior to AsyncRead read_buf
        let recv = reader
            .fill_buf()
            .context(ReadPduSnafu)
            .context(ReceivePduSnafu)?;
        let bytes_read = recv.len();
        read_buffer.extend_from_slice(recv);
        reader.consume(bytes_read);
        ensure!(bytes_read != 0, ConnectionClosedSnafu);
    };
    Ok(msg)
}

#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt};

/// Helper function to get a PDU from an async reader.
/// 
/// Chunks of data are read into `read_buffer`,
/// which should be passed in subsequent calls
/// to receive more PDUs from the same stream.
#[cfg(feature = "async")]
pub async fn read_pdu_from_wire_async<R: AsyncRead + Unpin>(
    reader: &mut R,
    read_buffer: &mut BytesMut,
    max_pdu_length: u32,
    strict: bool,
) -> Result<Pdu> {
    // receive response

    let msg = loop {
        let mut buf = Cursor::new(&read_buffer[..]);
        match pdu::read_pdu(&mut buf, max_pdu_length, strict).context(ReceivePduSnafu)? {
            Some(pdu) => {
                read_buffer.advance(buf.position() as usize);
                break pdu;
            }
            None => {
                // Reset position
                buf.set_position(0)
            }
        }
        let recv = reader
            .read_buf(read_buffer)
            .await
            .context(ReadPduSnafu)
            .context(ReceivePduSnafu)?;
        ensure!(recv > 0, ConnectionClosedSnafu);
    };
    Ok(msg)
}