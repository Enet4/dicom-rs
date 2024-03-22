//! Association requester module
//!
//! The module provides an abstraction for a DICOM association
//! in which this application entity is the one requesting the association.
//! See [`ClientAssociationOptions`](self::ClientAssociationOptions)
//! for details and examples on how to create an association.
use std::{
    borrow::Cow,
    convert::TryInto,
    io::Write,
    net::{TcpStream, ToSocketAddrs},
};

use crate::{
    pdu::{
        reader::{read_pdu, DEFAULT_MAX_PDU, MAXIMUM_PDU_SIZE},
        writer::write_pdu,
        AbortRQSource, AssociationAC, AssociationRJ, AssociationRQ, Pdu,
        PresentationContextProposed, PresentationContextResult, PresentationContextResultReason,
        UserVariableItem,
    },
    AeAddr, IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME,
};
use snafu::{ensure, Backtrace, ResultExt, Snafu};

use super::{
    pdata::{PDataReader, PDataWriter},
    uid::trim_uid,
};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// missing abstract syntax to begin negotiation
    MissingAbstractSyntax { backtrace: Backtrace },

    /// could not connect to server
    Connect {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// failed to send association request
    SendRequest {
        #[snafu(backtrace)]
        source: crate::pdu::writer::Error,
    },

    /// failed to receive association response
    ReceiveResponse {
        #[snafu(backtrace)]
        source: crate::pdu::reader::Error,
    },

    #[snafu(display("unexpected response from server `{:?}`", pdu))]
    #[non_exhaustive]
    UnexpectedResponse {
        /// the PDU obtained from the server
        pdu: Box<Pdu>,
    },

    #[snafu(display("unknown response from server `{:?}`", pdu))]
    #[non_exhaustive]
    UnknownResponse {
        /// the PDU obtained from the server, of variant Unknown
        pdu: Box<Pdu>,
    },

    #[snafu(display("protocol version mismatch: expected {}, got {}", expected, got))]
    ProtocolVersionMismatch {
        expected: u16,
        got: u16,
        backtrace: Backtrace,
    },

    #[snafu(display("association rejected by the server: {}", association_rj.source))]
    Rejected {
        association_rj: AssociationRJ,
        backtrace: Backtrace,
    },

    /// no presentation contexts accepted by the server
    NoAcceptedPresentationContexts { backtrace: Backtrace },

    /// failed to send PDU message
    #[non_exhaustive]
    Send {
        #[snafu(backtrace)]
        source: crate::pdu::writer::Error,
    },

    /// failed to send PDU message on wire
    #[non_exhaustive]
    WireSend {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "PDU is too large ({} bytes) to be sent to the remote application entity",
        length
    ))]
    #[non_exhaustive]
    SendTooLongPdu { length: usize, backtrace: Backtrace },

    /// failed to receive PDU message
    #[non_exhaustive]
    Receive {
        #[snafu(backtrace)]
        source: crate::pdu::reader::Error,
    },
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
///    .with_presentation_context("1.2.840.10008.1.1", vec!["1.2.840.10008.1.2.1", "1.2.840.10008.1.2"])
///    .establish("129.168.0.5:104")?;
/// # Ok(())
/// # }
/// ```
///
/// At least one presentation context must be specified,
/// using the method [`with_presentation_context`](Self::with_presentation_context)
/// and supplying both an abstract syntax and list of transfer syntaxes.
///
/// A helper method [`with_abstract_syntax`](Self::with_abstract_syntax) will
/// include by default the transfer syntaxes
/// _Implicit VR Little Endian_ and _Explicit VR Little Endian_
/// in the resulting presentation context.
/// # Example
///
/// ```no_run
/// # use dicom_ul::association::client::ClientAssociationOptions;
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let association = ClientAssociationOptions::new()
///     .with_abstract_syntax("1.2.840.10008.1.1")
///     .establish("129.168.0.5:104")?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ClientAssociationOptions<'a> {
    /// the calling AE title
    calling_ae_title: Cow<'a, str>,
    /// the called AE title
    called_ae_title: Option<Cow<'a, str>>,
    /// the requested application context name
    application_context_name: Cow<'a, str>,
    /// the list of requested presentation contexts
    presentation_contexts: Vec<(Cow<'a, str>, Vec<Cow<'a, str>>)>,
    /// the expected protocol version
    protocol_version: u16,
    /// the maximum PDU length requested for receiving PDUs
    max_pdu_length: u32,
    /// whether to receive PDUs in strict mode
    strict: bool,
}

impl<'a> Default for ClientAssociationOptions<'a> {
    fn default() -> Self {
        ClientAssociationOptions {
            // the calling AE title
            calling_ae_title: "THIS-SCU".into(),
            // the called AE title
            called_ae_title: None,
            // the requested application context name
            application_context_name: "1.2.840.10008.3.1.1.1".into(),
            // the list of requested presentation contexts
            presentation_contexts: Vec::new(),
            protocol_version: 1,
            max_pdu_length: crate::pdu::reader::DEFAULT_MAX_PDU,
            strict: true,
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
    /// Passing an emoty string resets the AE title to the default
    /// (or to the one passed via [`establish_with`](ClientAssociationOptions::establish_with)).
    pub fn called_ae_title<T>(mut self, called_ae_title: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let cae = called_ae_title.into();
        if cae.is_empty() {
            self.called_ae_title = None;
        } else {
            self.called_ae_title = Some(cae);
        }
        self
    }

    /// Include this presentation context
    /// in the list of proposed presentation contexts.
    pub fn with_presentation_context<T>(
        mut self,
        abstract_syntax_uid: T,
        transfer_syntax_uids: Vec<T>,
    ) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let transfer_syntaxes: Vec<Cow<'a, str>> = transfer_syntax_uids
            .into_iter()
            .map(|t| trim_uid(t.into()))
            .collect();
        self.presentation_contexts
            .push((trim_uid(abstract_syntax_uid.into()), transfer_syntaxes));
        self
    }

    /// Helper to add this abstract syntax
    /// with the default transfer syntaxes
    /// to the list of proposed presentation contexts.
    pub fn with_abstract_syntax<T>(self, abstract_syntax_uid: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let default_transfer_syntaxes: Vec<Cow<'a, str>> =
            vec!["1.2.840.10008.1.2.1".into(), "1.2.840.10008.1.2".into()];
        self.with_presentation_context(abstract_syntax_uid.into(), default_transfer_syntaxes)
    }

    /// Override the maximum PDU length
    /// that this application entity will admit.
    pub fn max_pdu_length(mut self, value: u32) -> Self {
        self.max_pdu_length = value;
        self
    }

    /// Override strict mode:
    /// whether receiving PDUs must not
    /// surpass the negotiated maximum PDU length.
    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Initiate the TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    pub fn establish<A: ToSocketAddrs>(self, address: A) -> Result<ClientAssociation> {
        self.establish_impl(AeAddr::new_socket_addr(address))
    }

    /// Initiate the TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    ///
    /// This method allows you to specify the called AE title
    /// alongside with the socket address.
    /// See [AeAddr](`crate::AeAddr`) for more details.
    /// However, the AE title in this parameter
    /// is overridden by any `called_ae_title` option
    /// previously received.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dicom_ul::association::client::ClientAssociationOptions;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let association = ClientAssociationOptions::new()
    ///     .with_abstract_syntax("1.2.840.10008.1.1")
    ///     // called AE title in address
    ///     .establish_with("MY-STORAGE@10.0.0.100:104")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn establish_with(self, ae_address: &str) -> Result<ClientAssociation> {
        match ae_address.try_into() {
            Ok(ae_address) => self.establish_impl(ae_address),
            Err(_) => self.establish_impl(AeAddr::new_socket_addr(ae_address)),
        }
    }

    fn establish_impl<T>(self, ae_address: AeAddr<T>) -> Result<ClientAssociation>
    where
        T: ToSocketAddrs,
    {
        let ClientAssociationOptions {
            calling_ae_title,
            called_ae_title,
            application_context_name,
            presentation_contexts,
            protocol_version,
            max_pdu_length,
            strict,
        } = self;

        // fail if no presentation contexts were provided: they represent intent,
        // should not be omitted by the user
        ensure!(
            !presentation_contexts.is_empty(),
            MissingAbstractSyntaxSnafu
        );

        // choose called AE title
        let called_ae_title: &str = match (&called_ae_title, ae_address.ae_title()) {
            (Some(aec), Some(_)) => {
                tracing::warn!(
                    "Option `called_ae_title` overrides the AE title to `{}`",
                    aec
                );
                aec
            }
            (Some(aec), None) => aec,
            (None, Some(aec)) => aec,
            (None, None) => "ANY-SCP",
        };

        let presentation_contexts: Vec<_> = presentation_contexts
            .into_iter()
            .enumerate()
            .map(|(i, presentation_context)| PresentationContextProposed {
                id: (i + 1) as u8,
                abstract_syntax: presentation_context.0.to_string(),
                transfer_syntaxes: presentation_context
                    .1
                    .iter()
                    .map(|uid| uid.to_string())
                    .collect(),
            })
            .collect();
        let msg = Pdu::AssociationRQ(AssociationRQ {
            protocol_version,
            calling_ae_title: calling_ae_title.to_string(),
            called_ae_title: called_ae_title.to_string(),
            application_context_name: application_context_name.to_string(),
            presentation_contexts,
            user_variables: vec![
                UserVariableItem::MaxLength(max_pdu_length),
                UserVariableItem::ImplementationClassUID(IMPLEMENTATION_CLASS_UID.to_string()),
                UserVariableItem::ImplementationVersionName(
                    IMPLEMENTATION_VERSION_NAME.to_string(),
                ),
            ],
        });

        let mut socket = std::net::TcpStream::connect(ae_address).context(ConnectSnafu)?;
        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        // send request

        write_pdu(&mut buffer, &msg).context(SendRequestSnafu)?;
        socket.write_all(&buffer).context(WireSendSnafu)?;
        buffer.clear();
        // receive response
        let msg =
            read_pdu(&mut socket, MAXIMUM_PDU_SIZE, self.strict).context(ReceiveResponseSnafu)?;

        match msg {
            Pdu::AssociationAC(AssociationAC {
                protocol_version: protocol_version_scp,
                application_context_name: _,
                presentation_contexts: presentation_contexts_scp,
                calling_ae_title: _,
                called_ae_title: _,
                user_variables,
            }) => {
                ensure!(
                    protocol_version == protocol_version_scp,
                    ProtocolVersionMismatchSnafu {
                        expected: protocol_version,
                        got: protocol_version_scp,
                    }
                );

                let acceptor_max_pdu_length = user_variables
                    .iter()
                    .find_map(|item| match item {
                        UserVariableItem::MaxLength(len) => Some(*len),
                        _ => None,
                    })
                    .unwrap_or(DEFAULT_MAX_PDU);

                // treat 0 as the maximum size admitted by the standard
                let acceptor_max_pdu_length = if acceptor_max_pdu_length == 0 {
                    MAXIMUM_PDU_SIZE
                } else {
                    acceptor_max_pdu_length
                };

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
                    return NoAcceptedPresentationContextsSnafu.fail();
                }
                Ok(ClientAssociation {
                    presentation_contexts,
                    requestor_max_pdu_length: max_pdu_length,
                    acceptor_max_pdu_length,
                    socket,
                    buffer,
                    strict,
                })
            }
            Pdu::AssociationRJ(association_rj) => RejectedSnafu { association_rj }.fail(),
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
                UnexpectedResponseSnafu { pdu }.fail()
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
                UnknownResponseSnafu { pdu }.fail()
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
    /// The maximum PDU length that this application entity is expecting to receive
    requestor_max_pdu_length: u32,
    /// The maximum PDU length that the remote application entity accepts
    acceptor_max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: TcpStream,
    /// Buffer to assemble PDU before sending it on wire
    buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
}

impl ClientAssociation {
    /// Retrieve the list of negotiated presentation contexts.
    pub fn presentation_contexts(&self) -> &[PresentationContextResult] {
        &self.presentation_contexts
    }

    /// Retrieve the maximum PDU length
    /// admitted by the association acceptor.
    pub fn acceptor_max_pdu_length(&self) -> u32 {
        self.acceptor_max_pdu_length
    }

    /// Retrieve the maximum PDU length
    /// that this application entity is expecting to receive.
    ///
    /// The current implementation is not required to fail
    /// and/or abort the association
    /// if a larger PDU is received.
    pub fn requestor_max_pdu_length(&self) -> u32 {
        self.requestor_max_pdu_length
    }

    /// Send a PDU message to the other intervenient.
    pub fn send(&mut self, msg: &Pdu) -> Result<()> {
        self.buffer.clear();
        write_pdu(&mut self.buffer, msg).context(SendSnafu)?;
        if self.buffer.len() > self.acceptor_max_pdu_length as usize {
            return SendTooLongPduSnafu {
                length: self.buffer.len(),
            }
            .fail();
        }
        self.socket.write_all(&self.buffer).context(WireSendSnafu)
    }

    /// Read a PDU message from the other intervenient.
    pub fn receive(&mut self) -> Result<Pdu> {
        read_pdu(&mut self.socket, self.requestor_max_pdu_length, self.strict).context(ReceiveSnafu)
    }

    /// Gracefully terminate the association by exchanging release messages
    /// and then shutting down the TCP connection.
    pub fn release(mut self) -> Result<()> {
        let out = self.release_impl();
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        out
    }
    
    /// Send an abort message and shut down the TCP connection,
    /// terminating the association.
    pub fn abort(mut self) -> Result<()> {
        let pdu = Pdu::AbortRQ {
            source: AbortRQSource::ServiceUser,
        };
        let out = self.send(&pdu);
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
            self.acceptor_max_pdu_length,
        )
    }

    /// Prepare a P-Data reader for receiving
    /// one or more data item PDUs.
    ///
    /// Returns a reader which automatically
    /// receives more data PDUs once the bytes collected are consumed.
    pub fn receive_pdata(&mut self) -> PDataReader<&mut TcpStream> {
        PDataReader::new(&mut self.socket, self.requestor_max_pdu_length)
    }

    /// Release implementation function,
    /// which tries to send a release request and receive a release response.
    /// This is in a separate private function because
    /// terminating a connection should still close the connection
    /// if the exchange fails.
    fn release_impl(&mut self) -> Result<()> {
        let pdu = Pdu::ReleaseRQ;
        self.send(&pdu)?;
        let pdu = read_pdu(&mut self.socket, self.requestor_max_pdu_length, self.strict)
            .context(ReceiveSnafu)?;

        match pdu {
            Pdu::ReleaseRP => {}
            pdu @ Pdu::AbortRQ { .. }
            | pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::AssociationRQ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRQ { .. } => return UnexpectedResponseSnafu { pdu }.fail(),
            pdu @ Pdu::Unknown { .. } => return UnknownResponseSnafu { pdu }.fail(),
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
