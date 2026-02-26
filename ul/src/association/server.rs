//! Association acceptor module
//!
//! The module provides an abstraction for a DICOM association
//! in which this application entity listens to incoming association requests.
//! See [`ServerAssociationOptions`]
//! for details and examples on how to create an association.
use bytes::BytesMut;
use std::borrow::Cow;
use std::time::Duration;
use std::{io::Write, net::TcpStream};

use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use snafu::{ensure, ResultExt};
use crate::association::private::SyncAssociationSealed;
use crate::association::{
    Association, CloseSocket, SocketOptions, SyncAssociation, encode_pdu,
    read_pdu_from_wire, AbortedSnafu, MissingAbstractSyntaxSnafu, RejectedSnafu, SendPduSnafu,
    UnexpectedPduSnafu,
    UnknownPduSnafu, WireSendSnafu,
};

use crate::association::NegotiatedOptions;
use crate::pdu::{PresentationContextNegotiated, LARGE_PDU_SIZE};
use crate::{
    pdu::{
        write_pdu, AbortRQServiceProviderReason, AbortRQSource, AssociationAC, AssociationRJ,
        AssociationRJResult, AssociationRJServiceUserReason, AssociationRJSource, AssociationRQ,
        Pdu, PresentationContextResult, PresentationContextResultReason, UserIdentity,
        UserVariableItem, DEFAULT_MAX_PDU, PDU_HEADER_SIZE,
    },
    IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME,
};

use super::{
    uid::trim_uid,
    Error, Result
};

#[cfg(feature = "sync-tls")]
pub type TlsStream = rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>;
#[cfg(feature = "async-tls")]
pub type AsyncTlsStream = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;

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
///
/// ## Basic Usage
///
/// ### Synchronous API
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
/// ### Asynchronous API
///
/// Spawn an async task for each incoming association request.
///
/// ```no_run
/// # use std::net::{Ipv4Addr, SocketAddrV4};
/// # use dicom_ul::association::{server::ServerAssociationOptions};
/// # #[cfg(feature = "async")]
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # use dicom_ul::association::AsyncAssociation;
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
/// ## TLS Support
/// 
/// Enabling one of the Cargo features `sync-tls` or `async-tls`
/// unlocks the methods for configuring TLS.
/// Call `tls_config`
/// for the server to expect associations established
/// over a secure transport connection.
///
/// #### TLS in synchronous API
/// 
/// Include the `sync-tls` feature in your `Cargo.toml`.
/// 
/// #### TLS in asynchronous API
/// 
/// Include the `async-tls` feature in your `Cargo.toml`.
/// 
/// ### Example
///
/// ```no_run
/// # use dicom_ul::association::server::ServerAssociationOptions;
/// # use std::time::Duration;
/// # use std::sync::Arc;
/// # #[cfg(feature = "sync-tls")]
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// use std::net::TcpListener;
/// use rustls::{
///     ServerConfig, RootCertStore,
///     pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
///     server::WebPkiClientVerifier,
/// };
/// # let tcp_listener: TcpListener = unimplemented!();
/// // Loading certificates and keys for demonstration purposes
/// let ca_cert = CertificateDer::from_pem_slice(std::fs::read("ssl/ca.crt")?.as_ref())
///     .expect("Failed to load client cert");
/// 
/// // Server certificate and private key -- signed by CA
/// let server_cert = CertificateDer::from_pem_slice(std::fs::read("ssl/server.crt")?.as_ref())
///     .expect("Failed to load server cert");
///
/// let server_private_key = PrivateKeyDer::from_pem_slice(std::fs::read("ssl/server.key")?.as_ref())
///     .expect("Failed to load client private key");
/// 
/// // Create a root cert store for the client which includes the server certificate
/// let mut certs = RootCertStore::empty();
/// certs.add_parsable_certificates(vec![ca_cert.clone()]);
///
/// // Server configuration.
/// // Creates a server config that requires client authentication (mutual TLS) using 
/// // webpki for certificate verification.
/// let server_config = ServerConfig::builder()
///     .with_client_cert_verifier(
///         WebPkiClientVerifier::builder(certs.clone().into())
///             .build()
///             .expect("Failed to create client certificate verifier")
///     )
///     .with_single_cert(vec![server_cert.clone(), ca_cert.clone()], server_private_key)
///     .expect("Failed to create server TLS config");
/// 
/// let (stream, _address) = tcp_listener.accept()?;
///
/// let association = ServerAssociationOptions::new()
///     .accept_called_ae_title()
///     .ae_title("TLS-SCP")
///     .with_abstract_syntax(dicom_dictionary_std::uids::VERIFICATION)
///     .tls_config(server_config)
///     .establish_tls(stream);
/// # Ok(())
/// # }
/// ```
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
    /// Options for the underlying TCP socket
    socket_options: SocketOptions,
    /// TLS configuration for the underlying TCP socket
    #[cfg(feature = "sync-tls")]
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
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
            socket_options: SocketOptions::default(),
            #[cfg(feature = "sync-tls")]
            tls_config: None,
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
            socket_options,
            #[cfg(feature = "sync-tls")]
            tls_config
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
            socket_options,
            #[cfg(feature = "sync-tls")]
            tls_config
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
            socket_options: SocketOptions {
                read_timeout: Some(timeout),
                write_timeout: self.socket_options.write_timeout,
                connection_timeout: self.socket_options.connection_timeout,
            },
            ..self
        }
    }

    /// Set the write timeout for the underlying TCP socket
    pub fn write_timeout(self, timeout: Duration) -> Self {
        Self {
            socket_options: SocketOptions {
                read_timeout: self.socket_options.read_timeout,
                write_timeout: Some(timeout),
                connection_timeout: self.socket_options.connection_timeout,
            },
            ..self
        }
    }

    /// Set the TLS configuration for the underlying TCP socket
    #[cfg(feature = "sync-tls")]
    pub fn tls_config(mut self, config: impl Into<std::sync::Arc<rustls::ServerConfig>>) -> Self {
        self.tls_config = Some(config.into());
        self
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
    fn process_a_association_rq(
        &self,
        msg: Pdu,
    ) -> std::result::Result<(Pdu, NegotiatedOptions), (Pdu, Error)> {
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
                    let association_rj = AssociationRJ {
                        result: AssociationRJResult::Permanent,
                        source: AssociationRJSource::ServiceUser(
                            AssociationRJServiceUserReason::NoReasonGiven,
                        ),
                    };
                    let pdu = Pdu::AssociationRJ(association_rj.clone());
                    return Err((pdu, RejectedSnafu { association_rj }.build()));
                }

                if application_context_name != self.application_context_name {
                    let association_rj = AssociationRJ {
                        result: AssociationRJResult::Permanent,
                        source: AssociationRJSource::ServiceUser(
                            AssociationRJServiceUserReason::ApplicationContextNameNotSupported,
                        ),
                    };
                    let pdu = Pdu::AssociationRJ(association_rj.clone());
                    return Err((pdu, RejectedSnafu { association_rj }.build()));
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
                        Err((pdu, RejectedSnafu { association_rj }.build()))
                    })?;

                // fetch requested maximum PDU length
                let requestor_max_pdu_length = user_variables
                    .iter()
                    .find_map(|item| match item {
                        UserVariableItem::MaxLength(len) => Some(*len),
                        _ => None,
                    })
                    .unwrap_or(DEFAULT_MAX_PDU);

                // treat 0 as practically unlimited,
                // so use the largest 32-bit unsigned number
                let requestor_max_pdu_length = if requestor_max_pdu_length == 0 {
                    u32::MAX
                } else {
                    requestor_max_pdu_length
                };

                let presentation_contexts_negotiated: Vec<_> = presentation_contexts
                    .into_iter()
                    .map(|pc| {
                        let abstract_syntax = trim_uid(Cow::from(pc.abstract_syntax));
                        if !self.abstract_syntax_uids.contains(&abstract_syntax)
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
                    peer_ae_title: calling_ae_title
                }))
            },
            Pdu::ReleaseRQ => Err((Pdu::ReleaseRP, AbortedSnafu.build())),
            pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRP
            | pdu @ Pdu::AbortRQ { .. } => Err((
                Pdu::AbortRQ {
                    source: AbortRQSource::ServiceProvider(
                        AbortRQServiceProviderReason::UnexpectedPdu,
                    ),
                },
                UnexpectedPduSnafu { pdu }.build(),
            )),
            pdu @ Pdu::Unknown { .. } => Err((
                Pdu::AbortRQ {
                    source: AbortRQSource::ServiceProvider(
                        AbortRQServiceProviderReason::UnrecognizedPdu,
                    ),
                },
                UnknownPduSnafu { pdu }.build(),
            )),
        }
    }

    /// Negotiate an association with the given TCP stream.
    pub fn establish(&self, mut socket: TcpStream) -> Result<ServerAssociation<TcpStream>>
    {
        ensure!(
            !self.abstract_syntax_uids.is_empty() || self.promiscuous,
            MissingAbstractSyntaxSnafu
        );

        socket
            .set_read_timeout(self.socket_options.read_timeout)
            .context(super::SetReadTimeoutSnafu)?;
        socket
            .set_write_timeout(self.socket_options.write_timeout)
            .context(super::SetWriteTimeoutSnafu)?;


        let mut read_buffer = BytesMut::with_capacity(
            (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
        );
        let msg = read_pdu_from_wire(&mut socket, &mut read_buffer, self.max_pdu_length, self.strict)?;
        let mut write_buffer: Vec<u8> = Vec::with_capacity(self.max_pdu_length as usize);
        match self.process_a_association_rq(msg) {
            Ok((pdu, NegotiatedOptions{ user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title })) => {
                write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                socket.write_all(&write_buffer).context(WireSendSnafu)?;
                Ok(ServerAssociation { 
                    presentation_contexts,
                    requestor_max_pdu_length: peer_max_pdu_length,
                    acceptor_max_pdu_length: self.max_pdu_length,
                    socket,
                    client_ae_title: peer_ae_title,
                    write_buffer,
                    strict: self.strict,
                    read_buffer,
                    user_variables,
                })
            }
            Err((pdu, err)) => {
                // send the rejection/abort PDU
                write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                socket.write_all(&write_buffer).context(WireSendSnafu)?;
                Err(err)
            }
        }
    }

    /// Negotiate an association with the given TCP stream using TLS.
    #[cfg(feature = "sync-tls")]
    pub fn establish_tls(&self, socket: TcpStream) -> Result<ServerAssociation<TlsStream>> {
        ensure!(
            !self.abstract_syntax_uids.is_empty() || self.promiscuous,
            MissingAbstractSyntaxSnafu
        );
        let tls_config = self.tls_config.as_ref().ok_or_else(|| {
            super::TlsConfigMissingSnafu {}.build()
        })?;

        socket
            .set_read_timeout(self.socket_options.read_timeout)
            .context(super::SetReadTimeoutSnafu)?;
        socket
            .set_write_timeout(self.socket_options.write_timeout)
            .context(super::SetWriteTimeoutSnafu)?;

        let conn = rustls::ServerConnection::new(tls_config.clone())
            .context(super::TlsConnectionSnafu)?;
        let mut tls_stream = rustls::StreamOwned::new(conn, socket);
        let mut read_buffer = BytesMut::with_capacity(
            (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
        );

        let msg = read_pdu_from_wire(&mut tls_stream, &mut read_buffer, self.max_pdu_length, self.strict)?;
        let mut write_buffer: Vec<u8> = Vec::with_capacity(self.max_pdu_length as usize);
        match self.process_a_association_rq(msg) {
            Ok((pdu, NegotiatedOptions{ user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title })) => {
                write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                tls_stream.write_all(&write_buffer).context(WireSendSnafu)?;
                Ok(ServerAssociation { 
                    presentation_contexts,
                    requestor_max_pdu_length: peer_max_pdu_length,
                    acceptor_max_pdu_length: self.max_pdu_length,
                    socket: tls_stream,
                    client_ae_title: peer_ae_title,
                    write_buffer,
                    strict: self.strict,
                    read_buffer,
                    user_variables,
                })
            },
            Err((pdu, err)) => {
                // send the rejection/abort PDU
                write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                tls_stream.write_all(&write_buffer).context(WireSendSnafu)?;
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
/// [`send`](SyncAssociation::send)
/// and [`receive`](SyncAssociation::receive).
/// Sending large P-Data fragments may be easier through the P-Data sender
/// abstraction (see [`send_pdata`](SyncAssociation::send_pdata)).
///
/// When the value falls out of scope,
/// the program will shut down the underlying TCP connection.
#[derive(Debug)]
pub struct ServerAssociation<S> 
where S: std::io::Read + std::io::Write + CloseSocket{
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
    /// Reusable buffer used for sending PDUs on the wire
    /// prevents reallocation on each send
    write_buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
    /// Read buffer from the socket
    read_buffer: bytes::BytesMut,
    /// User variables received from the peer
    user_variables: Vec<UserVariableItem>,
}

impl<S> Association for ServerAssociation<S>
where S: std::io::Read + std::io::Write + CloseSocket{

    /// Obtain a view of the negotiated presentation contexts.
    fn presentation_contexts(&self) -> &[PresentationContextNegotiated] {
        &self.presentation_contexts
    }

    /// Retrieve the maximum PDU length
    /// admitted by this application entity.
    fn acceptor_max_pdu_length(&self) -> u32 {
        self.acceptor_max_pdu_length
    }

    /// Retrieve the maximum PDU length
    /// that the requestor is expecting to receive.
    fn requestor_max_pdu_length(&self) -> u32 {
        self.requestor_max_pdu_length
    }

    /// Obtain the remote DICOM node's application entity title.
    fn peer_ae_title(&self) -> &str {
        &self.client_ae_title
    }

    fn user_variables(&self) -> &[UserVariableItem] {
        &self.user_variables
    }
}

impl<S> SyncAssociationSealed<S> for ServerAssociation<S>
    where S: std::io::Read + std::io::Write + CloseSocket {
    fn send(&mut self, pdu: &Pdu) -> Result<()> {
        self.write_buffer.clear();
        encode_pdu(&mut self.write_buffer, pdu, self.requestor_max_pdu_length + PDU_HEADER_SIZE)?;
        self.socket.write_all(&self.write_buffer).context(WireSendSnafu)
    }

    fn receive(&mut self) -> Result<Pdu> {
        read_pdu_from_wire(&mut self.socket, &mut self.read_buffer, self.acceptor_max_pdu_length, self.strict)
    }

    fn close(&mut self) -> std::io::Result<()>{
        self.socket.close()
    }
}

impl<S> SyncAssociation<S> for ServerAssociation<S>
where S: std::io::Read + std::io::Write + CloseSocket,{
    fn inner_stream(&mut self) -> &mut S {
        &mut self.socket
    }

    fn get_mut(&mut self) -> (&mut S, &mut BytesMut) {
        let Self { socket, read_buffer, .. } = self;
        (socket, read_buffer)
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
impl<A> ServerAssociationOptions<'_, A>
where
    A: AccessControl,
{
    /// Negotiate an association with the given TCP stream.
    pub async fn establish_async(&self, mut socket: tokio::net::TcpStream) -> Result<AsyncServerAssociation<tokio::net::TcpStream>> {
        use tokio::io::AsyncWriteExt;
        ensure!(
            !self.abstract_syntax_uids.is_empty() || self.promiscuous,
            MissingAbstractSyntaxSnafu
        );
        let read_timeout = self.socket_options.read_timeout;
        let task = async {
            let mut read_buffer = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let pdu = super::read_pdu_from_wire_async(&mut socket, &mut read_buffer, self.max_pdu_length, self.strict).await?;

            let mut write_buffer: Vec<u8> =
                Vec::with_capacity((DEFAULT_MAX_PDU + PDU_HEADER_SIZE) as usize);
            match self.process_a_association_rq(pdu) {
                Ok((pdu, NegotiatedOptions{ user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title})) => {
                    write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                    socket.write_all(&write_buffer).await.context(WireSendSnafu)?;
                    Ok(AsyncServerAssociation { 
                        presentation_contexts,
                        requestor_max_pdu_length: peer_max_pdu_length,
                        acceptor_max_pdu_length: self.max_pdu_length,
                        socket,
                        client_ae_title: peer_ae_title,
                        write_buffer,
                        strict: self.strict,
                        read_buffer,
                        read_timeout: self.socket_options.read_timeout,
                        write_timeout: self.socket_options.write_timeout,
                        user_variables
                    })
                },
                Err((pdu, err)) => {
                    // send the rejection/abort PDU
                    write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                    socket.write_all(&write_buffer).await.context(WireSendSnafu)?;
                    Err(err)
                }
            }

        };
        super::timeout(read_timeout, task).await
    }

    /// Negotiate an association with the given TCP stream.
    #[cfg(feature = "async-tls")]
    pub async fn establish_tls_async(&self, socket: tokio::net::TcpStream) -> Result<AsyncServerAssociation<AsyncTlsStream>> {
        use tokio_rustls::TlsAcceptor;
        use tokio::io::AsyncWriteExt;

        ensure!(
            !self.abstract_syntax_uids.is_empty() || self.promiscuous,
            MissingAbstractSyntaxSnafu
        );
        let tls_config = self.tls_config.as_ref().ok_or_else(|| {
            crate::association::TlsConfigMissingSnafu {}.build()
        })?;
        let acceptor = TlsAcceptor::from(tls_config.clone());
        let mut socket = acceptor.accept(socket).await.context(crate::association::ConnectSnafu)?;
        let read_timeout = self.socket_options.read_timeout;
        let task = async {
            let mut read_buffer = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let pdu = super::read_pdu_from_wire_async(&mut socket, &mut read_buffer, self.max_pdu_length, self.strict).await?;

            let mut write_buffer: Vec<u8> =
                Vec::with_capacity((DEFAULT_MAX_PDU + PDU_HEADER_SIZE) as usize);
            match self.process_a_association_rq(pdu) {
                Ok((pdu, NegotiatedOptions{ user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title})) => {
                    write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                    socket.write_all(&write_buffer).await.context(WireSendSnafu)?;
                    Ok(AsyncServerAssociation { 
                        presentation_contexts,
                        requestor_max_pdu_length: peer_max_pdu_length,
                        acceptor_max_pdu_length: self.max_pdu_length,
                        socket,
                        client_ae_title: peer_ae_title,
                        write_buffer,
                        strict: self.strict,
                        read_buffer,
                        read_timeout: self.socket_options.read_timeout,
                        write_timeout: self.socket_options.write_timeout,
                        user_variables
                    })
                },
                Err((pdu, err)) => {
                    // send the rejection/abort PDU
                    write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
                    socket.write_all(&write_buffer).await.context(WireSendSnafu)?;
                    Err(err)
                }
            }
        };
        super::timeout(read_timeout, task).await
    }
}

/// An async DICOM upper level association from the perspective
/// of an accepting application entity.
///
/// The most common operations of an established association are
/// [`send`](crate::association::AsyncAssociation::send)
/// and [`receive`](crate::association::AsyncAssociation::receive).
/// Sending large P-Data fragments may be easier through the P-Data sender
/// abstraction (see [`send_pdata`](crate::association::AsyncAssociation::send_pdata)).
///
/// When the value falls out of scope,
/// the program will shut down the underlying TCP connection.
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct AsyncServerAssociation<S> 
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send{
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
    write_buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
    /// Read buffer from the socket
    read_buffer: bytes::BytesMut,
    /// Timeout for individual receive operations
    read_timeout: Option<std::time::Duration>,
    /// Timeout for individual send operations
    write_timeout: Option<std::time::Duration>,
    /// User variables received from the peer
    user_variables: Vec<UserVariableItem>,
}

#[cfg(feature = "async")]
impl<S> Association for AsyncServerAssociation<S> 
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send{

    fn acceptor_max_pdu_length(&self) -> u32 {
        self.acceptor_max_pdu_length
    }

    fn requestor_max_pdu_length(&self) -> u32 {
        self.requestor_max_pdu_length
    }

    /// Obtain a view of the negotiated presentation contexts.
    fn presentation_contexts(&self) -> &[PresentationContextNegotiated] {
        &self.presentation_contexts
    }

    /// Obtain the remote DICOM node's application entity title.
    fn peer_ae_title(&self) -> &str {
        &self.client_ae_title
    }

    fn user_variables(&self) -> &[UserVariableItem] {
        &self.user_variables
    }
}

#[cfg(feature = "async")]
impl<S> crate::association::private::AsyncAssociationSealed<S> for AsyncServerAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send{
    /// Send a PDU message to the other intervenient.
    async fn send(&mut self, msg: &Pdu) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        self.write_buffer.clear();
        super::timeout(self.write_timeout, async {
            encode_pdu(&mut self.write_buffer, msg, self.requestor_max_pdu_length + PDU_HEADER_SIZE)?;
            self.socket
                .write_all(&self.write_buffer)
                .await
                .context(WireSendSnafu)
        }).await
    }

    /// Read a PDU message from the other intervenient.
    async fn receive(&mut self) -> Result<Pdu> {
        super::timeout(self.read_timeout,async {
            super::read_pdu_from_wire_async(
                &mut self.socket,
                &mut self.read_buffer,
                self.acceptor_max_pdu_length,
                self.strict
            ).await
        }).await
    }

    async fn close(&mut self) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        self.socket.shutdown().await
    }
}

#[cfg(feature = "async")]
impl<S> crate::association::AsyncAssociation<S> for AsyncServerAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send{
    fn inner_stream(&mut self) -> &mut S {
        &mut self.socket
    }

    fn get_mut(&mut self) -> (&mut S, &mut bytes::BytesMut) {
        let Self { socket, read_buffer, .. } = self; 
        (socket, read_buffer)
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
        A: AccessControl,
    {
        // Broken implementation of server establish which sends an extra pdu during establish
        pub(crate) fn establish_with_extra_pdus(
            &self,
            mut socket: std::net::TcpStream,
            extra_pdus: Vec<Pdu>,
        ) -> Result<ServerAssociation<TcpStream>> {
            let mut read_buffer = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let pdu = read_pdu_from_wire(
                &mut socket,
                &mut read_buffer,
                self.max_pdu_length,
                self.strict,
            )?;
            let (
                pdu,
                NegotiatedOptions{ user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title}
            ) = self.process_a_association_rq(pdu)
                .expect("Could not parse association req");

            let mut write_buffer: Vec<u8> =
                Vec::with_capacity((DEFAULT_MAX_PDU + PDU_HEADER_SIZE) as usize);
            write_pdu(&mut write_buffer, &pdu).context(SendPduSnafu)?;
            for extra_pdu in extra_pdus {
                write_pdu(&mut write_buffer, &extra_pdu).context(SendPduSnafu)?;
            }
            socket.write_all(&write_buffer).context(WireSendSnafu)?;

            Ok(ServerAssociation {
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: peer_ae_title,
                write_buffer,
                read_buffer,
                strict: self.strict,
                user_variables,
            })
        }

        // Broken implementation of server establish which sends an extra pdu during establish
        #[cfg(feature = "async")]
        pub(crate) async fn establish_with_extra_pdus_async(
            &self,
            mut socket: tokio::net::TcpStream,
            extra_pdus: Vec<Pdu>,
        ) -> Result<AsyncServerAssociation<tokio::net::TcpStream>> {
            use tokio::io::AsyncWriteExt;

            use crate::association::read_pdu_from_wire_async;

            let mut read_buffer = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let pdu = read_pdu_from_wire_async(
                &mut socket,
                &mut read_buffer,
                self.max_pdu_length,
                self.strict,
            )
            .await?;
            let (
                pdu,
                NegotiatedOptions{ user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title}
            ) = self.process_a_association_rq(pdu)
                .expect("Could not parse association req");

            let mut buffer: Vec<u8> = Vec::with_capacity(
                (peer_max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
            for extra_pdu in extra_pdus {
                write_pdu(&mut buffer, &extra_pdu).context(SendPduSnafu)?;
            }
            socket.write_all(&buffer).await.context(WireSendSnafu)?;

            Ok(AsyncServerAssociation { 
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: peer_ae_title,
                write_buffer: buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(
                    (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
                ),
                user_variables,
                read_timeout: self.socket_options.read_timeout,
                write_timeout: self.socket_options.write_timeout,
            })
        }

        // Broken implementation of server establish which reproduces behavior that #589 introduced
        pub fn broken_establish(
            &self,
            mut socket: TcpStream,
        ) -> Result<ServerAssociation<TcpStream>> {
            let mut read_buffer = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let msg = read_pdu_from_wire(
                &mut socket,
                &mut read_buffer,
                self.max_pdu_length,
                self.strict,
            )?;
            let (
                pdu,
                NegotiatedOptions{user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title}
            ) = self.process_a_association_rq(msg).expect("Could not parse association req");
            let mut buffer: Vec<u8> = Vec::with_capacity(
                (peer_max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
            socket.write_all(&buffer).context(WireSendSnafu)?;
            Ok(ServerAssociation { 
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: peer_ae_title,
                write_buffer: buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(
                    (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
                ),
                user_variables
            })
        }

        // Broken implementation of server establish which reproduces behavior that #589 introduced
        #[cfg(feature = "async")]
        pub async fn broken_establish_async(
            &self,
            mut socket: tokio::net::TcpStream,
        ) -> Result<AsyncServerAssociation<tokio::net::TcpStream>> {
            use tokio::io::AsyncWriteExt;

            use crate::association::read_pdu_from_wire_async;

            let mut read_buffer = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let msg = read_pdu_from_wire_async(
                &mut socket,
                &mut read_buffer,
                self.max_pdu_length,
                self.strict,
            )
            .await?;
            let (
                pdu,
                NegotiatedOptions{user_variables, presentation_contexts , peer_max_pdu_length, peer_ae_title},
            ) = self.process_a_association_rq(msg).expect("Could not parse association req");
            let mut buffer: Vec<u8> = Vec::with_capacity(
                (peer_max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            write_pdu(&mut buffer, &pdu).context(SendPduSnafu)?;
            socket.write_all(&buffer).await.context(WireSendSnafu)?;
            Ok(AsyncServerAssociation { 
                presentation_contexts,
                requestor_max_pdu_length: peer_max_pdu_length,
                acceptor_max_pdu_length: self.max_pdu_length,
                socket,
                client_ae_title: peer_ae_title,
                write_buffer: buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(
                    (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
                ),
                read_timeout: self.socket_options.read_timeout,
                write_timeout: self.socket_options.write_timeout,
                user_variables
            })
        }
    }
}
