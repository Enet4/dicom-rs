//! Association acceptor module
use std::{
    borrow::Cow,
    net::{TcpStream, ToSocketAddrs},
};

use crate::pdu::{
    reader::read_pdu, writer::write_pdu, AssociationRJResult, AssociationRJSource, Pdu,
    PresentationContextProposed, PresentationContextResultReason,
};
use snafu::{ensure, OptionExt, ResultExt, Snafu};

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

    /// failed to receive PDU message
    #[non_exhaustive]
    Receive { source: crate::pdu::reader::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A DICOM association builder for client node.
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
///
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let association = ClientAssociationOptions::new()
///    .with_abstract_syntax("1.2.840.10008.1.1")
///    .with_transfer_syntax("1.2.840.10008.1.2.1")
///    .establish("129.168.0.5:104")?;
/// # Ok(())
/// # }
/// ```
///
/// The SCU will admit by default the transfer syntaxes
/// _Implicit VR Little Endian_
/// and _Explicit VR Little Endian_.
/// Other transfer syntaxes can be requested in the association
/// via the method `with_transfer_syntax`.
///
#[derive(Debug, Clone)]
pub struct ClientAssociationOptions {
    /// the calling AE title
    calling_ae_title: Cow<'static, str>,
    /// the called AE title
    called_ae_title: Cow<'static, str>,
    /// the requested application context name
    application_context_name: Cow<'static, str>,
    /// the list of requested abstract syntaxes
    abstract_syntax_uids: Vec<Cow<'static, str>>,
    /// the list of requested transfer syntaxes
    transfer_syntax_uids: Vec<Cow<'static, str>>,
    /// the expected protocol version
    protocol_version: u16,
    /// the maximum PDU length
    max_pdu_length: u32,
}

impl Default for ClientAssociationOptions {
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

impl ClientAssociationOptions {
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
        T: Into<Cow<'static, str>>,
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
        T: Into<Cow<'static, str>>,
    {
        self.called_ae_title = called_ae_title.into();
        self
    }

    /// Include this abstract syntax
    /// in the list of proposed presentation contexts.
    pub fn with_abstract_syntax<T>(mut self, abstract_syntax_uid: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.abstract_syntax_uids.push(abstract_syntax_uid.into());
        self
    }

    /// Include this transfer syntax in each proposed presentation context.
    pub fn with_transfer_syntax<T>(mut self, transfer_syntax_uid: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.transfer_syntax_uids.push(transfer_syntax_uid.into());
        self
    }

    /// Override the maximum expected PDU length.
    pub fn max_pdu_length(mut self, value: u32) -> Self {
        self.max_pdu_length = value;
        self
    }

    /// Initiate the TCP connection and negotiate the
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

        // send request
        write_pdu(&mut socket, &msg).context(SendRequest)?;

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

                let selected_context = presentation_contexts_scp
                    .into_iter()
                    .find(|c| c.reason == PresentationContextResultReason::Acceptance)
                    .context(NoAcceptedPresentationContexts)?;

                let presentation_context = presentation_contexts
                    .into_iter()
                    .find(|c| c.id == selected_context.id)
                    .context(NoAcceptedPresentationContexts)?;

                Ok(ClientAssociation {
                    presentation_context_id: selected_context.id,
                    abstract_syntax_uid: presentation_context.abstract_syntax,
                    transfer_syntax_uid: selected_context.transfer_syntax,
                    max_pdu_length,
                    socket,
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
            | pdu @ Pdu::ReleaseRP { .. } => UnexpectedResponse { pdu }.fail(),
            pdu @ Pdu::Unknown { .. } => UnknownResponse { pdu }.fail(),
        }
    }
}

/// A DICOM upper level association from the perspective
/// of an association requester.
#[derive(Debug)]
pub struct ClientAssociation {
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

impl ClientAssociation {
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

impl Drop for ClientAssociation {
    fn drop(&mut self) {
        let _ = self.release();
    }
}