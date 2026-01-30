//! Association requester module
//!
//! The module provides an abstraction for a DICOM association
//! in which this application entity is the one requesting the association.
//! See [`ClientAssociationOptions`]
//! for details and examples on how to create an association.
use bytes::BytesMut;
use std::{
    borrow::Cow,
    convert::TryInto,
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use crate::{
    AeAddr, IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME, association::{
        Association, CloseSocket, NegotiatedOptions, SocketOptions, SyncAssociation, encode_pdu, private::SyncAssociationSealed, read_pdu_from_wire
    }, pdu::{
        AbortRQSource, AssociationAC, AssociationRQ, DEFAULT_MAX_PDU, LARGE_PDU_SIZE, PDU_HEADER_SIZE, Pdu, PresentationContextNegotiated, PresentationContextProposed, PresentationContextResultReason, UserIdentity, UserIdentityType, UserVariableItem, write_pdu
    }
};
use snafu::{ensure, ResultExt};

use super::{
    uid::trim_uid,
    Result,
};

#[cfg(feature = "sync-tls")]
pub type TlsStream = rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>;
#[cfg(feature = "async-tls")]
pub type AsyncTlsStream = tokio_rustls::client::TlsStream<tokio::net::TcpStream>;

/// Helper function to establish a TCP client connection
fn tcp_connection<T>(
    ae_address: &AeAddr<T>,
    opts: &SocketOptions,
) -> Result<TcpStream> where T: ToSocketAddrs
{
    // NOTE: TcpStream::connect_timeout needs a single SocketAddr, whereas TcpStream::connect can
    // take multiple 
    let conn_result: Result<TcpStream> = if let Some(timeout) = opts.connection_timeout {
        let addresses = ae_address.to_socket_addrs().context(super::ToAddressSnafu)?;
        let mut result = Result::Err(std::io::Error::from(std::io::ErrorKind::AddrNotAvailable));
        for address in addresses {
            result = TcpStream::connect_timeout(&address, timeout);
            if result.is_ok() {
                break;
            }
        }
        result.context(super::ConnectSnafu)
    } else {
        TcpStream::connect(ae_address).context(super::ConnectSnafu)
    };

    let socket = conn_result?;
    socket
        .set_read_timeout(opts.read_timeout)
        .context(super::SetReadTimeoutSnafu)?;
    socket
        .set_write_timeout(opts.write_timeout)
        .context(super::SetWriteTimeoutSnafu)?;

    Ok(socket)

}

/// Helper function to establish a TLS client connection
#[cfg(feature = "sync-tls")]
fn tls_connection<T>(
    ae_address: &AeAddr<T>,
    server_name: &str,
    opts: &SocketOptions,
    tls_config: std::sync::Arc<rustls::ClientConfig>,
) -> Result<TlsStream> where T: ToSocketAddrs{
    use std::convert::TryFrom;

    let socket =  tcp_connection(ae_address, opts)?;
    let server_name = rustls::pki_types::ServerName::try_from(server_name.to_string())
        .context(super::InvalidServerNameSnafu)?;
    
    let conn = rustls::ClientConnection::new(tls_config.clone(), server_name)
        .context(super::TlsConnectionSnafu)?;
        
    Ok(rustls::StreamOwned::new(conn, socket))
}

/// A DICOM association builder for a client node.
/// The final outcome is a [`ClientAssociation`].
///
/// This is the standard way of requesting and establishing
/// an association with another DICOM node,
/// that one usually taking the role of a service class provider (SCP).
///
/// You can create either a blocking or non-blocking client by calling either
/// `establish` or `establish_async` respectively.
///
/// > **⚠️ Warning:** It is highly recommended to set `read_timeout` and `write_timeout` to a reasonable
/// > value for the async client since there is _no_ default timeout on
/// > [`TcpStream`]
///
/// ## Basic usage
///
/// ### Sync
///
/// ```no_run
/// # use dicom_ul::association::client::ClientAssociationOptions;
/// # use std::time::Duration;
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let association = ClientAssociationOptions::new()
///    .with_presentation_context("1.2.840.10008.1.1", vec!["1.2.840.10008.1.2.1", "1.2.840.10008.1.2"])
///    .read_timeout(Duration::from_secs(60))
///    .write_timeout(Duration::from_secs(60))
///    .establish("129.168.0.5:104")?;
/// # Ok(())
/// # }
/// ```
///
/// ### Async
/// 
/// * Make sure you include the `async` feature in your `Cargo.toml`
///
/// ```ignore
/// # use dicom_ul::association::client::ClientAssociationOptions;
/// # use std::time::Duration;
/// # #[cfg(feature = "async")]
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let association = ClientAssociationOptions::new()
///    .with_presentation_context("1.2.840.10008.1.1", vec!["1.2.840.10008.1.2.1", "1.2.840.10008.1.2"])
///    .read_timeout(Duration::from_secs(60))
///    .write_timeout(Duration::from_secs(60))
///    .establish_async("129.168.0.5:104")
///    .await?;
/// # Ok(())
/// # }
/// ```
/// 
/// ## TLS Support
/// 
/// ### Sync TLS
/// 
/// * Make sure you include the `sync-tls` feature in your `Cargo.toml`
/// 
/// ### Async TLS
/// 
/// * Make sure you include the `async-tls` feature in your `Cargo.toml`
/// 
/// ### Example
/// ```no_compile
/// # use dicom_ul::association::client::ClientAssociationOptions;
/// # use std::time::Duration;
/// # use std::sync::Arc;
/// # #[cfg(feature = "sync-tls")]
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// use rustls::{
///     ClientConfig, RootCertStore,
///     pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
/// };
/// // Using a self-signed certificate for demonstration purposes only.
/// let ca_cert = CertificateDer::from_pem_slice(include_bytes!("../../assets/ca.crt").as_ref())
///     .expect("Failed to load client cert");
/// 
/// // Server certificate -- signed by CA
/// let server_cert = CertificateDer::from_pem_slice(include_bytes!("../../assets/server.crt").as_ref())
///     .expect("Failed to load server cert");
///
/// // Client cert and private key -- signed by CA
/// let client_cert = CertificateDer::from_pem_slice(include_bytes!("../../assets/client.crt").as_ref())
///     .expect("Failed to load client cert");
/// let client_private_key = PrivateKeyDer::from_pem_slice(include_bytes!("../../assets/client.key").as_ref())
///     .expect("Failed to load client private key");
/// 
/// // Create a root cert store for the client which includes the server certificate
/// let mut certs = RootCertStore::empty();
/// certs.add_parsable_certificates(vec![ca_cert.clone()]);
///
/// let config = ClientConfig::builder()
///     .with_root_certificates(certs)
///     .with_client_auth_cert(vec![client_cert, ca_cert], client_private_key)
///     .expect("Failed to create client TLS config");
///
/// let association = ClientAssociationOptions::new()
///    .with_presentation_context("1.2.840.10008.1.1", vec!["1.2.840.10008.1.2.1", "1.2.840.10008.1.2"])
///    .tls_config(config)
///    .read_timeout(Duration::from_secs(60))
///    .write_timeout(Duration::from_secs(60))
///    .establish("129.168.0.5:104")?;
/// # Ok(())
/// # }
/// ```
///
/// ## Presentation contexts
///
/// At least one presentation context must be specified,
/// using the method [`with_presentation_context`](Self::with_presentation_context)
/// and supplying both an abstract syntax and list of transfer syntaxes.
///
/// A helper method [`with_abstract_syntax`](Self::with_abstract_syntax) will
/// include by default the transfer syntaxes
/// _Implicit VR Little Endian_ and _Explicit VR Little Endian_
/// in the resulting presentation context.
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
    /// User identity username
    username: Option<Cow<'a, str>>,
    /// User identity password
    password: Option<Cow<'a, str>>,
    /// User identity Kerberos service ticket
    kerberos_service_ticket: Option<Cow<'a, str>>,
    /// User identity SAML assertion
    saml_assertion: Option<Cow<'a, str>>,
    /// User identity JWT
    jwt: Option<Cow<'a, str>>,
    /// Socket options for TCP connections
    socket_options: SocketOptions,
    /// TLS configuration to use for the connection
    #[cfg(feature = "sync-tls")]
    tls_config: Option<std::sync::Arc<rustls::ClientConfig>>,
    /// Server name for TLS
    #[cfg(feature = "sync-tls")]
    server_name: Option<String>,
}

impl Default for ClientAssociationOptions<'_> {
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
            max_pdu_length: DEFAULT_MAX_PDU,
            strict: true,
            username: None,
            password: None,
            kerberos_service_ticket: None,
            saml_assertion: None,
            jwt: None,
            socket_options: SocketOptions {
                read_timeout: None,
                write_timeout: None,
                connection_timeout: None,
            },
            #[cfg(feature = "sync-tls")]
            tls_config: None,
            #[cfg(feature = "sync-tls")]
            server_name: None,
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
    /// Passing an empty string resets the AE title to the default
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

    /// Sets the user identity username
    pub fn username<T>(mut self, username: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let username = username.into();
        if username.is_empty() {
            self.username = None;
        } else {
            self.username = Some(username);
            self.saml_assertion = None;
            self.jwt = None;
            self.kerberos_service_ticket = None;
        }
        self
    }

    /// Sets the user identity password
    pub fn password<T>(mut self, password: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let password = password.into();
        if password.is_empty() {
            self.password = None;
        } else {
            self.password = Some(password);
            self.saml_assertion = None;
            self.jwt = None;
            self.kerberos_service_ticket = None;
        }
        self
    }

    /// Sets the user identity username and password
    pub fn username_password<T, U>(mut self, username: T, password: U) -> Self
    where
        T: Into<Cow<'a, str>>,
        U: Into<Cow<'a, str>>,
    {
        let username = username.into();
        let password = password.into();
        if username.is_empty() {
            self.username = None;
            self.password = None;
        } else {
            self.username = Some(username);
            self.password = Some(password);
            self.saml_assertion = None;
            self.jwt = None;
            self.kerberos_service_ticket = None;
        }
        self
    }

    /// Sets the user identity Kerberos service ticket
    pub fn kerberos_service_ticket<T>(mut self, kerberos_service_ticket: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let kerberos_service_ticket = kerberos_service_ticket.into();
        if kerberos_service_ticket.is_empty() {
            self.kerberos_service_ticket = None;
        } else {
            self.kerberos_service_ticket = Some(kerberos_service_ticket);
            self.username = None;
            self.password = None;
            self.saml_assertion = None;
            self.jwt = None;
        }
        self
    }

    /// Sets the user identity SAML assertion
    pub fn saml_assertion<T>(mut self, saml_assertion: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let saml_assertion = saml_assertion.into();
        if saml_assertion.is_empty() {
            self.saml_assertion = None;
        } else {
            self.saml_assertion = Some(saml_assertion);
            self.username = None;
            self.password = None;
            self.jwt = None;
            self.kerberos_service_ticket = None;
        }
        self
    }

    /// Sets the user identity JWT
    pub fn jwt<T>(mut self, jwt: T) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        let jwt = jwt.into();
        if jwt.is_empty() {
            self.jwt = None;
        } else {
            self.jwt = Some(jwt);
            self.username = None;
            self.password = None;
            self.saml_assertion = None;
            self.kerberos_service_ticket = None;
        }
        self
    }

    /// Set the TLS configuration to use for the connection
    #[cfg(feature = "sync-tls")]
    pub fn tls_config(mut self, config: impl Into<std::sync::Arc<rustls::ClientConfig>>) -> Self {
        self.tls_config = Some(config.into());
        self
    }

    /// Set the server name to use for the TLS connection
    #[cfg(feature = "sync-tls")]
    pub fn server_name(mut self, server_name: &str) -> Self {
        self.server_name = Some(server_name.to_string());
        self
    }

    /// Initiate simple TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    pub fn establish<A: ToSocketAddrs>(
        self,
        address: A,
    ) -> Result<ClientAssociation<std::net::TcpStream>> 
    {
        let addr = AeAddr::new_socket_addr(address);
        let socket = tcp_connection(&addr, &self.socket_options)?;
        self.establish_impl(addr, socket)
    }

    /// Initiate simple TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    #[cfg(feature = "sync-tls")]
    pub fn establish_tls<A: ToSocketAddrs>(
        self, address: A
    ) -> Result<ClientAssociation<TlsStream>> {
        match (&self.tls_config, &self.server_name) {
            (Some(tls_config), Some(server_name)) => {
                let addr = AeAddr::new_socket_addr(address);
                let socket = tls_connection(
                    &addr, server_name, &self.socket_options, tls_config.clone()
                )?;
                self.establish_impl(addr, socket)
            },
            _ => super::TlsConfigMissingSnafu.fail()?
        }
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
    #[allow(unreachable_patterns)]
    pub fn establish_with(
        self,
        ae_address: &str,
    ) -> Result<ClientAssociation<TcpStream>> {
        match ae_address.try_into() {
            Ok(ae_address) => {
                let socket = tcp_connection(&ae_address, &self.socket_options)?;
                self.establish_impl(ae_address, socket)
            },
            Err(_) => {
                let addr = AeAddr::new_socket_addr(ae_address);
                let socket = tcp_connection(&addr, &self.socket_options)?;
                self.establish_impl(addr, socket)
            },
        }
    }


    /// Initiate TLS connection to the given address
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
    #[allow(unreachable_patterns)]
    #[cfg(feature = "sync-tls")]
    pub fn establish_with_tls(
        self,
        ae_address: &str,
    ) -> Result<ClientAssociation<TlsStream>> {
        match (&self.tls_config, &self.server_name) {
            (Some(tls_config), Some(server_name)) => {
                match ae_address.try_into() {
                    Ok(ae_address) => {
                        let socket = tls_connection(
                            &ae_address, server_name, &self.socket_options, tls_config.clone()
                        )?;
                        self.establish_impl(ae_address, socket)
                    },
                    Err(_) => {
                        let addr = AeAddr::new_socket_addr(ae_address);
                        let socket = tls_connection(
                            &addr, server_name, &self.socket_options, tls_config.clone()
                        )?;
                        self.establish_impl(addr, socket)
                    },
                }

            },
            _ => super::TlsConfigMissingSnafu.fail()?
        }
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

    /// Set the connection timeout for the underlying TCP socket
    pub fn connection_timeout(self, timeout: Duration) -> Self {
        Self {
            socket_options: SocketOptions {
                read_timeout: self.socket_options.read_timeout,
                write_timeout: self.socket_options.write_timeout,
                connection_timeout: Some(timeout),
            },
            ..self
        }
    }

    /// Construct the A-ASSOCIATE-RQ PDU given the options and the AE title.
    fn create_a_associate_req(
        &'a self,
        ae_title: Option<&str>,
    ) -> Result<(Vec<PresentationContextProposed>, Pdu)> {
        let ClientAssociationOptions {
            calling_ae_title,
            called_ae_title,
            application_context_name,
            presentation_contexts,
            protocol_version,
            max_pdu_length,
            username,
            password,
            kerberos_service_ticket,
            saml_assertion,
            jwt,
            ..
        } = self;
        // fail if no presentation contexts were provided: they represent intent,
        // should not be omitted by the user
        ensure!(
            !presentation_contexts.is_empty(),
            crate::association::MissingAbstractSyntaxSnafu
        );

        // choose called AE title
        let called_ae_title: &str = match (&called_ae_title, ae_title) {
            (Some(aec), Some(aet)) => {
                if aec != aet {
                    tracing::warn!(
                        "Option `called_ae_title` overrides the AE title from `{aet}` to `{aec}`"
                    );
                }
                aec
            }
            (Some(aec), None) => aec,
            (None, Some(aec)) => aec,
            (None, None) => "ANY-SCP",
        };

        let presentation_contexts_proposed: Vec<_> = presentation_contexts
            .iter()
            .enumerate()
            .map(|(i, presentation_context)| PresentationContextProposed {
                id: (2 * i + 1) as u8,
                abstract_syntax: presentation_context.0.to_string(),
                transfer_syntaxes: presentation_context
                    .1
                    .iter()
                    .map(|uid| uid.to_string())
                    .collect(),
            })
            .collect();

        let mut user_variables = vec![
            UserVariableItem::MaxLength(*max_pdu_length),
            UserVariableItem::ImplementationClassUID(IMPLEMENTATION_CLASS_UID.to_string()),
            UserVariableItem::ImplementationVersionName(IMPLEMENTATION_VERSION_NAME.to_string()),
        ];

        if let Some(user_identity) = Self::determine_user_identity(
            username.as_deref(),
            password.as_deref(),
            kerberos_service_ticket.as_deref(),
            saml_assertion.as_deref(),
            jwt.as_deref(),
        ) {
            user_variables.push(UserVariableItem::UserIdentityItem(user_identity));
        }

        Ok((
            presentation_contexts_proposed.clone(),
            Pdu::AssociationRQ(AssociationRQ {
                protocol_version: *protocol_version,
                calling_ae_title: calling_ae_title.to_string(),
                called_ae_title: called_ae_title.to_string(),
                application_context_name: application_context_name.to_string(),
                presentation_contexts: presentation_contexts_proposed,
                user_variables,
            }),
        ))
    }

    /// Process the A-ASSOCIATE-AC PDU received from the SCP.
    ///
    /// Returns the negotiated options for the association
    fn process_a_association_resp(
        &self,
        msg: Pdu,
        presentation_contexts_proposed: &[PresentationContextProposed],
    ) -> Result<NegotiatedOptions> {
        match msg {
            Pdu::AssociationAC(AssociationAC {
                protocol_version: protocol_version_scp,
                application_context_name: _,
                presentation_contexts: presentation_contexts_scp,
                calling_ae_title: _,
                called_ae_title,
                user_variables,
            }) => {
                ensure!(
                    self.protocol_version == protocol_version_scp,
                    crate::association::ProtocolVersionMismatchSnafu {
                        expected: self.protocol_version,
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

                // treat 0 as practically unlimited
                let acceptor_max_pdu_length = if acceptor_max_pdu_length == 0 {
                    u32::MAX
                } else {
                    acceptor_max_pdu_length
                };

                let presentation_contexts: Vec<_> = presentation_contexts_scp
                    .into_iter()
                    .filter(|c| {
                        c.reason == PresentationContextResultReason::Acceptance
                            && presentation_contexts_proposed.iter().any(|p| p.id == c.id)
                    })
                    .map(|c| {
                        let pcp = presentation_contexts_proposed
                            .iter()
                            .find(|pc| pc.id == c.id)
                            .unwrap();
                        PresentationContextNegotiated {
                            id: c.id,
                            reason: c.reason,
                            transfer_syntax: c.transfer_syntax,
                            abstract_syntax: pcp.abstract_syntax.clone(),
                        }
                    })
                    .collect();
                if presentation_contexts.is_empty() {
                    return crate::association::NoAcceptedPresentationContextsSnafu.fail();
                }
                Ok(NegotiatedOptions {
                    presentation_contexts,
                    peer_max_pdu_length: acceptor_max_pdu_length,
                    user_variables,
                    peer_ae_title: called_ae_title,
                })
            }
            Pdu::AssociationRJ(association_rj) => crate::association::RejectedSnafu { association_rj }.fail(),
            pdu @ Pdu::AbortRQ { .. }
            | pdu @ Pdu::ReleaseRQ
            | pdu @ Pdu::AssociationRQ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRP => crate::association::UnexpectedPduSnafu { pdu }.fail(),
            pdu @ Pdu::Unknown { .. } => crate::association::UnknownPduSnafu { pdu }.fail()
        }
    }

    /// Establish the association with the given AE address.
    fn establish_impl<T, S>(
        self,
        ae_address: AeAddr<T>,
        mut socket: S
    ) -> Result<ClientAssociation<S>>
    where
        T: ToSocketAddrs,
        S: CloseSocket + std::io::Read + std::io::Write,
    {
        let (pc_proposed, a_associate) = self.create_a_associate_req(ae_address.ae_title())?;
        let mut buffer: Vec<u8> = Vec::with_capacity(self.max_pdu_length as usize);

        write_pdu(&mut buffer, &a_associate).context(super::SendPduSnafu)?;
        socket.write_all(&buffer).context(super::WireSendSnafu)?;
        buffer.clear();

        let mut buf = BytesMut::with_capacity(
            (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
        );
        let resp = read_pdu_from_wire(&mut socket, &mut buf, self.max_pdu_length, self.strict)?;
        let negotiated_options = self.process_a_association_resp(resp, &pc_proposed);
        match negotiated_options {
            Err(e) => {
                // abort connection
                let _ = write_pdu(
                    &mut buffer,
                    &Pdu::AbortRQ {
                        source: AbortRQSource::ServiceUser,
                    },
                );
                let _ = socket.write_all(&buffer);
                buffer.clear();
                Err(e)
            },
            Ok(NegotiatedOptions{presentation_contexts, peer_max_pdu_length, user_variables, peer_ae_title}) => {
                Ok(ClientAssociation {
                    presentation_contexts,
                    requestor_max_pdu_length: self.max_pdu_length,
                    acceptor_max_pdu_length: peer_max_pdu_length,
                    socket,
                    write_buffer: buffer,
                    strict: self.strict,
                    // Fixes #589, instead of creating a new buffer, we pass the existing buffer into the Association object.
                    read_buffer: buf,
                    read_timeout: self.socket_options.read_timeout,
                    write_timeout: self.socket_options.write_timeout,
                    user_variables,
                    peer_ae_title,
                })
            }
        }
    }

    fn determine_user_identity<T>(
        username: Option<T>,
        password: Option<T>,
        kerberos_service_ticket: Option<T>,
        saml_assertion: Option<T>,
        jwt: Option<T>,
    ) -> Option<UserIdentity>
    where
        T: Into<Cow<'a, str>>,
    {
        if let Some(username) = username {
            if let Some(password) = password {
                return Some(UserIdentity::new(
                    false,
                    UserIdentityType::UsernamePassword,
                    username.into().as_bytes().to_vec(),
                    password.into().as_bytes().to_vec(),
                ));
            } else {
                return Some(UserIdentity::new(
                    false,
                    UserIdentityType::Username,
                    username.into().as_bytes().to_vec(),
                    vec![],
                ));
            }
        }

        if let Some(kerberos_service_ticket) = kerberos_service_ticket {
            return Some(UserIdentity::new(
                false,
                UserIdentityType::KerberosServiceTicket,
                kerberos_service_ticket.into().as_bytes().to_vec(),
                vec![],
            ));
        }

        if let Some(saml_assertion) = saml_assertion {
            return Some(UserIdentity::new(
                false,
                UserIdentityType::SamlAssertion,
                saml_assertion.into().as_bytes().to_vec(),
                vec![],
            ));
        }

        if let Some(jwt) = jwt {
            return Some(UserIdentity::new(
                false,
                UserIdentityType::Jwt,
                jwt.into().as_bytes().to_vec(),
                vec![],
            ));
        }

        None
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
///
/// This may either be sync or async depending on which method was called to
/// establish the association.
#[derive(Debug)]
pub struct ClientAssociation<S>
where S: CloseSocket + std::io::Read + std::io::Write,
{
    /// The presentation contexts accorded with the acceptor application entity,
    /// without the rejected ones.
    presentation_contexts: Vec<PresentationContextNegotiated>,
    /// The maximum PDU length that this application entity is expecting to receive
    requestor_max_pdu_length: u32,
    /// The maximum PDU length that the remote application entity accepts
    acceptor_max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: S,
    /// Buffer to write PDUs to the wire, prevents needing to allocate on every send
    write_buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
    /// Timeout for individual socket Reads
    read_timeout: Option<Duration>,
    /// Timeout for individual socket Writes.
    write_timeout: Option<Duration>,
    /// Buffer to assemble PDU before parsing
    read_buffer: BytesMut,
    /// User variables that were taken from the server
    user_variables: Vec<UserVariableItem>,
    /// The AE title of the peer
    peer_ae_title: String,
}

impl<S> Association for ClientAssociation<S>
where S: CloseSocket + std::io::Read + std::io::Write,
{
    fn peer_ae_title(&self) -> &str {
        &self.peer_ae_title
    }

    fn requestor_max_pdu_length(&self) -> u32 {
        self.requestor_max_pdu_length
    }

    fn acceptor_max_pdu_length(&self) -> u32 {
        self.acceptor_max_pdu_length
    }

    fn presentation_contexts(&self) -> &[PresentationContextNegotiated] {
        &self.presentation_contexts
    }

    fn user_variables(&self) -> &[UserVariableItem] {
        &self.user_variables
    }
}

impl<S> ClientAssociation<S>
where S: CloseSocket + std::io::Read + std::io::Write,
{
    /// Retrieve read timeout for the association
    pub fn read_timeout(&self) -> Option<Duration> {
        self.read_timeout
    }

    /// Retrieve write timeout for the association
    pub fn write_timeout(&self) -> Option<Duration> {
        self.write_timeout
    }
}

impl<S> SyncAssociationSealed<S> for ClientAssociation<S>
where S: CloseSocket + std::io::Read + std::io::Write{
    /// Send a PDU message to the other intervenient.
    fn send(&mut self, pdu: &Pdu) -> Result<()> {
        self.write_buffer.clear();
        encode_pdu(&mut self.write_buffer, pdu, self.acceptor_max_pdu_length + PDU_HEADER_SIZE)?;
        self.socket.write_all(&self.write_buffer).context(super::WireSendSnafu)
    }

    /// Read a PDU message from the other intervenient.
    fn receive(&mut self) -> Result<Pdu> {
        read_pdu_from_wire(&mut self.socket, &mut self.read_buffer, self.requestor_max_pdu_length, self.strict)
    }

    fn close(&mut self) -> std::io::Result<()> {
        self.socket.close()
    }
}

impl<S> SyncAssociation<S> for ClientAssociation<S>
where S: CloseSocket + std::io::Read + std::io::Write{
    fn inner_stream(&mut self) -> &mut S {
        &mut self.socket
    }

    fn get_mut(&mut self) -> (&mut S, &mut BytesMut) {
        let Self { socket, read_buffer, .. } = self;
        (socket, read_buffer)
    }
}

/// Automatically release the association and shut down the connection.
impl<S> Drop for ClientAssociation<S>
where S: CloseSocket + std::io::Read + std::io::Write,
{
    fn drop(&mut self) {
        let _ = SyncAssociationSealed::release(self);
    }
}

#[cfg(feature = "async")]
/// Initiate simple TCP connection to the given address
pub async fn async_connection<T>(
    ae_address: &AeAddr<T>,
    opts: &SocketOptions,
) -> Result<tokio::net::TcpStream> where T: tokio::net::ToSocketAddrs{
    super::timeout(opts.connection_timeout, async {
        tokio::net::TcpStream::connect(ae_address.socket_addr())
            .await
            .context(crate::association::ConnectSnafu)
    }).await
}

/// Initiate TLS connection to the given address
#[cfg(feature = "async-tls")]
pub(crate) async fn async_tls_connection<T>(
    ae_address: &AeAddr<T>,
    server_name: &str,
    opts: &SocketOptions,
    tls_config: std::sync::Arc<rustls::ClientConfig>,
) -> Result<AsyncTlsStream>
where
    T: tokio::net::ToSocketAddrs,
{
    use std::convert::TryFrom;
    use rustls::pki_types::ServerName;

    let tcp_stream = async_connection(ae_address, opts).await?;
    let connector = tokio_rustls::TlsConnector::from(tls_config);
    let domain = ServerName::try_from(server_name.to_string())
        .context(crate::association::InvalidServerNameSnafu)?;
    // NOTE: When tokio-rustls is updated to return a rustls::Error instead of std::io::Error,
    // switch to `crate::association::TlsConnectionSnafu` for context.
    let tls_stream = connector
        .connect(domain, tcp_stream)
        .await
        .context(crate::association::ConnectSnafu)?;
    Ok(tls_stream)
}

#[cfg(feature = "async")]
#[derive(Debug)]
pub struct AsyncClientAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    /// The presentation contexts accorded with the acceptor application entity,
    /// without the rejected ones.
    presentation_contexts: Vec<PresentationContextNegotiated>,
    /// The maximum PDU length that this application entity is expecting to receive
    requestor_max_pdu_length: u32,
    /// The maximum PDU length that the remote application entity accepts
    acceptor_max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: S,
    /// Buffer to assemble PDU before sending it on wire
    write_buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
    /// Timeout for individual socket Reads
    read_timeout: Option<Duration>,
    /// Timeout for individual socket Writes.
    write_timeout: Option<Duration>,
    /// Buffer to assemble PDU before parsing
    read_buffer: BytesMut,
    /// User variables that were taken from the server
    user_variables: Vec<UserVariableItem>,
    /// The AE title of the peer
    peer_ae_title: String,
}

#[cfg(feature = "async")]
impl<'a> ClientAssociationOptions<'a> {
    async fn establish_impl_async<T, S>(
        self,
        ae_address: AeAddr<T>,
        mut socket: S
    ) -> Result<AsyncClientAssociation<S>>
    where
        T: tokio::net::ToSocketAddrs,
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
    {
        use tokio::io::AsyncWriteExt;
        let (pc_proposed, a_associate) = self.create_a_associate_req(ae_address.ae_title())?;
        let mut write_buffer: Vec<u8> = Vec::with_capacity(DEFAULT_MAX_PDU as usize);

        // send request
        write_pdu(&mut write_buffer, &a_associate)
            .context(crate::association::SendPduSnafu)?;
        super::timeout(self.socket_options.write_timeout, async {
            socket.write_all(&write_buffer)
                .await
                .context(crate::association::WireSendSnafu)?;
            Ok(())
        }).await?;
        write_buffer.clear();

        // read buffer is prepared according to the requestor's max pdu length
        let mut read_buffer = BytesMut::with_capacity(
            (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
        );
        let resp = super::timeout(self.socket_options.read_timeout, async {
            super::read_pdu_from_wire_async(&mut socket, &mut read_buffer, self.max_pdu_length, self.strict).await
        })
        .await?;
        let negotiated_options = self.process_a_association_resp(resp, &pc_proposed);
        match negotiated_options {
            Err(e) => {
                // abort connection
                let _ = write_pdu(
                    &mut write_buffer,
                    &Pdu::AbortRQ {
                        source: AbortRQSource::ServiceUser,
                    },
                );
                socket.write_all(&write_buffer)
                    .await
                    .context(crate::association::WireSendSnafu)?;
                write_buffer.clear();
                Err(e)
            },
            Ok(NegotiatedOptions{presentation_contexts, peer_max_pdu_length, user_variables, peer_ae_title}) => {
                Ok(AsyncClientAssociation {
                    presentation_contexts,
                    requestor_max_pdu_length: self.max_pdu_length,
                    acceptor_max_pdu_length: peer_max_pdu_length,
                    socket,
                    write_buffer,
                    strict: self.strict,
                    // Fixes #589, instead of creating a new buffer, we pass the existing buffer into the Association object.
                    read_buffer,
                    read_timeout: self.socket_options.read_timeout,
                    write_timeout: self.socket_options.write_timeout,
                    user_variables,
                    peer_ae_title
                })
            }
        }
    }

    /// Initiate the TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    pub async fn establish_async<A: tokio::net::ToSocketAddrs>(
        self,
        address: A,
    ) -> Result<AsyncClientAssociation<tokio::net::TcpStream>> {
        let addr = AeAddr::new_socket_addr(address);
        let socket = async_connection(&addr, &self.socket_options).await?;
        self.establish_impl_async(addr, socket)
            .await
    }

    /// Initiate the TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    #[cfg(feature = "async-tls")]
    pub async fn establish_tls_async<A: tokio::net::ToSocketAddrs>(
        self,
        address: A,
    ) -> Result<AsyncClientAssociation<AsyncTlsStream>> {
        match (&self.tls_config, &self.server_name) {
            (Some(tls_config), Some(server_name)) => {
                let addr = AeAddr::new_socket_addr(address);
                let socket = async_tls_connection(
                    &addr, server_name, &self.socket_options, tls_config.clone()
                ).await?;
                self.establish_impl_async(addr, socket)
                    .await
            },
            _ => crate::association::TlsConfigMissingSnafu.fail()?
        }
    }

    /// Initiate async TCP connection to the given address
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
    /// #[tokio::main]
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let association = ClientAssociationOptions::new()
    ///     .with_abstract_syntax("1.2.840.10008.1.1")
    ///     // called AE title in address
    ///     .establish_with_async("MY-STORAGE@10.0.0.100:104")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(unreachable_patterns)]
    pub async fn establish_with_async(
        self,
        ae_address: &str,
    ) -> Result<AsyncClientAssociation<tokio::net::TcpStream>> {
        match ae_address.try_into() {
            Ok(ae_address) => {
                let socket = async_connection(&ae_address, &self.socket_options).await?;
                self.establish_impl_async(ae_address, socket).await
            },
            Err(_) => {
                let addr = AeAddr::new_socket_addr(ae_address);
                let socket = async_connection(&addr, &self.socket_options).await?;
                self.establish_impl_async(addr, socket)
                    .await
            }
        }
    }

    /// Initiate async TLS connection to the given address
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
    /// #[tokio::main]
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let association = ClientAssociationOptions::new()
    ///     .with_abstract_syntax("1.2.840.10008.1.1")
    ///     // called AE title in address
    ///     .establish_with_async_tls("MY-STORAGE@10.0.0.100:104")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tls")]
    #[allow(unreachable_patterns)]
    pub async fn establish_with_async_tls(
        self,
        ae_address: &str,
    ) -> Result<AsyncClientAssociation<AsyncTlsStream>> {
        match (&self.tls_config, &self.server_name) {
            (Some(tls_config), Some(server_name)) => {
                match ae_address.try_into() {
                    Ok(ae_address) => {
                        let socket = async_tls_connection(
                            &ae_address, server_name, &self.socket_options, tls_config.clone()
                        ).await?;
                        self.establish_impl_async(ae_address, socket).await
                    },
                    Err(_) => {
                        let addr = AeAddr::new_socket_addr(ae_address);
                        let socket = async_tls_connection(
                            &addr, server_name, &self.socket_options, tls_config.clone()
                        ).await?;
                        self.establish_impl_async(addr, socket).await
                    },
                }

            },
            _ => crate::association::TlsConfigMissingSnafu.fail()?
        }
    }
}

#[cfg(feature = "async")]
impl<S> Association for AsyncClientAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    fn peer_ae_title(&self) -> &str {
        &self.peer_ae_title
    }

    fn acceptor_max_pdu_length(&self) -> u32 {
        self.acceptor_max_pdu_length
    }

    fn requestor_max_pdu_length(&self) -> u32 {
        self.requestor_max_pdu_length 
    }

    fn presentation_contexts(&self) -> &[PresentationContextNegotiated] {
        &self.presentation_contexts
    }

    fn user_variables(&self) -> &[UserVariableItem] {
        &self.user_variables
    }
}

#[cfg(feature = "async")]
impl<S> AsyncClientAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    /// Retrieve read timeout for the association
    pub fn read_timeout(&self) -> Option<Duration> {
        self.read_timeout
    }

    /// Retrieve write timeout for the association
    pub fn write_timeout(&self) -> Option<Duration> {
        self.write_timeout
    }
}

#[cfg(feature = "async")]
impl<S> super::private::AsyncAssociationSealed<S> for AsyncClientAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    async fn send(&mut self, msg: &Pdu) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        self.write_buffer.clear();
        encode_pdu(&mut self.write_buffer, msg, self.acceptor_max_pdu_length + PDU_HEADER_SIZE)?;
        super::timeout(self.write_timeout, async {
            self.socket
                .write_all(&self.write_buffer)
                .await
                .context(crate::association::WireSendSnafu)
        })
        .await
    }

    async fn receive(&mut self) -> Result<Pdu> {
        use crate::association::read_pdu_from_wire_async;
        super::timeout(self.read_timeout, async {
            read_pdu_from_wire_async(
                &mut self.socket,
                &mut self.read_buffer,
                self.requestor_max_pdu_length,
                self.strict
            ).await
        })
        .await
    }

    async fn close(&mut self) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        self.socket.shutdown().await
    }
}

#[cfg(feature = "async")]
impl<S> super::AsyncAssociation<S> for AsyncClientAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send{

    fn inner_stream(&mut self) -> &mut S {
        &mut self.socket
    }

    fn get_mut(&mut self) -> (&mut S, &mut BytesMut) {
        let Self { socket, read_buffer, .. } = self;
        (socket, read_buffer)
    }
}

#[cfg(feature = "async")]
impl<S> Drop for AsyncClientAssociation<S>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    fn drop(&mut self) {
        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                let _ = crate::association::private::AsyncAssociationSealed::release(self).await;
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "async")]
    use crate::association::read_pdu_from_wire_async;
    use std::io::Write;


    impl<'a> ClientAssociationOptions<'a> {
        pub(crate) fn establish_with_extra_pdus<T>(
            &self,
            ae_address: AeAddr<T>,
            extra_pdus: Vec<Pdu>,
        ) -> Result<ClientAssociation<std::net::TcpStream>>
        where
            T: ToSocketAddrs,
        {
            let (pc_proposed, a_associate) = self.create_a_associate_req(ae_address.ae_title())?;
            let mut socket = tcp_connection(&ae_address, &self.socket_options)?;
            let mut write_buffer: Vec<u8> = Vec::with_capacity(DEFAULT_MAX_PDU as usize);
            // send request

            write_pdu(&mut write_buffer, &a_associate).context(crate::association::SendPduSnafu)?;
            for pdu in extra_pdus {
                write_pdu(&mut write_buffer, &pdu).context(crate::association::SendPduSnafu)?;
            }
            socket.write_all(&write_buffer).context(crate::association::WireSendSnafu)?;
            write_buffer.clear();

            let mut read_buffer = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let resp = read_pdu_from_wire(
                &mut socket,
                &mut read_buffer,
                self.max_pdu_length,
                self.strict,
            )?;
            let NegotiatedOptions {
                presentation_contexts,
                peer_max_pdu_length,
                user_variables,
                peer_ae_title
            } = self
                .process_a_association_resp(resp, &pc_proposed)
                .expect("Failed to process a associate response");
            Ok(ClientAssociation {
                presentation_contexts,
                requestor_max_pdu_length: self.max_pdu_length,
                acceptor_max_pdu_length: peer_max_pdu_length,
                socket,
                write_buffer,
                strict: self.strict,
                // Fixes #589, instead of creating a new buffer, we pass the existing buffer into the Association object.
                read_buffer,
                read_timeout: self.socket_options.read_timeout,
                write_timeout: self.socket_options.write_timeout,
                user_variables,
                peer_ae_title
            })
        }

        #[cfg(feature = "async")]
        pub(crate) async fn establish_with_extra_pdus_async<T>(
            &self,
            ae_address: AeAddr<T>,
            extra_pdus: Vec<Pdu>,
        ) -> Result<AsyncClientAssociation<tokio::net::TcpStream>>
        where
            T: tokio::net::ToSocketAddrs,
        {
            use tokio::io::AsyncWriteExt;

            let (pc_proposed, a_associate) = self.create_a_associate_req(ae_address.ae_title())?;
            let mut socket = async_connection(&ae_address, &self.socket_options).await?;
            let mut buffer: Vec<u8> = Vec::with_capacity(DEFAULT_MAX_PDU as usize);
            // send request

            write_pdu(&mut buffer, &a_associate).context(crate::association::SendPduSnafu)?;
            for pdu in extra_pdus {
                write_pdu(&mut buffer, &pdu).context(crate::association::SendPduSnafu)?;
            }
            socket.write_all(&buffer).await.context(crate::association::WireSendSnafu)?;
            buffer.clear();

            let mut buf = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let resp =
                read_pdu_from_wire_async(&mut socket, &mut buf, self.max_pdu_length, self.strict)
                    .await?;
            let NegotiatedOptions {
                presentation_contexts,
                peer_max_pdu_length,
                user_variables,
                peer_ae_title
            } = self
                .process_a_association_resp(resp, &pc_proposed)
                .expect("Failed to process a associate response");
            Ok(AsyncClientAssociation {
                presentation_contexts,
                requestor_max_pdu_length: self.max_pdu_length,
                acceptor_max_pdu_length: peer_max_pdu_length,
                socket,
                write_buffer: buffer,
                strict: self.strict,
                // Fixes #589, instead of creating a new buffer, we pass the existing buffer into the Association object.
                read_buffer: buf,
                read_timeout: self.socket_options.read_timeout,
                write_timeout: self.socket_options.write_timeout,
                user_variables,
                peer_ae_title
            })
        }

        // Broken implementation of server establish which reproduces behavior that #589 introduced
        pub fn broken_establish<T>(
            &self,
            ae_address: AeAddr<T>,
        ) -> Result<ClientAssociation<std::net::TcpStream>>
        where
            T: ToSocketAddrs,
        {
            let (pc_proposed, a_associate) = self.create_a_associate_req(ae_address.ae_title())?;
            let mut socket = tcp_connection(&ae_address, &self.socket_options)?;
            let mut buffer: Vec<u8> = Vec::with_capacity(DEFAULT_MAX_PDU as usize);
            // send request
            write_pdu(&mut buffer, &a_associate).context(crate::association::SendPduSnafu)?;
            socket.write_all(&buffer).context(crate::association::WireSendSnafu)?;
            buffer.clear();

            let mut buf = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let resp = read_pdu_from_wire(&mut socket, &mut buf, self.max_pdu_length, self.strict)?;
            let NegotiatedOptions {
                presentation_contexts,
                peer_max_pdu_length,
                user_variables,
                peer_ae_title
            } = self
                .process_a_association_resp(resp, &pc_proposed)
                .expect("Failed to process a associate response");
            Ok(ClientAssociation {
                presentation_contexts,
                requestor_max_pdu_length: self.max_pdu_length,
                acceptor_max_pdu_length: peer_max_pdu_length,
                socket,
                write_buffer: buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(
                    (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
                ),
                read_timeout: self.socket_options.read_timeout,
                write_timeout: self.socket_options.write_timeout,
                user_variables,
                peer_ae_title
            })
        }

        #[cfg(feature = "async")]
        // Broken implementation of server establish which reproduces behavior that #589 introduced
        pub async fn broken_establish_async<T>(
            &self,
            ae_address: AeAddr<T>,
        ) -> Result<AsyncClientAssociation<tokio::net::TcpStream>>
        where
            T: tokio::net::ToSocketAddrs,
        {
            use tokio::io::AsyncWriteExt;

            let (pc_proposed, a_associate) = self.create_a_associate_req(ae_address.ae_title())?;
            let mut socket = async_connection(&ae_address, &self.socket_options).await?;
            let mut buffer: Vec<u8> = Vec::with_capacity(DEFAULT_MAX_PDU as usize);
            // send request
            write_pdu(&mut buffer, &a_associate).context(crate::association::SendPduSnafu)?;
            socket.write_all(&buffer).await.context(crate::association::WireSendSnafu)?;
            buffer.clear();

            let mut buf = BytesMut::with_capacity(
                (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
            );
            let resp =
                read_pdu_from_wire_async(&mut socket, &mut buf, self.max_pdu_length, self.strict)
                    .await?;
            let NegotiatedOptions {
                presentation_contexts,
                peer_max_pdu_length,
                user_variables,
                peer_ae_title
            } = self
                .process_a_association_resp(resp, &pc_proposed)
                .expect("Failed to process a associate response");
            Ok(AsyncClientAssociation {
                presentation_contexts,
                requestor_max_pdu_length: self.max_pdu_length,
                acceptor_max_pdu_length: peer_max_pdu_length,
                socket,
                write_buffer: buffer,
                strict: self.strict,
                read_buffer: BytesMut::with_capacity(
                    (self.max_pdu_length.min(LARGE_PDU_SIZE) + PDU_HEADER_SIZE) as usize,
                ),
                read_timeout: self.socket_options.read_timeout,
                write_timeout: self.socket_options.write_timeout,
                user_variables,
                peer_ae_title
            })
        }
    }
}
