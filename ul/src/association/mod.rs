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
pub mod client;
pub mod server;
#[cfg(test)]
mod tests;

mod uid;

pub(crate) mod pdata;

use std::{
    backtrace::Backtrace,
    io::{BufRead, BufReader, Cursor, Read, Write},
    time::{Duration, Instant},
};

use bytes::{Buf, BytesMut};
#[cfg(feature = "async")]
pub use client::AsyncClientAssociation;
pub use client::{ClientAssociation, ClientAssociationOptions};
#[cfg(feature = "async")]
pub use pdata::non_blocking::AsyncPDataWriter;
pub use pdata::{PDataReader, PDataWriter};
#[cfg(feature = "async")]
pub use server::AsyncServerAssociation;
pub use server::{ServerAssociation, ServerAssociationOptions};
use snafu::{ResultExt, Snafu, ensure};

use crate::{
    Pdu,
    pdu::{
        self, AbortRQServiceProviderReason, AbortRQSource, AssociationRJ,
        PresentationContextNegotiated, ReadPduSnafu, RequestorRoles, UserVariableItem,
    },
    write_pdu,
};

/// Default timeout in seconds to wait for the peer to close the connection
pub const DEFAULT_FINALIZATION_TIMEOUT: f32 = 30.0;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// missing abstract syntax to begin negotiation
    MissingAbstractSyntax { backtrace: Backtrace },

    /// could not convert to socket address
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
    #[snafu(display("failed to send pdu"))]
    SendPdu {
        #[snafu(backtrace)]
        source: crate::pdu::WriteError,
    },

    /// failed to receive association response
    #[snafu(display("failed to receive pdu"))]
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

    #[snafu(display("failed close connection: {}", source))]
    Close {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "PDU is too large ({} bytes) to be sent to the remote application entity",
        length
    ))]
    #[non_exhaustive]
    SendTooLongPdu { length: usize, backtrace: Backtrace },

    #[snafu(display("connection closed by peer"))]
    ConnectionClosed { backtrace: Backtrace },

    /// TLS configuration is missing
    #[cfg(feature = "sync-tls")]
    #[snafu(display("TLS configuration is required but not provided"))]
    TlsConfigMissing { backtrace: Backtrace },

    /// Invalid server name for TLS
    #[cfg(feature = "sync-tls")]
    #[snafu(display("invalid server name for TLS connection"))]
    InvalidServerName {
        source: rustls::pki_types::InvalidDnsNameError,
        backtrace: Backtrace,
    },

    /// Failed to establish TLS connection
    #[cfg(feature = "sync-tls")]
    #[snafu(display("failed to establish TLS connection, does the remote support TLS?"))]
    TlsConnection {
        source: rustls::Error,
        backtrace: Backtrace,
    },

    /// Failed to establish TLS connection
    #[cfg(any(feature = "sync-tls", feature = "async-tls"))]
    #[snafu(display("failed to handshake TLS connection, does the remote support TLS?"))]
    TlsHandshake {
        source: std::io::Error,
        backtrace: Backtrace,
    },
    #[cfg(any(feature = "sync-tls", feature = "async-tls"))]
    #[snafu(display("TLS not enabled, but peer seems to be sending TLS data"))]
    TlsNotSupported,
}
/// Struct to hold negotiated options after association is accepted
pub(crate) struct NegotiatedOptions {
    /// Maximum PDU length the peer can handle
    peer_max_pdu_length: u32,
    /// User variables accepted by the peer
    user_variables: Vec<UserVariableItem>,
    /// Presentation contexts accepted by the peer
    presentation_contexts: Vec<PresentationContextNegotiated>,
    /// The peer's AE title
    peer_ae_title: String,
}

/// Socket configuration for associations
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SocketOptions {
    /// Timeout for individual read operations
    read_timeout: Option<Duration>,
    /// Timeout for individual send operations
    write_timeout: Option<Duration>,
    /// Timeout for connection establishment
    connection_timeout: Option<Duration>,
}

/// Trait to close underlying socket
pub trait CloseSocket {
    fn close(&mut self) -> std::io::Result<()>;
}

impl CloseSocket for std::net::TcpStream {
    fn close(&mut self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Both)
    }
}

#[cfg(feature = "sync-tls")]
impl CloseSocket for rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream> {
    fn close(&mut self) -> std::io::Result<()> {
        // The peer may have already disconnected. On linux, calling `shutdown` once the peer
        // disconnects is fine, but on Mac it returns a `NotConnected` error, ignore that
        match self.get_mut().shutdown(std::net::Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotConnected => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "sync-tls")]
impl CloseSocket for rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream> {
    fn close(&mut self) -> std::io::Result<()> {
        // The peer may have already disconnected. On linux, calling `shutdown` once the peer
        // disconnects is fine, but on Mac it returns a `NotConnected` error, ignore that
        match self.get_mut().shutdown(std::net::Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotConnected => Ok(()),
            Err(e) => Err(e),
        }
    }
}

pub trait SetReadTimeout {
    fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()>;
}

impl SetReadTimeout for std::net::TcpStream {
    fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        std::net::TcpStream::set_read_timeout(self, dur)
    }
}

#[cfg(feature = "sync-tls")]
impl<C> SetReadTimeout for rustls::StreamOwned<C, std::net::TcpStream> {
    fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        self.sock.set_read_timeout(dur)
    }
}

/// Trait that represents common properties of an association
pub trait Association {
    /// Obtain the remote DICOM node's application entity title.
    fn peer_ae_title(&self) -> &str;

    /// Retrieve the maximum PDU length
    /// admitted by the association acceptor.
    fn acceptor_max_pdu_length(&self) -> u32;

    /// Retrieve the maximum PDU length
    /// admitted by the association requestor.
    fn requestor_max_pdu_length(&self) -> u32;

    /// Retrieve the maximum PDU length
    /// that this application entity is expecting to receive.
    /// That's the same as acceptor_max_pdu_length() for
    /// server objects, and as requestor_max_pdu_length()
    /// for client objects.
    ///
    /// The current implementation is not required to fail
    /// and/or abort the association
    /// if a larger PDU is received.
    fn local_max_pdu_length(&self) -> u32;

    /// Retrieve the maximum PDU length
    /// admitted by the peer.
    /// That's the same as requestor_max_pdu_length() for
    /// server objects, and as acceptor_max_pdu_length()
    /// for client objects.
    fn peer_max_pdu_length(&self) -> u32;

    /// Retrieve the association finalization timeout
    /// defined for this association
    fn finalization_timeout(&self) -> Duration;

    /// Change the association finalization timeout
    /// defined for this association
    fn set_finalization_timeout(&mut self, timeout: Duration);

    /// Obtain a view of the negotiated presentation contexts.
    fn presentation_contexts(&self) -> &[PresentationContextNegotiated];

    /// Retrieve the user variables that were taken from the server.
    ///
    /// It usually contains the maximum PDU length,
    /// the implementation class UID, and the implementation version name.
    fn user_variables(&self) -> &[UserVariableItem];

    /// Retrieve the bytes associated to a specific SOP Class
    /// that was negotiated with extended negotiation.
    ///
    /// Returns `None` if that SOP class was rejected or not requested.
    fn extended_negotiation_for(&self, sop_class_uid: &str) -> Option<&[u8]> {
        self.user_variables().iter().find_map(|uv| match uv {
            UserVariableItem::SopClassExtendedNegotiationSubItem(uid, data)
                if uid == sop_class_uid =>
            {
                Some(data.as_slice())
            }
            _ => None,
        })
    }

    /// Roles that the Association-requestor may assume. Returns a
    /// `RequestorRoles` struct. If the given SOP Class UID was not
    /// returned by the server, the default values are returned.
    fn requestor_roles_for(&self, sop_class_uid: &str) -> RequestorRoles {
        self.user_variables()
            .iter()
            .find_map(|uv| match uv {
                UserVariableItem::ScuScpRoleSelectionSubItem(uid, roles)
                    if uid == sop_class_uid =>
                {
                    Some(*roles)
                }
                _ => None,
            })
            // PS3.7 D.3.3.4:
            // If the SCP/SCU Role Selection item is not returned by the
            // Association-acceptor then the role of the Association-requestor
            // shall be SCU and the role of the Association-acceptor shall be SCP.
            // These apply to the requestor only, so only SCU is set.
            .unwrap_or(RequestorRoles {
                scu: true,
                scp: false,
            })
    }
}

/// Trait that represents methods that can be made on a synchronous association.
pub trait SyncAssociation<S: Read + Write + CloseSocket + SetReadTimeout>: Association {
    /// Obtain access to the inner stream
    /// connected to the association acceptor.
    ///
    /// This can be used to send the PDU in semantic fragments of the message,
    /// thus using less memory.
    ///
    /// **Note:** reading and writing should be done with care
    /// to avoid inconsistencies in the association state.
    /// Do not call `send` and `receive` while not in a PDU boundary.
    fn inner_stream(&mut self) -> &mut S;

    /// Obtain mutable access to the inner stream, read and write buffers
    fn get_mut(&mut self) -> (&mut S, &mut BytesMut, &mut Vec<u8>);

    /// Send a PDU message to the other intervenient.
    ///
    /// In the DIMSE Association State Machine, this method
    /// must be entered in state Sta6 (ready to send/receive data).
    /// If the return value is `Ok`, the new state is still Sta6;
    /// if it is `Err`, the new state is Sta1 (no connection), and
    /// the association object should be considered no longer valid.
    // FIXME: Will that be the case, or only for certain Err values?
    //        e.g. a SendTooLongPdu error should probably not close
    //        the connection.
    // TODO: Actually implement state machine handling.
    fn send(&mut self, pdu: &Pdu) -> Result<()>;

    /// Read a PDU message from the other intervenient.
    ///
    /// In the DIMSE Association State Machine, this method
    /// must be entered in state Sta6 (ready to send/receive data).
    /// If the return value is `Ok`, the new state is still Sta6;
    /// if it is `Err`, the new state is Sta1 (no connection), and
    /// the association object should be considered no longer valid.
    // FIXME: Will that be the case, or only for certain Err values?
    // TODO: Actually implement state machine handling.
    fn receive(&mut self) -> Result<Pdu>;

    /// Send an abort message with a source/reason
    /// and shut down the TCP connection,
    /// terminating the association.
    /// This function may take up to a time defined by
    /// `finalization_timeout()` to complete, depending
    /// on how long the peer takes to close the socket.
    ///
    /// In the DIMSE Association State Machine, this method
    /// must be entered in state Sta6 (ready to send/receive data).
    /// It will return in state Sta1 (no connection) regardless of
    /// the return value, which serves to identify what happened.
    fn abort_with_source(mut self, source: AbortRQSource) -> Result<()>
    where
        Self: Sized,
    {
        let pdu = Pdu::AbortRQ { source };
        let local_max_pdu_length = self.local_max_pdu_length();
        let peer_max_pdu_length = self.peer_max_pdu_length();
        let close_assoc_timeout = self.finalization_timeout();
        let (socket, read_buffer, write_buffer) = self.get_mut();
        write_pdu_to_wire(socket, write_buffer, &pdu, peer_max_pdu_length)?;
        sta13(
            socket,
            read_buffer,
            write_buffer,
            local_max_pdu_length,
            peer_max_pdu_length,
            close_assoc_timeout,
        )
    }

    /// Send a user-initiated abort message
    /// and shut down the TCP connection,
    /// terminating the association.
    /// This function may take up to a time defined by
    /// `finalization_timeout()` to complete, depending
    /// on how long the peer takes to close the socket.
    ///
    /// In the DIMSE Association State Machine, this method
    /// must be entered in state Sta6 (ready to send/receive data).
    /// It will return in state Sta1 (no connection) regardless of
    /// the return value, which serves to identify what happened.
    fn abort(self) -> Result<()>
    where
        Self: Sized,
    {
        self.abort_with_source(AbortRQSource::ServiceUser)
    }

    /// Iniate a graceful release of the association.
    ///
    /// A DIMSE A-RELEASE transaction is initiated by this application entity,
    /// and the underlying socket is closed once settled.
    ///
    /// This function may take an indefinite time to continue, if receive
    /// timeout is not defined and the peer does not respond.
    ///
    /// Note that as of version 0.9.1,
    /// implementers of this trait no longer call this method on [`Drop`],
    /// so remember to call `release` explicitly
    /// at the end of all DIMSE transactions.
    ///
    /// This function may take an indefinite time to continue, if receive
    /// timeout is not defined and the peer does not respond.
    ///
    /// In the DIMSE Association State Machine, this method
    /// must be entered in state Sta6 (ready to send/receive data).
    /// It will return in state Sta1 (no connection) regardless of
    /// the return value, which serves to identify what happened.
    fn release(self) -> Result<()>
    where
        Self: Sized;

    /// Prepare a P-Data writer for sending
    /// one or more data item PDUs.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    fn send_pdata(&mut self, presentation_context_id: u8) -> PDataWriter<&mut S> {
        let max_pdu_length = self.peer_max_pdu_length();
        PDataWriter::new(self.inner_stream(), presentation_context_id, max_pdu_length)
    }

    /// Prepare a P-Data reader for receiving
    /// one or more data item PDUs.
    ///
    /// Returns a reader which automatically
    /// receives more data PDUs once the bytes collected are consumed.
    fn receive_pdata(&mut self) -> PDataReader<'_, &mut S> {
        let max_pdu_length = self.local_max_pdu_length();
        let (socket, read_buffer, _) = self.get_mut();
        PDataReader::new(socket, max_pdu_length, read_buffer)
    }

    fn close(&mut self) -> std::io::Result<()>;
}

#[cfg(feature = "async")]
/// Trait that represents methods that can be made on an asynchronous association.
pub trait AsyncAssociation<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send>:
    Association
{
    /// Obtain access to the inner stream
    /// connected to the association acceptor.
    /// The value will be `None` when the stream was
    /// deliberately closed by this side of the wire,
    /// for example in response to an A-ABORT PDU.
    ///
    /// This can be used to send the PDU in semantic fragments of the message,
    /// thus using less memory.
    ///
    /// **Note:** reading and writing should be done with care
    /// to avoid inconsistencies in the association state.
    /// Do not call `send` and `receive` while not in a PDU boundary.
    fn inner_stream(&mut self) -> &mut Option<S>;

    /// Obtain mutable access to the inner stream, read and write buffers
    fn get_mut(&mut self) -> (&mut Option<S>, &mut BytesMut, &mut Vec<u8>);

    /// Send a PDU message to the other intervenient.
    fn send(&mut self, pdu: &Pdu) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send;

    /// Read a PDU message from the other intervenient.
    fn receive(&mut self) -> impl std::future::Future<Output = Result<Pdu>> + Send
    where
        Self: Send;

    /// Send an abort message with a source/reason
    /// and shut down the TCP connection,
    /// terminating the association.
    /// This function may take up to a time defined by
    /// `finalization_timeout()` to complete, depending
    /// on how long the peer takes to close the socket.
    ///
    /// In the DIMSE Association State Machine, this method
    /// must be entered in state Sta6 (ready to send/receive data).
    /// It will return in state Sta1 (no connection) regardless of
    /// the return value, which serves to identify what happened.
    fn abort_with_source(
        mut self,
        source: AbortRQSource,
    ) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Sized + Send,
    {
        async move {
            let pdu = Pdu::AbortRQ { source };
            let local_max_pdu_length = self.local_max_pdu_length();
            let peer_max_pdu_length = self.peer_max_pdu_length();
            let write_timeout = self.write_timeout();
            let close_assoc_timeout = self.finalization_timeout();
            let (socket, read_buffer, write_buffer) = self.get_mut();
            write_pdu_to_wire_async(
                socket.as_mut(),
                write_buffer,
                &pdu,
                peer_max_pdu_length,
                write_timeout,
            )
            .await?;

            sta13_async(
                socket,
                read_buffer,
                write_buffer,
                local_max_pdu_length,
                peer_max_pdu_length,
                close_assoc_timeout,
            )
            .await
        }
    }

    /// Send a user-initiated abort message
    /// and shut down the TCP connection,
    /// terminating the association.
    /// This function may take up to a time defined by
    /// `finalization_timeout()` to complete, depending
    /// on how long the peer takes to close the socket.
    ///
    /// In the DIMSE Association State Machine, this method
    /// must be entered in state Sta6 (ready to send/receive data).
    /// It will return in state Sta1 (no connection) regardless of
    /// the return value, which serves to identify what happened.
    fn abort(self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Sized + Send,
    {
        self.abort_with_source(AbortRQSource::ServiceUser)
    }

    /// Iniate a graceful release of the association.
    ///
    /// A DIMSE A-RELEASE transaction is initiated by this application entity,
    /// and the underlying socket is closed once settled.
    ///
    /// Note that implementers of this trait
    /// do not try to release the association on [`Drop`],
    /// so remember to call `release` explicitly
    /// at the end of all DIMSE transactions.
    fn release(self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Sized + Send;

    /// Prepare a P-Data writer for sending
    /// one or more data item PDUs.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    ///
    /// Panics if used when the socket has been destroyed.
    fn send_pdata(&mut self, presentation_context_id: u8) -> AsyncPDataWriter<&mut S> {
        let max_pdu_length = self.peer_max_pdu_length();
        AsyncPDataWriter::new(
            self.inner_stream().as_mut().expect("Stream was destroyed"),
            presentation_context_id,
            max_pdu_length,
        )
    }

    /// Prepare a P-Data reader for receiving
    /// one or more data item PDUs.
    ///
    /// Returns a reader which automatically
    /// receives more data PDUs once the bytes collected are consumed.
    ///
    /// Panics if used when the socket has been destroyed.
    fn receive_pdata(&mut self) -> PDataReader<'_, &mut S> {
        let max_pdu_length = self.local_max_pdu_length();
        let (socket, read_buffer, _) = self.get_mut();
        PDataReader::new(
            socket.as_mut().expect("Stream was destroyed"),
            max_pdu_length,
            read_buffer,
        )
    }

    fn close(&mut self) {
        drop(self.inner_stream().take());
    }

    /// Returns the timeout that was set for sending data, or None if no
    /// timeout was specified.
    fn write_timeout(&self) -> Option<Duration>;

    /// Returns the timeout that was set for receiving data, or None if no
    /// timeout was specified.
    fn read_timeout(&self) -> Option<Duration>;
}

// Helper function to perform an operation with timeout
#[cfg(feature = "async")]
async fn timeout<T>(
    timeout: Option<Duration>,
    block: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    if let Some(timeout) = timeout {
        tokio::time::timeout(timeout, block)
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))
            .context(TimeoutSnafu)?
    } else {
        block.await
    }
}

/// Encode a PDU into the provided buffer
pub(crate) fn encode_pdu(buffer: &mut Vec<u8>, pdu: &Pdu, peer_max_pdu_length: u32) -> Result<()> {
    write_pdu(buffer, pdu).context(SendPduSnafu)?;
    if buffer.len() > peer_max_pdu_length as usize {
        return SendTooLongPduSnafu {
            length: buffer.len(),
        }
        .fail();
    }
    Ok(())
}

/// Helper function to send a PDU to a writer
pub fn write_pdu_to_wire<W: Write>(
    writer: &mut W,
    write_buffer: &mut Vec<u8>,
    msg: &Pdu,
    max_pdu_length: u32,
) -> Result<()> {
    write_buffer.clear();
    encode_pdu(write_buffer, msg, max_pdu_length + pdu::PDU_HEADER_SIZE)?;
    writer
        .write_all(write_buffer)
        .context(crate::association::WireSendSnafu)
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

/// Helper function to send a PDU to an async writer
#[cfg(feature = "async")]
pub async fn write_pdu_to_wire_async<W: tokio::io::AsyncWrite + Unpin>(
    writer: Option<&mut W>,
    write_buffer: &mut Vec<u8>,
    msg: &Pdu,
    max_pdu_length: u32,
    write_timeout: Option<Duration>,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let Some(writer) = writer else {
        return ConnectionClosedSnafu.fail();
    };
    write_buffer.clear();
    encode_pdu(write_buffer, msg, max_pdu_length + pdu::PDU_HEADER_SIZE)?;
    timeout(write_timeout, async {
        writer.write_all(write_buffer).await.context(WireSendSnafu)
    })
    .await
}

/// Helper function to get a PDU from an async reader.
///
/// Chunks of data are read into `read_buffer`,
/// which should be passed in subsequent calls
/// to receive more PDUs from the same stream.
#[cfg(feature = "async")]
pub async fn read_pdu_from_wire_async<R: tokio::io::AsyncRead + Unpin>(
    reader: Option<&mut R>,
    read_buffer: &mut BytesMut,
    max_pdu_length: u32,
    strict: bool,
) -> Result<Pdu> {
    use tokio::io::AsyncReadExt;
    // receive response

    let Some(reader) = reader else {
        return ConnectionClosedSnafu.fail();
    };

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

/// Helper function to implement the behaviour of the
/// association release for both servers and clients.
/// They differ only very slightly, so this factors
/// both server and client into one function.
///
/// This is the sync version.
pub(crate) fn release_impl<S>(mut assoc: impl SyncAssociation<S>, requestor: bool) -> Result<()>
where
    S: Read + Write + CloseSocket + SetReadTimeout,
{
    // Entered in Sta6
    // Action AR-1: send A-RELEASE-RQ PDU; next state: Sta7
    let max_pdu_peer = assoc.acceptor_max_pdu_length();
    let max_pdu_local = assoc.requestor_max_pdu_length();
    let finalization_timeout = assoc.finalization_timeout();
    let (socket, read_buffer, write_buffer) = assoc.get_mut();
    write_pdu_to_wire(socket, write_buffer, &Pdu::ReleaseRQ, max_pdu_peer)?;
    // Sta7
    loop {
        // Note: without a socket timeout, this function may
        // block indefinitely if the peer does not send anything valid.
        let pdu = read_pdu_from_wire(socket, read_buffer, max_pdu_local, false)?;

        match pdu {
            Pdu::ReleaseRP => {
                // Action AR-3 (indicate successful release) and close
                // connection. Next state is Sta1 (return).
                assoc.close().context(CloseSnafu)?;
                return Ok(());
            }
            Pdu::PData { .. } => {
                // Action AR-6, remain in Sta7
                // We can't send a P-DATA indication because
                // the API does not allow it at this point.
                // So we just ignore the PDU.
            }
            Pdu::ReleaseRQ => {
                // Release collision, action AR-8 (nothing in our case).
                // Here's where client and server differ.
                if requestor {
                    // Next state for a client is Sta9.
                    // We immediately issue an A-RELEASE response primitive,
                    // so action is AR-9 (send A-RELEASE-RP) and next state
                    // is Sta11.
                    write_pdu_to_wire(socket, write_buffer, &Pdu::ReleaseRP, max_pdu_peer)?;
                } else {
                    // Next state for a server is Sta10. We start by receiving.
                }
                // Sta10 or Sta11
                // We had sent an A-RELEASE-RQ which may be replied.
                // We also may receive an A-ABORT which we need to
                // process. Or the peer may receive our A-RELEASE-RP and
                // close the socket. In any case, we need to receive
                // once more.
                let result = read_pdu_from_wire(socket, read_buffer, max_pdu_local, false);
                let pdu = match result {
                    Ok(pdu) => pdu,
                    Err(Error::ConnectionClosed { .. }) => {
                        // The peer closed the connection; action is AA-4
                        // (issue A-ABORT indication). Next state is Sta1.
                        return AbortedSnafu {}.fail();
                    }
                    Err(err) => {
                        return Err(err);
                    }
                };
                match pdu {
                    Pdu::ReleaseRP => {
                        // The peer responded to our release request
                        if requestor {
                            // Sta11: Action AR-3 (close socket and confirm
                            // success). Next state is Sta1.
                            assoc.close().context(CloseSnafu)?;
                            return Ok(());
                        }
                        // We're a server, so we're still in Sta10. Action
                        // is AR-10 (confirm release); next state is Sta12.
                        // From Sta12, we immediately send a release response
                        // primitive, which results in action AR-4 (send
                        // A-RELEASE-RP) and state Sta13.
                        write_pdu_to_wire(socket, write_buffer, &Pdu::ReleaseRP, max_pdu_peer)?;
                        return sta13(
                            socket,
                            read_buffer,
                            write_buffer,
                            max_pdu_local,
                            max_pdu_peer,
                            finalization_timeout,
                        );
                    }
                    Pdu::AbortRQ { .. } => {
                        // Action AA-3, close socket and issue A-ABORT indication.
                        assoc.close().context(CloseSnafu)?;
                        return AbortedSnafu {}.fail();
                    }
                    Pdu::Unknown { .. } => {
                        // Action AA-8: abort
                        assoc.abort_with_source(AbortRQSource::ServiceProvider(
                            AbortRQServiceProviderReason::UnrecognizedPdu,
                        ))?;
                        return UnknownPduSnafu { pdu }.fail();
                    }
                    _ => {
                        // Action AA-8, abort
                        assoc.abort_with_source(AbortRQSource::ServiceProvider(
                            AbortRQServiceProviderReason::UnexpectedPdu,
                        ))?;
                        return UnexpectedPduSnafu { pdu }.fail();
                    }
                }
            }
            Pdu::AbortRQ { .. } => {
                // Action AA-3, close socket and issue A-ABORT indication.
                assoc.close().context(CloseSnafu)?;
                return AbortedSnafu {}.fail();
            }
            pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::AssociationRQ { .. } => {
                // Action AA-8, abort
                assoc.abort_with_source(AbortRQSource::ServiceProvider(
                    AbortRQServiceProviderReason::UnexpectedPdu,
                ))?;
                return UnexpectedPduSnafu { pdu }.fail();
            }
            pdu @ Pdu::Unknown { .. } => {
                // Action AA-8, abort
                assoc.abort_with_source(AbortRQSource::ServiceProvider(
                    AbortRQServiceProviderReason::UnrecognizedPdu,
                ))?;
                return UnknownPduSnafu { pdu }.fail();
            }
        }
    }
}

/// Helper function to implement the behaviour of the
/// association release for both servers and clients.
/// They differ only very slightly, so this factors
/// both server and client into one function.
///
/// This is the async version.
#[cfg(feature = "async")]
pub(crate) async fn release_impl_async<S>(
    mut assoc: impl AsyncAssociation<S> + Send,
    requestor: bool,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    // Entered in Sta6
    // Action AR-1: send A-RELEASE-RQ PDU; next state: Sta7
    let max_pdu_peer = assoc.acceptor_max_pdu_length();
    let max_pdu_local = assoc.requestor_max_pdu_length();
    let finalization_timeout = assoc.finalization_timeout();
    let read_timeout = assoc.read_timeout();
    let write_timeout = assoc.write_timeout();
    let (socket, read_buffer, write_buffer) = assoc.get_mut();
    write_pdu_to_wire_async(
        socket.as_mut(),
        write_buffer,
        &Pdu::ReleaseRQ,
        max_pdu_peer,
        write_timeout,
    )
    .await?;
    // Sta7
    loop {
        // Note: without a socket timeout, this function may
        // block indefinitely if the peer does not send anything valid.
        let pdu = timeout(
            read_timeout,
            read_pdu_from_wire_async(socket.as_mut(), read_buffer, max_pdu_local, false),
        )
        .await?;

        match pdu {
            Pdu::ReleaseRP => {
                // Action AR-3 (indicate successful release) and close
                // connection. Next state is Sta1 (return).
                assoc.close();
                return Ok(());
            }
            Pdu::PData { .. } => {
                // Action AR-6, remain in Sta7
                // We can't send a P-DATA indication because
                // the API does not allow it at this point.
                // So we just ignore the PDU.
            }
            Pdu::ReleaseRQ => {
                // Release collision, action AR-8 (nothing in our case).
                // Here's where client and server differ.
                if requestor {
                    // Next state for a client is Sta9.
                    // We immediately issue an A-RELEASE response primitive,
                    // so action is AR-9 (send A-RELEASE-RP) and next state
                    // is Sta11.
                    write_pdu_to_wire_async(
                        socket.as_mut(),
                        write_buffer,
                        &Pdu::ReleaseRP,
                        max_pdu_peer,
                        write_timeout,
                    )
                    .await?;
                } else {
                    // Next state for a server is Sta10. We start by receiving.
                }
                // Sta10 or Sta11
                // We had sent an A-RELEASE-RQ which may be replied.
                // We also may receive an A-ABORT which we need to
                // process. Or the peer may receive our A-RELEASE-RP and
                // close the socket. In any case, we need to receive
                // once more.
                let result = timeout(
                    read_timeout,
                    read_pdu_from_wire_async(socket.as_mut(), read_buffer, max_pdu_local, false),
                )
                .await;
                let pdu = match result {
                    Ok(pdu) => pdu,
                    Err(Error::ConnectionClosed { .. }) => {
                        // The peer closed the connection; action is AA-4
                        // (issue A-ABORT indication). Next state is Sta1.
                        return AbortedSnafu {}.fail();
                    }
                    Err(err) => {
                        return Err(err);
                    }
                };
                match pdu {
                    Pdu::ReleaseRP => {
                        // The peer responded to our release request
                        if requestor {
                            // Sta11: Action AR-3 (close socket and confirm
                            // success). Next state is Sta1.
                            assoc.close();
                            return Ok(());
                        }
                        // We're a server, so we're still in Sta10. Action
                        // is AR-10 (confirm release); next state is Sta12.
                        // From Sta12, we immediately send a release response
                        // primitive, which results in action AR-4 (send
                        // A-RELEASE-RP) and state Sta13.
                        write_pdu_to_wire_async(
                            socket.as_mut(),
                            write_buffer,
                            &Pdu::ReleaseRP,
                            max_pdu_peer,
                            write_timeout,
                        )
                        .await?;
                        return sta13_async(
                            socket,
                            read_buffer,
                            write_buffer,
                            max_pdu_local,
                            max_pdu_peer,
                            finalization_timeout,
                        )
                        .await;
                    }
                    Pdu::AbortRQ { .. } => {
                        // Action AA-3, close socket and issue A-ABORT indication.
                        assoc.close();
                        return AbortedSnafu {}.fail();
                    }
                    Pdu::Unknown { .. } => {
                        // Action AA-8: abort
                        assoc
                            .abort_with_source(AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::UnrecognizedPdu,
                            ))
                            .await?;
                        return UnknownPduSnafu { pdu }.fail();
                    }
                    _ => {
                        // Action AA-8, abort
                        assoc
                            .abort_with_source(AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::UnexpectedPdu,
                            ))
                            .await?;
                        return UnexpectedPduSnafu { pdu }.fail();
                    }
                }
            }
            Pdu::AbortRQ { .. } => {
                // Action AA-3, close socket and issue A-ABORT indication.
                assoc.close();
                return AbortedSnafu {}.fail();
            }
            pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::AssociationRQ { .. } => {
                // Action AA-8, abort
                assoc
                    .abort_with_source(AbortRQSource::ServiceProvider(
                        AbortRQServiceProviderReason::UnexpectedPdu,
                    ))
                    .await?;
                return UnexpectedPduSnafu { pdu }.fail();
            }
            pdu @ Pdu::Unknown { .. } => {
                // Action AA-8, abort
                assoc
                    .abort_with_source(AbortRQSource::ServiceProvider(
                        AbortRQServiceProviderReason::UnrecognizedPdu,
                    ))
                    .await?;
                return UnknownPduSnafu { pdu }.fail();
            }
        }
    }
}

/// Helper function that handles state Sta13 of the Association
/// State Machine. It is entered when we're waiting for the peer
/// to close the connection. In normal conditions, that happens
/// when we have sent the last PDU of an association (normally
/// A-ASSOCIATE-RJ, A-RELEASE-RP, or A-ABORT); then it's the
/// peer's responsibility to close the socket, but just in
/// case the peer does not respond, we close it after a timeout
/// if that didn't happen. The response dictated by the ASM in
/// case certain PDUs are received in the meantime is also
/// handled here.
///
/// Returns with the connection closed either way. THe return
/// value is `Ok(())` if the peer closed the association
/// before the timer expired; `Err(Error::Timeout)` if the
/// peer didn't close the connection in time,
/// `Err(Error::Aborted)` if the peer sent us an abort request
/// while waiting, or other errors from intermediate functions.
///
/// For reference see [PS3.8 (2025d) section 9.2.3][1]
/// [1] https://dicom.nema.org/medical/dicom/2025d/output/html/part08.html
pub(crate) fn sta13<S: Read + Write + CloseSocket + SetReadTimeout>(
    socket: &mut S,
    read_buffer: &mut BytesMut,
    write_buffer: &mut Vec<u8>,
    local_max_pdu_length: u32,
    peer_max_pdu_length: u32,
    close_wait_timeout: Duration,
) -> Result<()> {
    // Start ARTIM timer. All actions that lead to Sta13 (except some that
    // happen in Sta13 itself) start it, or restart it if running.
    let deadline = Instant::now() + close_wait_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        // ARTIM expired?
        if remaining.is_zero() {
            // Action AA-2 (stop timer and close socket); next state is
            // Sta1.
            let _ = socket.close();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timed out while waiting for peer to close the connection",
            ))
            .context(TimeoutSnafu);
        }

        socket
            .set_read_timeout(Some(remaining))
            .context(SetReadTimeoutSnafu)?;

        match read_pdu_from_wire(socket, read_buffer, local_max_pdu_length, false) {
            Err(Error::ConnectionClosed { .. }) => {
                // Peer did the correct thing and closed the socket;
                // action is AR-5 (nothing in our case) and next state
                // is Sta1.
                return Ok(());
            }

            Ok(Pdu::AbortRQ { .. }) => {
                // Peer requested an abort while we waited for peer to
                // close the connection, so we close it ourselves
                // instead and return Aborted (action AA-2, next state Sta1).
                socket.close().context(CloseSnafu)?;
                return AbortedSnafu {}.fail();
            }

            Ok(Pdu::AssociationAC(_))
            | Ok(Pdu::AssociationRJ(_))
            | Ok(Pdu::PData { .. })
            | Ok(Pdu::ReleaseRQ)
            | Ok(Pdu::ReleaseRP) => {
                // Action AA-6: Ignore PDU; remain in Sta13 without
                // restarting the timer
            }

            Ok(pdu) => {
                // A-ASSOCIATE-RQ or invalid PDU
                // Action AA-7: Send A-ABORT PDU; remain in Sta13
                // without restarting the timer
                write_pdu_to_wire(
                    socket,
                    write_buffer,
                    &Pdu::AbortRQ {
                        source: AbortRQSource::ServiceProvider(if let Pdu::Unknown { .. } = pdu {
                            AbortRQServiceProviderReason::UnrecognizedPdu
                        } else {
                            AbortRQServiceProviderReason::UnexpectedPdu
                        }),
                    },
                    peer_max_pdu_length,
                )?;
            }

            Err(Error::Timeout { .. }) => {
                // Do nothing; we will catch it when the timer expires
            }

            Err(e) => {
                let _ = socket.close().context(CloseSnafu);
                return Err(e);
            }
        }
    }
}

/// Helper function that handles state Sta13 of the Association
/// State Machine. It is entered when we're waiting for the peer
/// to close the connection. In normal conditions, that happens
/// when we have sent the last PDU of an association (normally
/// A-ASSOCIATE-RJ, A-RELEASE-RP, or A-ABORT); then it's the
/// peer's responsibility to close the socket, but just in
/// case the peer does not respond, we close it after a timeout
/// if that didn't happen. The response dictated by the ASM in
/// case certain PDUs are received in the meantime is also
/// handled here.
///
/// Returns with the connection closed either way. The return
/// value is `Ok(())` if the peer closed the association
/// before the timer expired; `Err(Error::Timeout)` if the
/// peer didn't close the connection in time,
/// `Err(Error::Aborted)` if the peer sent us an abort request
/// while waiting, or other errors from intermediate functions.
///
/// For reference see [PS3.8 (2025d) section 9.2.3][1]
/// [1] https://dicom.nema.org/medical/dicom/2025d/output/html/part08.html
///
/// This is the asynchronous version.
#[cfg(feature = "async")]
pub(crate) async fn sta13_async<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    socket: &mut Option<S>,
    read_buffer: &mut BytesMut,
    write_buffer: &mut Vec<u8>,
    local_max_pdu_length: u32,
    peer_max_pdu_length: u32,
    close_wait_timeout: Duration,
) -> Result<()> {
    let timer = tokio::time::sleep(close_wait_timeout);
    tokio::pin!(timer);

    loop {
        tokio::select! {
            biased;

            _ = &mut timer => {
                // Action AA-2 (stop timer and close socket); next state is
                // Sta1.
                drop(socket.take());
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Timed out while waiting for peer to close the connection",
                ))
                .context(TimeoutSnafu);
            }

            result = read_pdu_from_wire_async(socket.as_mut(), read_buffer, local_max_pdu_length, false) => {

                match result {
                    Err(Error::ConnectionClosed { .. }) => {
                        // Peer did the correct thing and closed the socket;
                        // action is AR-5 (nothing in our case) and next state
                        // is Sta1.
                        return Ok(());
                    }

                    Ok(Pdu::AbortRQ { .. }) => {
                        // Peer requested an abort while we waited for peer to
                        // close the connection, so we close it ourselves
                        // instead and return Aborted (action AA-2, next state Sta1).
                        drop(socket.take());
                        return AbortedSnafu {}.fail();
                    }

                    Ok(Pdu::AssociationAC(_))
                    | Ok(Pdu::AssociationRJ(_))
                    | Ok(Pdu::PData { .. })
                    | Ok(Pdu::ReleaseRQ)
                    | Ok(Pdu::ReleaseRP) => {
                        // Action AA-6: Ignore PDU; remain in Sta13 without
                        // restarting the timer
                    }

                    Ok(pdu) => {
                        // A-ASSOCIATE-RQ or invalid PDU
                        // Action AA-7: Send A-ABORT PDU; remain in Sta13
                        // without restarting the timer
                        write_pdu_to_wire_async(
                            socket.as_mut(),
                            write_buffer,
                            &Pdu::AbortRQ {
                                source: AbortRQSource::ServiceProvider(if let Pdu::Unknown { .. } = pdu {
                                    AbortRQServiceProviderReason::UnrecognizedPdu
                                } else {
                                    AbortRQServiceProviderReason::UnexpectedPdu
                                }),
                            },
                            peer_max_pdu_length,
                            Some(close_wait_timeout),
                        ).await?;
                    }

                    Err(Error::Timeout { .. }) => {
                        // Do nothing; we will catch it when the timer expires
                    }

                    Err(e) => {
                        drop(socket.take());
                        return Err(e);
                    }
                }
            }
        }
    }
}
