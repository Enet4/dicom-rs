//! Association acceptor module
//!
//! The module provides an abstraction for a DICOM association
//! in which this application entity listens to incoming association requests.
//! See [`ServerAssociationOptions`]
//! for details and examples on how to create an association.
use bytes::BytesMut;
use std::time::Duration;
use std::borrow::Cow;
use std::{io::Write, net::TcpStream};

use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use snafu::{ensure, ResultExt};
use crate::association::{
    read_pdu_from_wire, AbortedSnafu,
    MissingAbstractSyntaxSnafu, RejectedSnafu, SendPduSnafu, 
    SendTooLongPduSnafu, SetReadTimeoutSnafu, SetWriteTimeoutSnafu, 
    UnexpectedPduSnafu, UnknownPduSnafu, WireSendSnafu
};

use crate::association::NegotiatedOptions;
use crate::pdu::PresentationContextNegotiated;
use crate::{
    pdu::{
        write_pdu, AbortRQServiceProviderReason, AbortRQSource, AssociationAC,
        AssociationRJ, AssociationRJResult, AssociationRJServiceUserReason, AssociationRJSource,
        AssociationRQ, Pdu, PresentationContextResult, PresentationContextResultReason,
        UserIdentity, UserVariableItem, DEFAULT_MAX_PDU, MAXIMUM_PDU_SIZE,
    },
    IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME,
};

use super::{
    pdata::{PDataReader, PDataWriter},
    uid::trim_uid,
    Error
};

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
/// ## Basic Usage
///
/// ### Sync
///
/// Spawn a single sync thread to listen for incoming requests.
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
/// ### Async
///
/// Spawn an async task for each incoming association request.
///
/// ```no_run
/// # use std::net::{Ipv4Addr, SocketAddrV4};
/// # use dicom_ul::association::server::ServerAssociationOptions;
/// # #[cfg(feature = "async")]
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), 11111);
/// let listener = tokio::net::TcpListener::bind(listen_addr).await?;
/// loop {
///     let (socket, _addr) = listener.accept().await?;
///     tokio::task::spawn(async move {
///         let mut scp = ServerAssociationOptions::new()
///             .accept_any()
///             .with_abstract_syntax("1.2.840.10008.1.1")
///             .with_transfer_syntax("1.2.840.10008.1.2.1")
///             .establish_async(socket)
///             .await
///             .expect("Could not establish association on socket");
///         loop {
///             match scp.receive().await {
///                 Ok(dicom_ul::Pdu::PData { data }) => {
///                     // read P-Data here
///                 },
///                 Ok(dicom_ul::Pdu::ReleaseRP) => {
///                     break;
///                 },
///                 Ok(dicom_ul::Pdu::AbortRQ { source }) => {
///                     eprintln!("Association aborted: {source:?}");
///                     break;
///                 },
///                 Ok(pdu) => {
///                     eprintln!("Unexpected PDU");
///                 },
///                 Err(e) => {
///                     eprintln!("Oops! {e}");
///                 },
///             }
///         }
///     });
/// }
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "async"))]
/// fn main() {}
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
    /// TCP read timeout
    read_timeout: Option<Duration>,
    /// TCP write timeout
    write_timeout: Option<Duration>,
}

impl Default for ServerAssociationOptions<'_, AcceptAny> {
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
            read_timeout: None,
            write_timeout: None,
        }
    }
}

impl ServerAssociationOptions<'_, AcceptAny> {
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
            read_timeout,
            write_timeout,
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
            read_timeout,
            write_timeout,
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

    /// Set the read timeout for the underlying TCP socket
    ///
    /// This is used to set both the read and write timeout.
    pub fn read_timeout(self, timeout: Duration) -> Self {
        Self {
            read_timeout: Some(timeout),
            ..self
        }
    }

    /// Set the write timeout for the underlying TCP socket
    pub fn write_timeout(self, timeout: Duration) -> Self {
        Self {
            write_timeout: Some(timeout),
            ..self
        }
    }

    /// Process an association request PDU
    /// 
    /// In the success case, returns
    /// * Pdu to be written back to client
    /// * Negotiated options
    /// * Calling AE title
    /// 
    /// In the error case, returns
    /// * Pdu to be written back to client
    /// * Error
    #[allow(clippy::result_large_err)]
    fn process_a_association_rq(&self, msg: Pdu) -> std::result::Result<(Pdu, NegotiatedOptions, String),(Pdu, Error)>{
        match msg {
            Pdu::AssociationRQ(AssociationRQ {
                protocol_version,
                calling_ae_title,
                called_ae_title,
                application_context_name,
                presentation_contexts,
                user_variables,
            }) => {
                if protocol_version != self.protocol_version {
                    let association_rj= AssociationRJ {
                        result: AssociationRJResult::Permanent,
                        source: AssociationRJSource::ServiceUser(
                            AssociationRJServiceUserReason::NoReasonGiven,
                        ),
                    };
                    let pdu = Pdu::AssociationRJ(association_rj.clone());
                    return Err((pdu, RejectedSnafu{association_rj}.build()));
                }

                if application_context_name != self.application_context_name {
                    let association_rj = AssociationRJ {
                        result: AssociationRJResult::Permanent,
                        source: AssociationRJSource::ServiceUser(
                            AssociationRJServiceUserReason::ApplicationContextNameNotSupported,
                        ),
                    };
                    let pdu = Pdu::AssociationRJ(association_rj.clone());
                    return Err((pdu, RejectedSnafu{association_rj}.build()));
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
                        let association_rj = AssociationRJ {
                            result: AssociationRJResult::Permanent,
                            source: AssociationRJSource::ServiceUser(reason),
                        };
                        let pdu = Pdu::AssociationRJ(association_rj.clone());
                        Err((pdu, RejectedSnafu{ association_rj}.build()))
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

                let presentation_contexts_negotiated: Vec<_> = presentation_contexts
                    .into_iter()
                    .map(|pc| {
                        let abstract_syntax = trim_uid(Cow::from(pc.abstract_syntax));
                        if !self
                            .abstract_syntax_uids
                            .contains(&abstract_syntax)
                            && !self.promiscuous
                        {
                            return PresentationContextNegotiated {
                                id: pc.id,
                                reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                                transfer_syntax: "1.2.840.10008.1.2".to_string(),
                                abstract_syntax: abstract_syntax.to_string(),
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

                        PresentationContextNegotiated {
                            id: pc.id,
                            reason,
                            transfer_syntax,
                            abstract_syntax: abstract_syntax.to_string(),
                        }
                    })
                    .collect();

                let pdu = Pdu::AssociationAC(AssociationAC {
                    protocol_version: self.protocol_version,
                    application_context_name,
                    presentation_contexts: presentation_contexts_negotiated
                        .iter()
                        .map(|pc| PresentationContextResult {
                            id: pc.id,
                            reason: pc.reason.clone(),
                            transfer_syntax: pc.transfer_syntax.clone(),
                        })
                        .collect(),
                    calling_ae_title: calling_ae_title.clone(),
                    called_ae_title,
                    user_variables: vec![
                        UserVariableItem::MaxLength(self.max_pdu_length),
                        UserVariableItem::ImplementationClassUID(
                            IMPLEMENTATION_CLASS_UID.to_string(),
                        ),
                        UserVariableItem::ImplementationVersionName(
                            IMPLEMENTATION_VERSION_NAME.to_string(),
                        ),
                    ],
                });
                Ok((pdu, NegotiatedOptions{
                    peer_max_pdu_length: requestor_max_pdu_length,
                    user_variables,
                    presentation_contexts: presentation_contexts_negotiated,
                }, calling_ae_title))
            },
            Pdu::ReleaseRQ => Err((Pdu::ReleaseRP, AbortedSnafu.build())),
            pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRP
            | pdu @ Pdu::AbortRQ { .. } => Err((
                Pdu::AbortRQ {source: AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::UnexpectedPdu)},
                UnexpectedPduSnafu { pdu }.build()
            )),
            pdu @ Pdu::Unknown { .. } => Err((
                Pdu::AbortRQ {source: AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::UnrecognizedPdu)},
                UnknownPduSnafu { pdu }.build()
            )),
        }

    }

    /// Negotiate an association with the given TCP stream.
    pub fn establish(&self, mut socket: TcpStream) -> Result<ServerAssociation<TcpStream>> {
        ensure!(
            !self.abstract_syntax_uids.is_empty() || self.promiscuous,
            MissingAbstractSyntaxSnafu
        );

        let max_pdu_length = self.max_pdu_length;
        socket
            .set_read_timeout(self.read_timeout)
            .context(SetReadTimeoutSnafu)?;
        socket
            .set_write_timeout(self.write_timeout)
            .context(SetWriteTimeoutSnafu)?;

        let mut buf = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);
        let msg = read_pdu_from_wire(&mut socket, &mut buf, MAXIMUM_PDU_SIZE, self.strict)?;
        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        match self.process_a_association_rq(msg) {
            Ok((pdu, NegotiatedOptions{ user_variables: _, presentation_contexts , peer_max_pdu_length}, calling_ae_title)) => {
                write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
                socket.write_all(&buffer).context(WireSendSnafu)?;
                Ok(ServerAssociation { 
                    presentation_contexts,
                    requestor_max_pdu_length: peer_max_pdu_length,
                    acceptor_max_pdu_length: max_pdu_length,
                    socket,
                    client_ae_title: calling_ae_title,
                    buffer,
                    strict: self.strict,
                    read_buffer: buf,
                    read_timeout: self.read_timeout,
                    write_timeout: self.write_timeout,
                })
            },
            Err((pdu, err)) => {
                // send the rejection/abort PDU
                write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
                socket.write_all(&buffer).context(WireSendSnafu)?;
                Err(err)
            }
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
pub struct ServerAssociation<S> {
    /// The accorded presentation contexts
    presentation_contexts: Vec<PresentationContextNegotiated>,
    /// The maximum PDU length that the remote application entity accepts
    requestor_max_pdu_length: u32,
    /// The maximum PDU length that this application entity is expecting to receive
    acceptor_max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: S,
    /// The application entity title of the other DICOM node
    client_ae_title: String,
    /// write buffer to send fully assembled PDUs on wire
    buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
    /// Read buffer from the socket
    read_buffer: bytes::BytesMut,
    /// Timeout for individual receive operations
    read_timeout: Option<std::time::Duration>,
    /// Timeout for individual send operations
    write_timeout: Option<std::time::Duration>,
}

impl<S> ServerAssociation<S> {
    /// Obtain a view of the negotiated presentation contexts.
    pub fn presentation_contexts(&self) -> &[PresentationContextNegotiated] {
        &self.presentation_contexts
    }

    /// Obtain the remote DICOM node's application entity title.
    pub fn client_ae_title(&self) -> &str {
        &self.client_ae_title
    }
}

impl ServerAssociation<TcpStream> {
    /// Send a PDU message to the other intervenient.
    pub fn send(&mut self, msg: &Pdu) -> Result<()> {
        self.buffer.clear();
        write_pdu(&mut self.buffer, msg).context(SendPduSnafu)?;
        if self.buffer.len() > self.requestor_max_pdu_length as usize {
            return SendTooLongPduSnafu {
                length: self.buffer.len(),
            }
            .fail();
        }
        self.socket.write_all(&self.buffer).context(WireSendSnafu)
    }

    /// Read a PDU message from the other intervenient.
    pub fn receive(&mut self) -> Result<Pdu> {
        read_pdu_from_wire(&mut self.socket, &mut self.read_buffer, self.acceptor_max_pdu_length, self.strict)
    }

    /// Send a provider initiated abort message
    /// and shut down the TCP connection,
    /// terminating the association.
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

    /// Prepare a P-Data writer for sending
    /// one or more data item PDUs.
    ///
    /// Returns a writer which automatically
    /// splits the inner data into separate PDUs if necessary.
    pub fn send_pdata(&mut self, presentation_context_id: u8) -> PDataWriter<&mut TcpStream> {
        PDataWriter::new(
            &mut self.socket,
            presentation_context_id,
            self.requestor_max_pdu_length,
        )
    }

    /// Prepare a P-Data reader for receiving
    /// one or more data item PDUs.
    ///
    /// Returns a reader which automatically
    /// receives more data PDUs once the bytes collected are consumed.
    pub fn receive_pdata(&mut self) -> PDataReader<'_, &mut TcpStream> {
        PDataReader::new(
            &mut self.socket,
            self.acceptor_max_pdu_length,
            &mut self.read_buffer,
        )
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

#[cfg(feature = "async")]
pub mod non_blocking {

    use bytes::BytesMut;
    use snafu::{ensure, ResultExt};
    use tokio::{io::AsyncWriteExt, net::TcpStream};

    use super::{
        AccessControl, Result, SendTooLongPduSnafu, ServerAssociation,
        ServerAssociationOptions, WireSendSnafu,
    };
    use crate::{
        association::{
            read_pdu_from_wire_async, server::{
                MissingAbstractSyntaxSnafu,
            }, NegotiatedOptions, ReceivePduSnafu, SendPduSnafu, TimeoutSnafu
        },
        pdu::{
            AbortRQServiceProviderReason, AbortRQSource,
            ReadPduSnafu, MAXIMUM_PDU_SIZE,
        },
        write_pdu, Pdu,
    };

    impl<A> ServerAssociationOptions<'_, A>
    where
        A: AccessControl,
    {
        /// Negotiate an association with the given TCP stream.
        pub async fn establish_async(
            &self,
            mut socket: TcpStream,
        ) -> Result<ServerAssociation<TcpStream>> {
            ensure!(
                !self.abstract_syntax_uids.is_empty() || self.promiscuous,
                MissingAbstractSyntaxSnafu
            );
            let read_timeout = self.read_timeout;
            let task = async {
                let max_pdu_length = self.max_pdu_length;
                let mut buf = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);
                let pdu = read_pdu_from_wire_async(&mut socket, &mut buf, MAXIMUM_PDU_SIZE, self.strict).await?;

                let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
                match self.process_a_association_rq(pdu) {
                    Ok((pdu, NegotiatedOptions{ user_variables: _, presentation_contexts , peer_max_pdu_length}, calling_ae_title)) => {
                        write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
                        socket.write_all(&buffer).await.context(WireSendSnafu)?;
                        Ok(ServerAssociation { 
                            presentation_contexts,
                            requestor_max_pdu_length: peer_max_pdu_length,
                            acceptor_max_pdu_length: max_pdu_length,
                            socket,
                            client_ae_title: calling_ae_title,
                            buffer,
                            strict: self.strict,
                            read_buffer: buf,
                            read_timeout: self.read_timeout,
                            write_timeout: self.write_timeout,
                        })
                    },
                    Err((pdu, err)) => {
                        // send the rejection/abort PDU
                        write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
                        socket.write_all(&buffer).await.context(WireSendSnafu)?;
                        Err(err)
                    }
                }

            };
            if let Some(timeout) = read_timeout {
                tokio::time::timeout(timeout, task)
                    .await
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))
                    .context(TimeoutSnafu)?
            } else {
                task.await
            }
        }
    }

    impl ServerAssociation<TcpStream> {
        /// Send a PDU message to the other intervenient.
        pub async fn send(&mut self, msg: &Pdu) -> Result<()> {
            let timeout = self.write_timeout;
            let task = async {
                self.buffer.clear();
                write_pdu(&mut self.buffer, msg).context(SendPduSnafu)?;
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
            };
            if let Some(timeout) = timeout {
                tokio::time::timeout(timeout, task)
                    .await
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))
                    .context(WireSendSnafu)?
            } else {
                task.await
            }
        }

        /// Read a PDU message from the other intervenient.
        pub async fn receive(&mut self) -> Result<Pdu> {
            let timeout = self.read_timeout;
            let task = async {
                read_pdu_from_wire_async(
                    &mut self.socket,
                    &mut self.read_buffer,
                    self.acceptor_max_pdu_length,
                    self.strict
                ).await
            };
            if let Some(timeout) = timeout {
                tokio::time::timeout(timeout, task)
                    .await
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))
                    .context(ReadPduSnafu)
                    .context(ReceivePduSnafu)?
            } else {
                task.await
            }
        }

        /// Send a provider initiated abort message
        /// and shut down the TCP connection,
        /// terminating the association.
        pub async fn abort(mut self) -> Result<()> {
            let timeout = self.write_timeout;
            let task = async {
                let pdu = Pdu::AbortRQ {
                    source: AbortRQSource::ServiceProvider(
                        AbortRQServiceProviderReason::ReasonNotSpecified,
                    ),
                };
                let out = self.send(&pdu).await;
                let _ = self.socket.shutdown().await;
                out
            };
            if let Some(timeout) = timeout {
                tokio::time::timeout(timeout, task)
                    .await
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))
                    .context(WireSendSnafu)?
            } else {
                task.await
            }
        }

        pub fn inner_stream(&mut self) -> &mut TcpStream {
            &mut self.socket
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    impl<'a, A> ServerAssociationOptions<'a, A>
    where
        A: AccessControl 
    {
        // Broken implementation of server establish which sends an extra pdu during establish
        pub(crate) fn establish_with_extra_pdus(&self, mut socket: std::net::TcpStream, extra_pdus: Vec<Pdu>) -> Result<ServerAssociation<TcpStream>> {
            let mut buf = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);
            let pdu = read_pdu_from_wire(&mut socket, &mut buf, MAXIMUM_PDU_SIZE, self.strict)?;
            let (
                pdu,
                NegotiatedOptions{ user_variables: _, presentation_contexts , peer_max_pdu_length},
                calling_ae_title
            ) = self.process_a_association_rq(pdu)
                .expect("Could not parse association req");

            let mut buffer: Vec<u8> = Vec::with_capacity(self.max_pdu_length as usize);
            write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
            for extra_pdu in extra_pdus {
                write_pdu(&mut buffer, &extra_pdu).context(SendPduSnafu)?;
            }
            socket.write_all(&buffer).context(WireSendSnafu)?;

            Ok(ServerAssociation { 
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: calling_ae_title,
                buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize),
                read_timeout: self.read_timeout,
                write_timeout: self.write_timeout,
            })
        }

        // Broken implementation of server establish which sends an extra pdu during establish
        #[cfg(feature = "async")]
        pub(crate) async fn establish_with_extra_pdus_async(
            &self, mut socket: tokio::net::TcpStream, extra_pdus: Vec<Pdu>
        ) -> Result<ServerAssociation<tokio::net::TcpStream>> {
            use tokio::io::AsyncWriteExt;

            use crate::association::read_pdu_from_wire_async;

            let mut buf = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);
            let pdu = read_pdu_from_wire_async(&mut socket, &mut buf, MAXIMUM_PDU_SIZE, self.strict).await?;
            let (
                pdu,
                NegotiatedOptions{ user_variables: _, presentation_contexts , peer_max_pdu_length},
                calling_ae_title
            ) = self.process_a_association_rq(pdu)
                .expect("Could not parse association req");

            let mut buffer: Vec<u8> = Vec::with_capacity(self.max_pdu_length as usize);
            write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
            for extra_pdu in extra_pdus {
                write_pdu(&mut buffer, &extra_pdu).context(SendPduSnafu)?;
            }
            socket.write_all(&buffer).await.context(WireSendSnafu)?;

            Ok(ServerAssociation { 
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: calling_ae_title,
                buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize),
                read_timeout: self.read_timeout,
                write_timeout: self.write_timeout,
            })
        }

        // Broken implementation of server establish which reproduces behavior that #589 introduced
        pub fn broken_establish(&self, mut socket: TcpStream) -> Result<ServerAssociation<TcpStream>> {
            let mut buf = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);
            let msg = read_pdu_from_wire(&mut socket, &mut buf, MAXIMUM_PDU_SIZE, self.strict)?;
            let mut buffer: Vec<u8> = Vec::with_capacity(self.max_pdu_length as usize);
            let (
                pdu,
                NegotiatedOptions{user_variables: _, presentation_contexts , peer_max_pdu_length},
                calling_ae_title
            ) = self.process_a_association_rq(msg).expect("Could not parse association req");
            write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
            socket.write_all(&buffer).context(WireSendSnafu)?;
            Ok(ServerAssociation { 
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: calling_ae_title,
                buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize),
                read_timeout: self.read_timeout,
                write_timeout: self.write_timeout,
            })
        }

        // Broken implementation of server establish which reproduces behavior that #589 introduced
        #[cfg(feature = "async")]
        pub async fn broken_establish_async(&self, mut socket: tokio::net::TcpStream) -> Result<ServerAssociation<tokio::net::TcpStream>> {
            use tokio::io::AsyncWriteExt;

            use crate::association::read_pdu_from_wire_async;

            let mut buf = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);
            let msg = read_pdu_from_wire_async(&mut socket, &mut buf, MAXIMUM_PDU_SIZE, self.strict).await?;
            let mut buffer: Vec<u8> = Vec::with_capacity(self.max_pdu_length as usize);
            let (
                pdu,
                NegotiatedOptions{user_variables: _, presentation_contexts , peer_max_pdu_length},
                calling_ae_title
            ) = self.process_a_association_rq(msg).expect("Could not parse association req");
            write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
            socket.write_all(&buffer).await.context(WireSendSnafu)?;
            Ok(ServerAssociation { 
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: calling_ae_title,
                buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize),
                read_timeout: self.read_timeout,
                write_timeout: self.write_timeout,
            })
        }
    }

}
