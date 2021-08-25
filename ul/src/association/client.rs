//! Association requester module
//!
//! The module provides an abstraction for a DICOM association
//! in which this application entity is the one requesting the association.
//! See [`ClientAssociationOptions`](self::ClientAssociationOptions)
//! for details and examples on how to create an association.
use std::{borrow::Cow, io::Write, net::{TcpStream, ToSocketAddrs}};

use crate::pdu::{
    reader::read_pdu, writer::write_pdu, AbortRQSource, AssociationRJResult, AssociationRJSource,
    Pdu, PresentationContextProposed, PresentationContextResult, PresentationContextResultReason,
};
use snafu::{ensure, ResultExt, Snafu};

use super::pdata::PDataWriter;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// missing abstract syntax to begin negotiation
    MissingAbstractSyntax,

    /// could not connect to server
    Connect { source: std::io::Error },

    /// failed to send association request
    SendRequest { source: crate::pdu::writer::Error },

    /// failed to receive association response
    ReceiveResponse { source: crate::pdu::reader::Error },

    #[snafu(display("unexpected response from server `{:?}`", pdu))]
    #[non_exhaustive]
    UnexpectedResponse {
        /// the PDU obtained from the server
        pdu: Pdu,
    },

    #[snafu(display("unknown response from server `{:?}`", pdu))]
    #[non_exhaustive]
    UnknownResponse {
        /// the PDU obtained from the server, of variant Unknown
        pdu: Pdu,
    },

    #[snafu(display("protocol version mismatch: expected {}, got {}", expected, got))]
    ProtocolVersionMismatch { expected: u16, got: u16 },

    /// the association was rejected by the server
    Rejected {
        association_result: AssociationRJResult,
        association_source: AssociationRJSource,
    },

    /// no presentation contexts accepted by the server
    NoAcceptedPresentationContexts,

    /// failed to send PDU message
    #[non_exhaustive]
    Send { source: crate::pdu::writer::Error },

    /// failed to send PDU message on wire
    #[non_exhaustive]
    WireSend { source: std::io::Error },

    /// failed to receive PDU message
    #[non_exhaustive]
    Receive { source: crate::pdu::reader::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A DICOM association builder for a client node.
/// The final outcome is a [`ClientAssociation`].
///
/// This is the standard way of requesting and establishing
/// an association with another DICOM node,
/// that one usually taking the role of a service class provider (SCP).
///
/// # Example
///
/// ```no_run
/// # use dicom_ul::association::client::ClientAssociationOptions;
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let association = ClientAssociationOptions::new()
///    .with_abstract_syntax("1.2.840.10008.1.1")
///    .with_transfer_syntax("1.2.840.10008.1.2.1")
///    .establish("129.168.0.5:104")?;
/// # Ok(())
/// # }
/// ```
///
/// At least one abstract syntax must be specified,
/// using the method [`with_abstract_syntax`](Self::with_abstract_syntax).
/// The requester will admit by default the transfer syntaxes
/// _Implicit VR Little Endian_
/// and _Explicit VR Little Endian_.
/// Other transfer syntaxes can be requested in the association
/// via the method [`with_transfer_syntax`](Self::with_transfer_syntax).
#[derive(Debug, Clone)]
pub struct ClientAssociationOptions<'a> {
    /// the calling AE title
    calling_ae_title: Cow<'a, str>,
    /// the called AE title
    called_ae_title: Cow<'a, str>,
    /// the requested application context name
    application_context_name: Cow<'a, str>,
    /// the list of requested abstract syntaxes
    abstract_syntax_uids: Vec<Cow<'a, str>>,
    /// the list of requested transfer syntaxes
    transfer_syntax_uids: Vec<Cow<'a, str>>,
    /// the expected protocol version
    protocol_version: u16,
    /// the maximum PDU length
    max_pdu_length: u32,
}

impl<'a> Default for ClientAssociationOptions<'a> {
    fn default() -> Self {
        ClientAssociationOptions {
            /// the calling AE title
            calling_ae_title: "THIS-SCU".into(),
            /// the called AE title
            called_ae_title: "ANY-SCP".into(),
            /// the requested application context name
            application_context_name: "1.2.840.10008.3.1.1.1".into(),
            /// the list of requested abstract syntaxes
            abstract_syntax_uids: Vec::new(),
            /// the application context name
            transfer_syntax_uids: Vec::new(),
            protocol_version: 1,
            max_pdu_length: crate::pdu::reader::DEFAULT_MAX_PDU,
        }
    }
}

impl<'a> ClientAssociationOptions<'a> {
    /// Create a new set of options for establishing an association.
    pub fn new() -> Self {
        Self::default()
    }

    /// Define the calling application entity title for the association,
    /// which refers to this DICOM node.
    ///
    /// The default is `THIS-SCU`.
    pub fn calling_ae_title<T>(mut self, calling_ae_title: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.calling_ae_title = calling_ae_title.into();
        self
    }

    /// Define the called application entity title for the association,
    /// which refers to the target DICOM node.
    ///
    /// The default is `ANY-SCP`.
    pub fn called_ae_title<T>(mut self, called_ae_title: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.called_ae_title = called_ae_title.into();
        self
    }

    /// Include this abstract syntax
    /// in the list of proposed presentation contexts.
    pub fn with_abstract_syntax<T>(mut self, abstract_syntax_uid: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.abstract_syntax_uids.push(abstract_syntax_uid.into());
        self
    }

    /// Include this transfer syntax in each proposed presentation context.
    pub fn with_transfer_syntax<T>(mut self, transfer_syntax_uid: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.transfer_syntax_uids.push(transfer_syntax_uid.into());
        self
    }

    /// Override the maximum expected PDU length.
    pub fn max_pdu_length(mut self, value: u32) -> Self {
        self.max_pdu_length = value;
        self
    }

    /// Initiate the TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    pub fn establish<A: ToSocketAddrs>(self, address: A) -> Result<ClientAssociation> {
        let ClientAssociationOptions {
            calling_ae_title,
            called_ae_title,
            application_context_name,
            abstract_syntax_uids,
            mut transfer_syntax_uids,
            protocol_version,
            max_pdu_length,
        } = self;

        // fail if no abstract syntaxes were provided: they represent intent,
        // should not be omitted by the user
        ensure!(!abstract_syntax_uids.is_empty(), MissingAbstractSyntax);

        // provide default transfer syntaxes
        if transfer_syntax_uids.is_empty() {
            // Explicit VR Little Endian
            transfer_syntax_uids.push("1.2.840.10008.1.2.1".into());
            // Implicit VR Little Endian
            transfer_syntax_uids.push("1.2.840.10008.1.2".into());
        }

        let presentation_contexts: Vec<_> = abstract_syntax_uids
            .into_iter()
            .enumerate()
            .map(|(i, abstract_syntax)| PresentationContextProposed {
                id: (i + 1) as u8,
                abstract_syntax: abstract_syntax.to_string(),
                transfer_syntaxes: transfer_syntax_uids
                    .iter()
                    .map(|uid| uid.to_string())
                    .collect(),
            })
            .collect();
        let msg = Pdu::AssociationRQ {
            protocol_version,
            calling_ae_title: calling_ae_title.to_string(),
            called_ae_title: called_ae_title.to_string(),
            application_context_name: application_context_name.to_string(),
            presentation_contexts: presentation_contexts.clone(),
            user_variables: vec![],
        };

        let mut socket = std::net::TcpStream::connect(address).context(Connect)?;
        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        // send request

        write_pdu(&mut buffer, &msg).context(SendRequest)?;
        socket.write_all(&buffer).context(WireSend)?;
        buffer.clear();
        // receive response
        let msg = read_pdu(&mut socket, max_pdu_length).context(ReceiveResponse)?;

        match msg {
            Pdu::AssociationAC {
                protocol_version: protocol_version_scp,
                application_context_name: _,
                presentation_contexts: presentation_contexts_scp,
                user_variables: _,
            } => {
                ensure!(
                    protocol_version == protocol_version_scp,
                    ProtocolVersionMismatch {
                        expected: protocol_version,
                        got: protocol_version_scp,
                    }
                );

                let presentation_contexts: Vec<_> = presentation_contexts_scp
                    .into_iter()
                    .filter(|c| c.reason == PresentationContextResultReason::Acceptance)
                    .collect();
                if presentation_contexts.is_empty() {
                    // abort connection
                    let _ = write_pdu(
                        &mut buffer,
                        &Pdu::AbortRQ {
                            source: AbortRQSource::ServiceUser,
                        },
                    );
                    let _ = socket.write_all(&buffer);
                    buffer.clear();
                    return NoAcceptedPresentationContexts.fail();
                }
                Ok(ClientAssociation {
                    presentation_contexts,
                    max_pdu_length,
                    socket,
                    buffer
                })
            }
            Pdu::AssociationRJ { result, source } => Rejected {
                association_result: result,
                association_source: source,
            }
            .fail(),
            pdu @ Pdu::AbortRQ { .. }
            | pdu @ Pdu::ReleaseRQ { .. }
            | pdu @ Pdu::AssociationRQ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRP { .. } => {
                // abort connection
                let _ = write_pdu(
                    &mut buffer,
                    &Pdu::AbortRQ {
                        source: AbortRQSource::ServiceUser,
                    },
                );
                let _ = socket.write_all(&buffer);
                UnexpectedResponse { pdu }.fail()
            }
            pdu @ Pdu::Unknown { .. } => {
                // abort connection
                let _ = write_pdu(
                    &mut buffer,
                    &Pdu::AbortRQ {
                        source: AbortRQSource::ServiceUser,
                    },
                );
                let _ = socket.write_all(&buffer);
                UnknownResponse { pdu }.fail()
            }
        }
    }
}

/// A DICOM upper level association from the perspective
/// of a requesting application entity.
///
/// The most common operations of an established association are
/// [`send`](Self::send)
/// and [`receive`](Self::receive).
/// Sending large P-Data fragments may be easier through the P-Data sender
/// abstraction (see [`send_pdata`](Self::send_pdata)).
///
/// When the value falls out of scope,
/// the program will automatically try to gracefully release the association
/// through a standard C-RELEASE message exchange,
/// then shut down the underlying TCP connection.
#[derive(Debug)]
pub struct ClientAssociation {
    /// The presentation contexts accorded with the acceptor application entity,
    /// without the rejected ones.
    presentation_contexts: Vec<PresentationContextResult>,
    /// The maximum PDU length
    max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: TcpStream,
    /// Buffer to assemble PDU before sending it on wire
    buffer: Vec<u8>,
}

impl ClientAssociation {
    /// Retrieve the list of negotiated presentation contexts.
    pub fn presentation_contexts(&self) -> &[PresentationContextResult] {
        &self.presentation_contexts
    }

    /// Send a PDU message to the other intervenient.
    pub fn send(&mut self, msg: &Pdu) -> Result<()> {
        self.buffer.clear();
        write_pdu(&mut self.buffer, &msg).context(Send)?;
        self.socket.write_all(&self.buffer).context(WireSend)
    }

    /// Read a PDU message from the other intervenient.
    pub fn receive(&mut self) -> Result<Pdu> {
        read_pdu(&mut self.socket, self.max_pdu_length).context(Receive)
    }

    /// Gracefully terminate the association by exchanging release messages
    /// and then shutting down the TCP connection.
    pub fn release(mut self) -> Result<()> {
        let out = self.release_impl();
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        out
    }

    /// Release implementation function,
    /// which tries to send a release request and receive a release response.
    /// This is in a separate private function because
    /// terminating a connection should still close the connection
    /// if the exchange fails.
    /// Send an abort message and shut down the TCP connection,
    /// terminating the association.
    pub fn abort(mut self) -> Result<()> {
        let pdu = Pdu::AbortRQ {
            source: AbortRQSource::ServiceUser,
        };
        let out =self.send(&pdu);
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        out
    }

    /// Obtain access to the inner TCP stream
    /// connected to the association acceptor.
    ///
    /// This can be used to send the PDU in semantic fragments of the message,
    /// thus using less memory.
    ///
    /// **Note:** reading and writing should be done with care
    /// to avoid inconsistencies in the association state.
    /// Do not call `send` and `receive` while not in a PDU boundary.
    pub fn inner_stream(&mut self) -> &mut TcpStream {
        &mut self.socket
    }

    /// Prepare a P-Data writer for sending
    /// one or more data items.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    pub fn send_pdata(&mut self, presentation_context_id: u8) -> PDataWriter<&mut TcpStream> {
        PDataWriter::new(
            &mut self.socket,
            presentation_context_id,
            self.max_pdu_length,
        )
    }

    fn release_impl(&mut self) -> Result<()> {
        let pdu = Pdu::ReleaseRQ;
        self.send(&pdu)?;
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
        Ok(())
    }
}

/// Automatically release the association and shut down the connection.
impl Drop for ClientAssociation {
    fn drop(&mut self) {
        let _ = self.release_impl();
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
    }
}
