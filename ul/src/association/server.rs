//! Association acceptor module
//!
//! The module provides an abstraction for a DICOM association
//! in which this application entity listens to incoming association requests.
//! See [`ServerAssociationOptions`]
//! for details and examples on how to create an association.
use std::borrow::Cow;
#[cfg(not(feature = "tokio"))]
use std::{io::Write, net::TcpStream};
#[cfg(feature = "tokio")]
use tokio::{io::AsyncWriteExt, net::TcpStream};

use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use snafu::{ensure, Backtrace, ResultExt, Snafu};

use crate::{
    pdu::{
        read_pdu, write_pdu, AbortRQServiceProviderReason, AbortRQSource, AssociationAC,
        AssociationRJ, AssociationRJResult, AssociationRJServiceUserReason, AssociationRJSource,
        AssociationRQ, Pdu, PresentationContextResult, PresentationContextResultReason,
        UserIdentity, UserVariableItem, DEFAULT_MAX_PDU, MAXIMUM_PDU_SIZE,
    },
    IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME,
};

use super::{
    //pdata::{PDataReader, PDataWriter},
    uid::trim_uid,
};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// missing at least one abstract syntax to accept negotiations
    MissingAbstractSyntax { backtrace: Backtrace },

    /// failed to receive association request
    ReceiveRequest {
        #[snafu(backtrace)]
        source: crate::pdu::ReadError,
    },

    /// failed to send association response
    SendResponse {
        #[snafu(backtrace)]
        source: crate::pdu::WriteError,
    },

    /// failed to prepare PDU
    Send {
        #[snafu(backtrace)]
        source: crate::pdu::WriteError,
    },

    /// failed to send PDU over the wire
    WireSend {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    /// failed to receive PDU
    Receive {
        #[snafu(backtrace)]
        source: crate::pdu::ReadError,
    },

    #[snafu(display("unexpected request from SCU `{:?}`", pdu))]
    #[non_exhaustive]
    UnexpectedRequest {
        /// the PDU obtained from the server
        pdu: Box<Pdu>,
    },

    #[snafu(display("unknown request from SCU `{:?}`", pdu))]
    #[non_exhaustive]
    UnknownRequest {
        /// the PDU obtained from the server, of variant Unknown
        pdu: Box<Pdu>,
    },

    /// association rejected
    Rejected { backtrace: Backtrace },

    /// association aborted
    Aborted { backtrace: Backtrace },

    #[snafu(display(
        "PDU is too large ({} bytes) to be sent to the remote application entity",
        length
    ))]
    #[non_exhaustive]
    SendTooLongPdu { length: usize, backtrace: Backtrace },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Common interface for application entity access control policies.
///
/// Existing implementations include [`AcceptAny`] and [`AcceptCalledAeTitle`],
/// but users are free to implement their own.
pub trait AccessControl {
    /// Obtain the decision of whether to accept an incoming association request
    /// based on the recorded application entity titles and/or user identity.
    ///
    /// Returns Ok(()) if the requester node should be given clearance.
    /// Otherwise, a concrete association RJ service user reason is given.
    fn check_access(
        &self,
        this_ae_title: &str,
        calling_ae_title: &str,
        called_ae_title: &str,
        user_identity: Option<&UserIdentity>,
    ) -> Result<(), AssociationRJServiceUserReason>;
}

/// An access control rule that accepts any incoming association request.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
pub struct AcceptAny;

impl AccessControl for AcceptAny {
    fn check_access(
        &self,
        _this_ae_title: &str,
        _calling_ae_title: &str,
        _called_ae_title: &str,
        _user_identity: Option<&UserIdentity>,
    ) -> Result<(), AssociationRJServiceUserReason> {
        Ok(())
    }
}

/// An access control rule that accepts association requests
/// that match the called AE title with the node's AE title.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
pub struct AcceptCalledAeTitle;

impl AccessControl for AcceptCalledAeTitle {
    fn check_access(
        &self,
        this_ae_title: &str,
        _calling_ae_title: &str,
        called_ae_title: &str,
        _user_identity: Option<&UserIdentity>,
    ) -> Result<(), AssociationRJServiceUserReason> {
        if this_ae_title == called_ae_title {
            Ok(())
        } else {
            Err(AssociationRJServiceUserReason::CalledAETitleNotRecognized)
        }
    }
}

/// A DICOM association builder for an acceptor DICOM node,
/// often taking the role of a service class provider (SCP).
///
/// This is the standard way of negotiating and establishing
/// an association with a requesting node.
/// The outcome is a [`ServerAssociation`].
/// Unlike the [`ClientAssociationOptions`],
/// a value of this type can be reused for multiple connections.
///
/// [`ClientAssociationOptions`]: crate::association::ClientAssociationOptions
///
/// # Example
///
/// ```no_run
/// # use std::net::TcpListener;
/// # use dicom_ul::association::server::ServerAssociationOptions;
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// # let tcp_listener: TcpListener = unimplemented!();
/// let scp_options = ServerAssociationOptions::new()
///    .with_abstract_syntax("1.2.840.10008.1.1")
///    .with_transfer_syntax("1.2.840.10008.1.2.1");
///
/// let (stream, _address) = tcp_listener.accept()?;
/// scp_options.establish(stream)?;
/// # Ok(())
/// # }
/// ```
///
/// The SCP will by default accept all transfer syntaxes
/// supported by the main [transfer syntax registry][1],
/// unless one or more transfer syntaxes are explicitly indicated
/// through calls to [`with_transfer_syntax`][2].
///
/// Access control logic is also available,
/// enabling application entities to decide on
/// whether to accept or reject the association request
/// based on the _called_ and _calling_ AE titles.
///
/// - By default, the application will accept requests from anyone
///   ([`AcceptAny`])
/// - To only accept requests with a matching _called_ AE title,
///   add a call to [`accept_called_ae_title`]
///   ([`AcceptCalledAeTitle`]).
/// - Any other policy can be implemented through the [`AccessControl`] trait.
///
/// [`accept_called_ae_title`]: Self::accept_called_ae_title
/// [`AcceptAny`]: AcceptAny
/// [`AcceptCalledAeTitle`]: AcceptCalledAeTitle
/// [`AccessControl`]: AccessControl
///
/// [1]: dicom_transfer_syntax_registry
/// [2]: ServerAssociationOptions::with_transfer_syntax
#[derive(Debug, Clone)]
pub struct ServerAssociationOptions<'a, A> {
    /// the application entity access control policy
    ae_access_control: A,
    /// the AE title of this DICOM node
    ae_title: Cow<'a, str>,
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
    /// whether to receive PDUs in strict mode
    strict: bool,
    /// whether to accept unknown abstract syntaxes
    promiscuous: bool,
}

impl<'a> Default for ServerAssociationOptions<'a, AcceptAny> {
    fn default() -> Self {
        ServerAssociationOptions {
            ae_access_control: AcceptAny,
            ae_title: "THIS-SCP".into(),
            application_context_name: "1.2.840.10008.3.1.1.1".into(),
            abstract_syntax_uids: Vec::new(),
            transfer_syntax_uids: Vec::new(),
            protocol_version: 1,
            max_pdu_length: DEFAULT_MAX_PDU,
            strict: true,
            promiscuous: false,
        }
    }
}

impl<'a> ServerAssociationOptions<'a, AcceptAny> {
    /// Create a new set of options for establishing an association.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<'a, A> ServerAssociationOptions<'a, A>
where
    A: AccessControl,
{
    /// Change the access control policy to accept any association
    /// regardless of the specified AE titles.
    ///
    /// This is the default behavior when the options are first created.
    pub fn accept_any(self) -> ServerAssociationOptions<'a, AcceptAny> {
        self.ae_access_control(AcceptAny)
    }

    /// Change the access control policy to accept an association
    /// if the called AE title matches this node's AE title.
    ///
    /// The default is to accept any requesting node
    /// regardless of the specified AE titles.
    pub fn accept_called_ae_title(self) -> ServerAssociationOptions<'a, AcceptCalledAeTitle> {
        self.ae_access_control(AcceptCalledAeTitle)
    }

    /// Change the access control policy.
    ///
    /// The default is to accept any requesting node
    /// regardless of the specified AE titles.
    pub fn ae_access_control<P>(self, access_control: P) -> ServerAssociationOptions<'a, P>
    where
        P: AccessControl,
    {
        let ServerAssociationOptions {
            ae_title,
            application_context_name,
            abstract_syntax_uids,
            transfer_syntax_uids,
            protocol_version,
            max_pdu_length,
            strict,
            promiscuous,
            ae_access_control: _,
        } = self;

        ServerAssociationOptions {
            ae_access_control: access_control,
            ae_title,
            application_context_name,
            abstract_syntax_uids,
            transfer_syntax_uids,
            protocol_version,
            max_pdu_length,
            strict,
            promiscuous,
        }
    }

    /// Define the application entity title referring to this DICOM node.
    ///
    /// The default is `THIS-SCP`.
    pub fn ae_title<T>(mut self, ae_title: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.ae_title = ae_title.into();
        self
    }

    /// Include this abstract syntax
    /// in the list of proposed presentation contexts.
    pub fn with_abstract_syntax<T>(mut self, abstract_syntax_uid: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.abstract_syntax_uids
            .push(trim_uid(abstract_syntax_uid.into()));
        self
    }

    /// Include this transfer syntax in each proposed presentation context.
    pub fn with_transfer_syntax<T>(mut self, transfer_syntax_uid: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        self.transfer_syntax_uids
            .push(trim_uid(transfer_syntax_uid.into()));
        self
    }

    /// Override the maximum expected PDU length.
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

    /// Override promiscuous mode:
    /// whether to accept unknown abstract syntaxes.
    pub fn promiscuous(mut self, promiscuous: bool) -> Self {
        self.promiscuous = promiscuous;
        self
    }

    #[cfg(not(feature = "tokio"))]
    /// Negotiate an association with the given TCP stream.
    pub fn establish(&self, mut socket: TcpStream) -> Result<ServerAssociation> {
        ensure!(
            !self.abstract_syntax_uids.is_empty() || self.promiscuous,
            MissingAbstractSyntaxSnafu
        );

        let max_pdu_length = self.max_pdu_length;

        let pdu =
            read_pdu(&mut socket, max_pdu_length, self.strict).context(ReceiveRequestSnafu)?;
        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        match pdu {
            Pdu::AssociationRQ(AssociationRQ {
                protocol_version,
                calling_ae_title,
                called_ae_title,
                application_context_name,
                presentation_contexts,
                user_variables,
            }) => {
                if protocol_version != self.protocol_version {
                    write_pdu(
                        &mut buffer,
                        &Pdu::AssociationRJ(AssociationRJ {
                            result: AssociationRJResult::Permanent,
                            source: AssociationRJSource::ServiceUser(
                                AssociationRJServiceUserReason::NoReasonGiven,
                            ),
                        }),
                    )
                    .context(SendResponseSnafu)?;
                    socket.write_all(&buffer).context(WireSendSnafu)?;
                    return RejectedSnafu.fail();
                }

                if application_context_name != self.application_context_name {
                    write_pdu(
                        &mut buffer,
                        &Pdu::AssociationRJ(AssociationRJ {
                            result: AssociationRJResult::Permanent,
                            source: AssociationRJSource::ServiceUser(
                                AssociationRJServiceUserReason::ApplicationContextNameNotSupported,
                            ),
                        }),
                    )
                    .context(SendResponseSnafu)?;
                    socket.write_all(&buffer).context(WireSendSnafu)?;
                    return RejectedSnafu.fail();
                }

                self.ae_access_control
                    .check_access(
                        &self.ae_title,
                        &calling_ae_title,
                        &called_ae_title,
                        user_variables
                            .iter()
                            .find_map(|user_variable| match user_variable {
                                UserVariableItem::UserIdentityItem(user_identity) => {
                                    Some(user_identity)
                                }
                                _ => None,
                            }),
                    )
                    .map(Ok)
                    .unwrap_or_else(|reason| {
                        write_pdu(
                            &mut buffer,
                            &Pdu::AssociationRJ(AssociationRJ {
                                result: AssociationRJResult::Permanent,
                                source: AssociationRJSource::ServiceUser(reason),
                            }),
                        )
                        .context(SendResponseSnafu)?;
                        socket.write_all(&buffer).context(WireSendSnafu)?;
                        RejectedSnafu.fail()
                    })?;

                // fetch requested maximum PDU length
                let requestor_max_pdu_length = user_variables
                    .iter()
                    .find_map(|item| match item {
                        UserVariableItem::MaxLength(len) => Some(*len),
                        _ => None,
                    })
                    .unwrap_or(DEFAULT_MAX_PDU);

                // treat 0 as the maximum size admitted by the standard
                let requestor_max_pdu_length = if requestor_max_pdu_length == 0 {
                    MAXIMUM_PDU_SIZE
                } else {
                    requestor_max_pdu_length
                };

                let presentation_contexts: Vec<_> = presentation_contexts
                    .into_iter()
                    .map(|pc| {
                        if !self
                            .abstract_syntax_uids
                            .contains(&trim_uid(Cow::from(pc.abstract_syntax)))
                            && !self.promiscuous
                        {
                            return PresentationContextResult {
                                id: pc.id,
                                reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                                transfer_syntax: "1.2.840.10008.1.2".to_string(),
                            };
                        }

                        let (transfer_syntax, reason) = self
                            .choose_ts(pc.transfer_syntaxes)
                            .map(|ts| (ts, PresentationContextResultReason::Acceptance))
                            .unwrap_or_else(|| {
                                (
                                    "1.2.840.10008.1.2".to_string(),
                                    PresentationContextResultReason::TransferSyntaxesNotSupported,
                                )
                            });

                        PresentationContextResult {
                            id: pc.id,
                            reason,
                            transfer_syntax,
                        }
                    })
                    .collect();

                write_pdu(
                    &mut buffer,
                    &Pdu::AssociationAC(AssociationAC {
                        protocol_version: self.protocol_version,
                        application_context_name,
                        presentation_contexts: presentation_contexts.clone(),
                        calling_ae_title: calling_ae_title.clone(),
                        called_ae_title,
                        user_variables: vec![
                            UserVariableItem::MaxLength(max_pdu_length),
                            UserVariableItem::ImplementationClassUID(
                                IMPLEMENTATION_CLASS_UID.to_string(),
                            ),
                            UserVariableItem::ImplementationVersionName(
                                IMPLEMENTATION_VERSION_NAME.to_string(),
                            ),
                        ],
                    }),
                )
                .context(SendResponseSnafu)?;
                socket.write_all(&buffer).context(WireSendSnafu)?;

                Ok(ServerAssociation {
                    presentation_contexts,
                    requestor_max_pdu_length,
                    acceptor_max_pdu_length: max_pdu_length,
                    socket,
                    client_ae_title: calling_ae_title,
                    buffer,
                    strict: self.strict,
                })
            }
            Pdu::ReleaseRQ => {
                write_pdu(&mut buffer, &Pdu::ReleaseRP).context(SendResponseSnafu)?;
                socket.write_all(&buffer).context(WireSendSnafu)?;
                AbortedSnafu.fail()
            }
            pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRP
            | pdu @ Pdu::AbortRQ { .. } => UnexpectedRequestSnafu { pdu }.fail(),
            pdu @ Pdu::Unknown { .. } => UnknownRequestSnafu { pdu }.fail(),
        }
    }

    #[cfg(feature = "tokio")]
    /// Negotiate an association with the given TCP stream.
    pub async fn establish(&self, mut socket: TcpStream) -> Result<ServerAssociation> {
        ensure!(
            !self.abstract_syntax_uids.is_empty() || self.promiscuous,
            MissingAbstractSyntaxSnafu
        );

        let max_pdu_length = self.max_pdu_length;

        let pdu = read_pdu(&mut socket, max_pdu_length, self.strict)
            .await
            .context(ReceiveRequestSnafu)?;
        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        match pdu {
            Pdu::AssociationRQ(AssociationRQ {
                protocol_version,
                calling_ae_title,
                called_ae_title,
                application_context_name,
                presentation_contexts,
                user_variables,
            }) => {
                if protocol_version != self.protocol_version {
                    write_pdu(
                        &mut buffer,
                        &Pdu::AssociationRJ(AssociationRJ {
                            result: AssociationRJResult::Permanent,
                            source: AssociationRJSource::ServiceUser(
                                AssociationRJServiceUserReason::NoReasonGiven,
                            ),
                        }),
                    )
                    .await
                    .context(SendResponseSnafu)?;
                    socket.write_all(&buffer).await.context(WireSendSnafu)?;
                    return RejectedSnafu.fail();
                }

                if application_context_name != self.application_context_name {
                    write_pdu(
                        &mut buffer,
                        &Pdu::AssociationRJ(AssociationRJ {
                            result: AssociationRJResult::Permanent,
                            source: AssociationRJSource::ServiceUser(
                                AssociationRJServiceUserReason::ApplicationContextNameNotSupported,
                            ),
                        }),
                    )
                    .await
                    .context(SendResponseSnafu)?;
                    socket.write_all(&buffer).await.context(WireSendSnafu)?;
                    return RejectedSnafu.fail();
                }

                self.ae_access_control
                    .check_access(
                        &self.ae_title,
                        &calling_ae_title,
                        &called_ae_title,
                        user_variables
                            .iter()
                            .find_map(|user_variable| match user_variable {
                                UserVariableItem::UserIdentityItem(user_identity) => {
                                    Some(user_identity)
                                }
                                _ => None,
                            }),
                    )
                    .map(Ok)
                    .unwrap_or_else(|reason| {
                        async {
                            write_pdu(
                                &mut buffer,
                                &Pdu::AssociationRJ(AssociationRJ {
                                    result: AssociationRJResult::Permanent,
                                    source: AssociationRJSource::ServiceUser(reason),
                                }),
                            )
                            .await
                            .context(SendResponseSnafu)?;
                            socket.write_all(&buffer).await.context(WireSendSnafu)?;
                            return Err::<(), Error>(RejectedSnafu.build());
                        };
                        Ok(())
                    })?;

                // fetch requested maximum PDU length
                let requestor_max_pdu_length = user_variables
                    .iter()
                    .find_map(|item| match item {
                        UserVariableItem::MaxLength(len) => Some(*len),
                        _ => None,
                    })
                    .unwrap_or(DEFAULT_MAX_PDU);

                // treat 0 as the maximum size admitted by the standard
                let requestor_max_pdu_length = if requestor_max_pdu_length == 0 {
                    MAXIMUM_PDU_SIZE
                } else {
                    requestor_max_pdu_length
                };

                let presentation_contexts: Vec<_> = presentation_contexts
                    .into_iter()
                    .map(|pc| {
                        if !self
                            .abstract_syntax_uids
                            .contains(&trim_uid(Cow::from(pc.abstract_syntax)))
                            && !self.promiscuous
                        {
                            return PresentationContextResult {
                                id: pc.id,
                                reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                                transfer_syntax: "1.2.840.10008.1.2".to_string(),
                            };
                        }

                        let (transfer_syntax, reason) = self
                            .choose_ts(pc.transfer_syntaxes)
                            .map(|ts| (ts, PresentationContextResultReason::Acceptance))
                            .unwrap_or_else(|| {
                                (
                                    "1.2.840.10008.1.2".to_string(),
                                    PresentationContextResultReason::TransferSyntaxesNotSupported,
                                )
                            });

                        PresentationContextResult {
                            id: pc.id,
                            reason,
                            transfer_syntax,
                        }
                    })
                    .collect();

                write_pdu(
                    &mut buffer,
                    &Pdu::AssociationAC(AssociationAC {
                        protocol_version: self.protocol_version,
                        application_context_name,
                        presentation_contexts: presentation_contexts.clone(),
                        calling_ae_title: calling_ae_title.clone(),
                        called_ae_title,
                        user_variables: vec![
                            UserVariableItem::MaxLength(max_pdu_length),
                            UserVariableItem::ImplementationClassUID(
                                IMPLEMENTATION_CLASS_UID.to_string(),
                            ),
                            UserVariableItem::ImplementationVersionName(
                                IMPLEMENTATION_VERSION_NAME.to_string(),
                            ),
                        ],
                    }),
                )
                .await
                .context(SendResponseSnafu)?;
                socket.write_all(&buffer).await.context(WireSendSnafu)?;

                Ok(ServerAssociation {
                    presentation_contexts,
                    requestor_max_pdu_length,
                    acceptor_max_pdu_length: max_pdu_length,
                    socket,
                    client_ae_title: calling_ae_title,
                    buffer,
                    strict: self.strict,
                })
            }
            Pdu::ReleaseRQ => {
                write_pdu(&mut buffer, &Pdu::ReleaseRP)
                    .await
                    .context(SendResponseSnafu)?;
                socket.write_all(&buffer).await.context(WireSendSnafu)?;
                AbortedSnafu.fail()
            }
            pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRP
            | pdu @ Pdu::AbortRQ { .. } => UnexpectedRequestSnafu { pdu }.fail(),
            pdu @ Pdu::Unknown { .. } => UnknownRequestSnafu { pdu }.fail(),
        }
    }

    /// From a sequence of transfer syntaxes,
    /// choose the first transfer syntax to
    /// - be on the options' list of transfer syntaxes, and
    /// - be supported by the main transfer syntax registry.
    ///
    /// If the options' list is empty,
    /// accept the first transfer syntax supported.
    fn choose_ts<I, T>(&self, it: I) -> Option<T>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        if self.transfer_syntax_uids.is_empty() {
            return choose_supported(it);
        }

        it.into_iter().find(|ts| {
            let ts = ts.as_ref();
            if self.transfer_syntax_uids.is_empty() {
                ts.trim_end_matches(|c: char| c.is_whitespace() || c == '\0') == "1.2.840.10008.1.2"
            } else {
                self.transfer_syntax_uids.contains(&trim_uid(ts.into())) && is_supported(ts)
            }
        })
    }
}

/// A DICOM upper level association from the perspective
/// of an accepting application entity.
///
/// The most common operations of an established association are
/// [`send`](Self::send)
/// and [`receive`](Self::receive).
/// Sending large P-Data fragments may be easier through the P-Data sender
/// abstraction (see [`send_pdata`](Self::send_pdata)).
///
/// When the value falls out of scope,
/// the program will shut down the underlying TCP connection.
#[derive(Debug)]
pub struct ServerAssociation {
    /// The accorded presentation contexts
    presentation_contexts: Vec<PresentationContextResult>,
    /// The maximum PDU length that the remote application entity accepts
    requestor_max_pdu_length: u32,
    /// The maximum PDU length that this application entity is expecting to receive
    acceptor_max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: TcpStream,
    /// The application entity title of the other DICOM node
    client_ae_title: String,
    /// write buffer to send fully assembled PDUs on wire
    buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
}

impl ServerAssociation {
    /// Obtain a view of the negotiated presentation contexts.
    pub fn presentation_contexts(&self) -> &[PresentationContextResult] {
        &self.presentation_contexts
    }

    /// Obtain the remote DICOM node's application entity title.
    pub fn client_ae_title(&self) -> &str {
        &self.client_ae_title
    }

    #[cfg(not(feature = "tokio"))]
    /// Send a PDU message to the other intervenient.
    pub fn send(&mut self, msg: &Pdu) -> Result<()> {
        self.buffer.clear();
        write_pdu(&mut self.buffer, msg).context(SendSnafu)?;
        if self.buffer.len() > self.requestor_max_pdu_length as usize {
            return SendTooLongPduSnafu {
                length: self.buffer.len(),
            }
            .fail();
        }
        self.socket.write_all(&self.buffer).context(WireSendSnafu)
    }

    #[cfg(feature = "tokio")]
    /// Send a PDU message to the other intervenient.
    pub async fn send(&mut self, msg: &Pdu) -> Result<()> {
        self.buffer.clear();
        write_pdu(&mut self.buffer, msg).await.context(SendSnafu)?;
        if self.buffer.len() > self.requestor_max_pdu_length as usize {
            return SendTooLongPduSnafu {
                length: self.buffer.len(),
            }
            .fail();
        }
        self.socket
            .write_all(&self.buffer)
            .await
            .context(WireSendSnafu)
    }

    #[cfg(not(feature = "tokio"))]
    /// Read a PDU message from the other intervenient.
    pub fn receive(&mut self) -> Result<Pdu> {
        read_pdu(&mut self.socket, self.acceptor_max_pdu_length, self.strict).context(ReceiveSnafu)
    }

    #[cfg(feature = "tokio")]
    /// Read a PDU message from the other intervenient.
    pub async fn receive(&mut self) -> Result<Pdu> {
        read_pdu(&mut self.socket, self.acceptor_max_pdu_length, self.strict)
            .await
            .context(ReceiveSnafu)
    }

    /// Send a provider initiated abort message
    /// and shut down the TCP connection,
    /// terminating the association.
    #[cfg(not(feature = "tokio"))]
    pub fn abort(mut self) -> Result<()> {
        let pdu = Pdu::AbortRQ {
            source: AbortRQSource::ServiceProvider(
                AbortRQServiceProviderReason::ReasonNotSpecified,
            ),
        };
        let out = self.send(&pdu);
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        out
    }

    /// Send a provider initiated abort message
    /// and shut down the TCP connection,
    /// terminating the association.
    #[cfg(feature = "tokio")]
    pub async fn abort(mut self) -> Result<()> {
        let pdu = Pdu::AbortRQ {
            source: AbortRQSource::ServiceProvider(
                AbortRQServiceProviderReason::ReasonNotSpecified,
            ),
        };
        let out = self.send(&pdu).await;
        let _ = self.socket.shutdown().await;
        out
    }

    /// Prepare a P-Data writer for sending
    /// one or more data item PDUs.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    #[cfg(not(feature = "tokio"))]
    pub fn send_pdata(&mut self, presentation_context_id: u8) -> PDataWriter<&mut TcpStream> {
        PDataWriter::new(
            &mut self.socket,
            presentation_context_id,
            self.requestor_max_pdu_length,
        )
    }

    /// Prepare a P-Data writer for sending
    /// one or more data item PDUs.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    // #[cfg(feature = "tokio")]
    // pub fn send_pdata(&mut self, presentation_context_id: u8) -> PDataWriter<&mut TcpStream> {
    //     PDataWriter::new(
    //         &mut self.socket,
    //         presentation_context_id,
    //         self.requestor_max_pdu_length,
    //     )
    // }

    /// Prepare a P-Data reader for receiving
    /// one or more data item PDUs.
    ///
    /// Returns a reader which automatically
    /// receives more data PDUs once the bytes collected are consumed.
    // pub fn receive_pdata(&mut self) -> PDataReader<&mut TcpStream> {
    //     PDataReader::new(&mut self.socket, self.acceptor_max_pdu_length)
    // }

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
}

/// Check that a transfer syntax repository
/// supports the given transfer syntax,
/// meaning that it can parse and decode DICOM data sets.
///
/// ```
/// # use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
/// # use dicom_ul::association::server::is_supported_with_repo;
/// // Implicit VR Little Endian is guaranteed to be supported
/// assert!(is_supported_with_repo(TransferSyntaxRegistry, "1.2.840.10008.1.2"));
/// ```
pub fn is_supported_with_repo<R>(ts_repo: R, ts_uid: &str) -> bool
where
    R: TransferSyntaxIndex,
{
    ts_repo
        .get(ts_uid)
        .filter(|ts| !ts.is_unsupported())
        .is_some()
}

/// Check that the main transfer syntax registry
/// supports the given transfer syntax,
/// meaning that it can parse and decode DICOM data sets.
///
/// ```
/// # use dicom_ul::association::server::is_supported;
/// // Implicit VR Little Endian is guaranteed to be supported
/// assert!(is_supported("1.2.840.10008.1.2"));
/// ```
pub fn is_supported(ts_uid: &str) -> bool {
    is_supported_with_repo(TransferSyntaxRegistry, ts_uid)
}

/// From a sequence of transfer syntaxes,
/// choose the first transfer syntax to be supported
/// by the given transfer syntax repository.
pub fn choose_supported_with_repo<R, I, T>(ts_repo: R, it: I) -> Option<T>
where
    R: TransferSyntaxIndex,
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    it.into_iter()
        .find(|ts| is_supported_with_repo(&ts_repo, ts.as_ref()))
}

/// From a sequence of transfer syntaxes,
/// choose the first transfer syntax to be supported
/// by the main transfer syntax registry.
pub fn choose_supported<I, T>(it: I) -> Option<T>
where
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    it.into_iter().find(|ts| is_supported(ts.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::choose_supported;

    #[test]
    fn test_choose_supported() {
        assert_eq!(choose_supported(vec!["1.1.1.1.1"]), None,);

        // string slices, impl VR first
        assert_eq!(
            choose_supported(vec!["1.2.840.10008.1.2", "1.2.840.10008.1.2.1"]),
            Some("1.2.840.10008.1.2"),
        );

        // heap allocated strings slices, expl VR first
        assert_eq!(
            choose_supported(vec![
                "1.2.840.10008.1.2.1".to_string(),
                "1.2.840.10008.1.2".to_string()
            ]),
            Some("1.2.840.10008.1.2.1".to_string()),
        );
    }
}
