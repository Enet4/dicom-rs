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
    io::{BufRead, BufReader, Cursor, Read}, time::Duration,
};

use bytes::{Buf, BytesMut};
pub use pdata::{PDataReader, PDataWriter};
pub use client::{ClientAssociation, ClientAssociationOptions};
pub use server::{ServerAssociation, ServerAssociationOptions};
#[cfg(feature = "async")]
pub use pdata::non_blocking::AsyncPDataWriter;
#[cfg(feature = "async")]
pub use client::AsyncClientAssociation;
#[cfg(feature = "async")]
pub use server::AsyncServerAssociation;
use snafu::{ensure, ResultExt, Snafu};

use crate::{Pdu, pdu::{self, AssociationRJ, PresentationContextNegotiated, ReadPduSnafu, UserVariableItem}, write_pdu};

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
    SendPdu {
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

    #[snafu(display("failed close connection: {}", source))]
    Close {
        source: std::io::Error,
        backtrace: Backtrace
    },

    #[snafu(display(
        "PDU is too large ({} bytes) to be sent to the remote application entity",
        length
    ))]
    #[non_exhaustive]
    SendTooLongPdu { length: usize, backtrace: Backtrace },

    #[snafu(display("Connection closed by peer"))]
    ConnectionClosed,

    /// TLS configuration is missing
    #[cfg(feature = "tls")]
    #[snafu(display("TLS configuration is required but not provided"))]
    TlsConfigMissing { backtrace: Backtrace },

    /// Invalid server name for TLS
    #[cfg(feature = "tls")]
    #[snafu(display("Invalid server name for TLS connection"))]
    InvalidServerName { 
        source: rustls::pki_types::InvalidDnsNameError,
        backtrace: Backtrace 
    },

    /// Failed to establish TLS connection
    #[cfg(feature = "tls")]
    #[snafu(display("Failed to establish TLS connection: {:?}", source))]
    TlsConnection { 
        source: rustls::Error,
        backtrace: Backtrace 
    },
}
/// Struct to hold negotiated options after association is accepted
pub(crate) struct NegotiatedOptions{
    /// Maximum PDU length the peer can handle
    peer_max_pdu_length: u32,
    /// User variables accepted by the peer
    user_variables: Vec<UserVariableItem>,
    /// Presentation contexts accepted by the peer
    presentation_contexts: Vec<PresentationContextNegotiated>,
    /// The peer's AE title
    peer_ae_title: String
}

/// Socket configuration for associations
#[derive(Debug, Clone, Copy, Default)]
pub struct SocketOptions {
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

#[cfg(feature = "tls")]
impl CloseSocket for rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>{
    fn close(&mut self) -> std::io::Result<()> {
        self.get_mut().shutdown(std::net::Shutdown::Both)
    }
}

#[cfg(feature = "tls")]
impl CloseSocket for rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>{
    fn close(&mut self) -> std::io::Result<()> {
        self.get_mut().shutdown(std::net::Shutdown::Both)
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
    /// that this application entity is expecting to receive.
    ///
    /// The current implementation is not required to fail
    /// and/or abort the association
    /// if a larger PDU is received.
    fn requestor_max_pdu_length(&self) -> u32;

    /// Obtain a view of the negotiated presentation contexts.
    fn presentation_contexts(&self) -> &[PresentationContextNegotiated];

    /// Retrieve the user variables that were taken from the server.
    ///
    /// It usually contains the maximum PDU length,
    /// the implementation class UID, and the implementation version name.
    fn user_variables(&self) -> &[UserVariableItem];
}

mod private {
    use crate::{Pdu, pdu::{AbortRQServiceProviderReason, AbortRQSource}};
    use snafu::{ResultExt};

    /// Private trait which exposes "unsafe" methods that should not be called by the user
    /// 
    /// `close` and `release` _should_ take ownership, and in the public interface, they 
    /// do. However, in order to implement `Drop` we need to expose a version of these 
    /// methods that don't take ownership.
    /// 
    /// `send` and `receive` implementations are needed in order to provide
    /// the implementation for `release`
    pub trait SyncAssociationSealed<S: std::io::Read + std::io::Write + super::CloseSocket> {

        fn close(&mut self) -> std::io::Result<()>;
        fn send(&mut self, pdu: &Pdu) -> super::Result<()>;
        fn receive(&mut self) -> super::Result<Pdu>;
        fn release(&mut self) -> super::Result<()>{
            let pdu = Pdu::ReleaseRQ;
            self.send(&pdu)?;
            let pdu = self.receive()?;

            match pdu {
                Pdu::ReleaseRP => {}
                pdu @ Pdu::AbortRQ { .. }
                | pdu @ Pdu::AssociationAC { .. }
                | pdu @ Pdu::AssociationRJ { .. }
                | pdu @ Pdu::AssociationRQ { .. }
                | pdu @ Pdu::PData { .. }
                | pdu @ Pdu::ReleaseRQ => return super::UnexpectedPduSnafu { pdu }.fail(),
                pdu @ Pdu::Unknown { .. } => return super::UnknownPduSnafu { pdu }.fail(),
            }
            self.close()
                .context(super::CloseSnafu)?;
            Ok(())
        }

        fn abort(&mut self) -> super::Result<()> where Self: Sized {
            let pdu = Pdu::AbortRQ {
                source: AbortRQSource::ServiceProvider(
                    AbortRQServiceProviderReason::ReasonNotSpecified,
                ),
            };
            let out = self.send(&pdu);
            let _ = self.close();
            out
        }
    }

    /// Private trait which exposes "unsafe" methods that should not be called by the user
    /// 
    /// `close` and `release` _should_ take ownership, and in the public interface, they 
    /// do. However, in order to implement `Drop` we need to expose a version of these 
    /// methods that don't take ownership.
    /// 
    /// `send` and `receive` implementations are needed in order to provide
    /// the implementation for `release`
    #[cfg(feature = "async")]
    pub trait AsyncAssociationSealed<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> {
        fn close(&mut self) -> impl std::future::Future<Output = std::io::Result<()>> + Send
            where Self: Send;
        fn send(&mut self, pdu: &Pdu) -> impl std::future::Future<Output=super::Result<()>> + Send 
            where Self: Send;
        fn receive(&mut self) -> impl std::future::Future<Output = super::Result<Pdu>> + Send 
            where Self: Send;
        fn release(&mut self) -> impl std::future::Future<Output=super::Result<()>> + Send
            where Self: Send {
            async move {
                let pdu = Pdu::ReleaseRQ;
                self.send(&pdu).await?;
                let pdu = self.receive().await?;

                match pdu {
                    Pdu::ReleaseRP => {}
                    pdu @ Pdu::AbortRQ { .. }
                    | pdu @ Pdu::AssociationAC { .. }
                    | pdu @ Pdu::AssociationRJ { .. }
                    | pdu @ Pdu::AssociationRQ { .. }
                    | pdu @ Pdu::PData { .. }
                    | pdu @ Pdu::ReleaseRQ => return super::UnexpectedPduSnafu { pdu }.fail(),
                    pdu @ Pdu::Unknown { .. } => return super::UnknownPduSnafu { pdu }.fail(),
                }
                self.close()
                    .await
                    .context(super::CloseSnafu)?;
                Ok(())
            }
        }

        fn abort(&mut self) -> impl std::future::Future<Output = super::Result<()>> + Send 
        where Self: Sized + Send {
            let pdu = Pdu::AbortRQ {
                source: AbortRQSource::ServiceProvider(
                    AbortRQServiceProviderReason::ReasonNotSpecified,
                ),
            };
            async move {
                let out = self.send(&pdu).await;
                let _ = self.close().await;
                out
            }
        }
    }
}

/// Trait that represents methods that can be made on a synchronous association.
pub trait SyncAssociation<S: std::io::Read + std::io::Write + CloseSocket>: private::SyncAssociationSealed<S> + Association {

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

    /// Obtain mutable access to the inner stream and read buffer
    fn get_mut(&mut self) -> (&mut S, &mut BytesMut);

    /// Send a PDU message to the other intervenient.
    fn send(&mut self, pdu: &Pdu) -> Result<()>{
        private::SyncAssociationSealed::send(self, pdu)
    }

    /// Read a PDU message from the other intervenient.
    fn receive(&mut self) -> Result<Pdu>{
        private::SyncAssociationSealed::receive(self)
    }

    /// Send a provider initiated abort message
    /// and shut down the TCP connection,
    /// terminating the association.
    fn abort(mut self) -> Result<()> where Self: Sized {
        private::SyncAssociationSealed::abort(&mut self)
    }

    /// Iniate a graceful release of the association
    fn release(mut self) -> Result<()> where Self: Sized{
        private::SyncAssociationSealed::release(&mut self)
    }

    /// Prepare a P-Data writer for sending
    /// one or more data item PDUs.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    fn send_pdata(&mut self, presentation_context_id: u8) -> PDataWriter<&mut S>{
        let max_pdu_length = self.acceptor_max_pdu_length();
        PDataWriter::new(
            self.inner_stream(),
            presentation_context_id,
            max_pdu_length,
        )

    }

    /// Prepare a P-Data reader for receiving
    /// one or more data item PDUs.
    ///
    /// Returns a reader which automatically
    /// receives more data PDUs once the bytes collected are consumed.
    fn receive_pdata(&mut self) -> PDataReader<'_, &mut S>{
        let max_pdu_length = self.requestor_max_pdu_length();
        let (socket, read_buffer) = self.get_mut();
        PDataReader::new(
            socket,
            max_pdu_length,
            read_buffer,
        )
    }
}

#[cfg(feature = "async")]
/// Trait that represents methods that can be made on an asynchronous association.
pub trait AsyncAssociation<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>: private::AsyncAssociationSealed<S> + Association {

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

    /// Obtain mutable access to the inner stream and read buffer
    fn get_mut(&mut self) -> (&mut S, &mut BytesMut);

    /// Send a PDU message to the other intervenient.
    fn send(&mut self, pdu: &Pdu) -> impl std::future::Future<Output = Result<()>> + Send 
    where Self: Send {
        async move{ 
            private::AsyncAssociationSealed::send(self, pdu).await
        }
    }

    /// Read a PDU message from the other intervenient.
    fn receive(&mut self) -> impl std::future::Future<Output = Result<Pdu>> + Send
    where Self: Send {
        async move {
            private::AsyncAssociationSealed::receive(self).await
        }
    }

    /// Send a provider initiated abort message
    /// and shut down the TCP connection,
    /// terminating the association.
    fn abort(mut self) -> impl std::future::Future<Output = Result<()>> + Send 
    where Self: Sized + Send {
        async move {
            private::AsyncAssociationSealed::abort(&mut self).await
        }
    }

    /// Iniate a graceful release of the association
    fn release(mut self) -> impl std::future::Future<Output = Result<()>> + Send 
    where Self: Sized + Send {
        async move {
            private::AsyncAssociationSealed::release(&mut self).await
        }
    }

    /// Prepare a P-Data writer for sending
    /// one or more data item PDUs.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    fn send_pdata(&mut self, presentation_context_id: u8) -> AsyncPDataWriter<&mut S>{
        let max_pdu_length = self.acceptor_max_pdu_length();
        AsyncPDataWriter::new(
            self.inner_stream(),
            presentation_context_id,
            max_pdu_length,
        )

    }

    /// Prepare a P-Data reader for receiving
    /// one or more data item PDUs.
    ///
    /// Returns a reader which automatically
    /// receives more data PDUs once the bytes collected are consumed.
    fn receive_pdata(&mut self) -> PDataReader<'_, &mut S>{
        let max_pdu_length = self.requestor_max_pdu_length();
        let (socket, read_buffer) = self.get_mut();
        PDataReader::new(
            socket,
            max_pdu_length,
            read_buffer,
        )
    }
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
            .context(crate::association::TimeoutSnafu)?
    } else {
        block.await
    }
}

/// Encode a PDU into the provided buffer
pub fn encode_pdu(buffer: &mut Vec<u8>, pdu: &Pdu, peer_max_pdu_length: u32) -> Result<()> {
    write_pdu( buffer, pdu).context(SendPduSnafu)?;
    if buffer.len() > peer_max_pdu_length as usize {
        return SendTooLongPduSnafu {
            length: buffer.len(),
        }
        .fail();
    }
    Ok(())
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

/// Helper function to get a PDU from an async reader.
///
/// Chunks of data are read into `read_buffer`,
/// which should be passed in subsequent calls
/// to receive more PDUs from the same stream.
#[cfg(feature = "async")]
pub async fn read_pdu_from_wire_async<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    read_buffer: &mut BytesMut,
    max_pdu_length: u32,
    strict: bool,
) -> Result<Pdu> {
    use tokio::io::AsyncReadExt;
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
