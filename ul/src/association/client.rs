//! Association requester module
//!
//! The module provides an abstraction for a DICOM association
//! in which this application entity is the one requesting the association.
//! See [`ClientAssociationOptions`]
//! for details and examples on how to create an association.
use bytes::BytesMut;
use std::{borrow::Cow, convert::TryInto, io::Cursor, net::ToSocketAddrs, time::Duration};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
};

use crate::{
    pdu::{
        read_pdu, write_pdu, AbortRQSource, AssociationAC, AssociationRJ, AssociationRQ, Pdu,
        PresentationContextProposed, PresentationContextResult, PresentationContextResultReason,
        ReadPduSnafu, UserIdentity, UserIdentityType, UserVariableItem, DEFAULT_MAX_PDU,
        MAXIMUM_PDU_SIZE,
    },
    AeAddr, IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME,
};
use snafu::{ensure, Backtrace, ResultExt, Snafu};

use bytes::Buf;

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
    SendRequest {
        #[snafu(backtrace)]
        source: crate::pdu::WriteError,
    },

    /// failed to receive association response
    ReceiveResponse {
        #[snafu(backtrace)]
        source: crate::pdu::ReadError,
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
        source: crate::pdu::WriteError,
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
        source: crate::pdu::ReadError,
    },

    #[snafu(display("Connection closed by peer"))]
    ConnectionClosed,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub fn get_client_pdu<R: Read>(reader: &mut R, max_pdu_length: u32, strict: bool) -> Result<Pdu> {
    // Receive response

    let mut read_buffer = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);
    let mut reader = BufReader::new(reader);

    let msg = loop {
        let mut buf = Cursor::new(&read_buffer[..]);
        match read_pdu(&mut buf, max_pdu_length, strict).context(ReceiveResponseSnafu)? {
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
            .context(ReceiveSnafu)?
            .to_vec();
        reader.consume(recv.len());
        read_buffer.extend_from_slice(&recv);
        ensure!(!recv.is_empty(), ConnectionClosedSnafu);
    };
    Ok(msg)
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
/// > **⚠️ Warning:** It is highly recommended to set `timeout` to a reasonable value for the
/// > async client since there is _no_ default timeout on
/// > [`tokio::net::TcpStream`]
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
///    .timeout(Duration::from_secs(60))
///    .establish("129.168.0.5:104")?;
/// # Ok(())
/// # }
/// ```
///
/// ### Async
///
/// ```no_run
/// # use dicom_ul::association::client::ClientAssociationOptions;
/// # use std::time::Duration;
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let association = ClientAssociationOptions::new()
///    .with_presentation_context("1.2.840.10008.1.1", vec!["1.2.840.10008.1.2.1", "1.2.840.10008.1.2"])
///    .timeout(Duration::from_secs(60))
///    .establish_async("129.168.0.5:104")
///    .await?;
/// # Ok(())
/// # }
/// ```
///
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
    /// Timeout for individual send/receive operations
    timeout: Option<Duration>,
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
            max_pdu_length: DEFAULT_MAX_PDU,
            strict: true,
            username: None,
            password: None,
            kerberos_service_ticket: None,
            saml_assertion: None,
            jwt: None,
            timeout: None,
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

    /// Initiate the TCP connection to the given address
    /// and request a new DICOM association,
    /// negotiating the presentation contexts in the process.
    pub fn establish<A: ToSocketAddrs>(self, address: A) -> Result<ClientAssociation<TcpStream>> {
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
    pub fn establish_with(self, ae_address: &str) -> Result<ClientAssociation<TcpStream>> {
        match ae_address.try_into() {
            Ok(ae_address) => self.establish_impl(ae_address),
            Err(_) => self.establish_impl(AeAddr::new_socket_addr(ae_address)),
        }
    }

    /// Set the read timeout for the underlying TCP socket
    pub fn timeout(self, timeout: Duration) -> Self {
        Self {
            timeout: Some(timeout),
            ..self
        }
    }

    fn establish_impl<T>(self, ae_address: AeAddr<T>) -> Result<ClientAssociation<TcpStream>>
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
            username,
            password,
            kerberos_service_ticket,
            saml_assertion,
            jwt,
            timeout,
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
            UserVariableItem::MaxLength(max_pdu_length),
            UserVariableItem::ImplementationClassUID(IMPLEMENTATION_CLASS_UID.to_string()),
            UserVariableItem::ImplementationVersionName(IMPLEMENTATION_VERSION_NAME.to_string()),
        ];

        if let Some(user_identity) = Self::determine_user_identity(
            username,
            password,
            kerberos_service_ticket,
            saml_assertion,
            jwt,
        ) {
            user_variables.push(UserVariableItem::UserIdentityItem(user_identity));
        }

        let msg = Pdu::AssociationRQ(AssociationRQ {
            protocol_version,
            calling_ae_title: calling_ae_title.to_string(),
            called_ae_title: called_ae_title.to_string(),
            application_context_name: application_context_name.to_string(),
            presentation_contexts,
            user_variables,
        });

        let mut socket = std::net::TcpStream::connect(ae_address).context(ConnectSnafu)?;
        socket
            .set_read_timeout(timeout)
            .context(SetReadTimeoutSnafu)?;
        socket
            .set_write_timeout(timeout)
            .context(SetWriteTimeoutSnafu)?;
        let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
        // send request

        write_pdu(&mut buffer, &msg).context(SendRequestSnafu)?;
        socket.write_all(&buffer).context(WireSendSnafu)?;
        buffer.clear();

        let msg = get_client_pdu(&mut socket, MAXIMUM_PDU_SIZE, self.strict)?;

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
                    read_buffer: BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize),
                    timeout,
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

pub trait CloseSocket {
    fn close(&mut self) -> std::io::Result<()>;
}

impl CloseSocket for TcpStream {
    fn close(&mut self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Both)
    }
}
pub trait Release {
    fn release(&mut self) -> Result<()>;
}

impl Release for ClientAssociation<TcpStream> {
    fn release(&mut self) -> Result<()> {
        self.release_impl()
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
pub struct ClientAssociation<S>
where
    S: CloseSocket,
    ClientAssociation<S>: Release,
{
    /// The presentation contexts accorded with the acceptor application entity,
    /// without the rejected ones.
    presentation_contexts: Vec<PresentationContextResult>,
    /// The maximum PDU length that this application entity is expecting to receive
    requestor_max_pdu_length: u32,
    /// The maximum PDU length that the remote application entity accepts
    acceptor_max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: S,
    /// Buffer to assemble PDU before sending it on wire
    buffer: Vec<u8>,
    /// whether to receive PDUs in strict mode
    strict: bool,
    /// Timeout for individual Send/Receive operations
    timeout: Option<Duration>,
    /// Buffer to assemble PDU before parsing
    read_buffer: BytesMut,
}

impl<S: CloseSocket> ClientAssociation<S>
where
    ClientAssociation<S>: Release,
{
    /// Retrieve timeout for the association
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }
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
}

impl ClientAssociation<TcpStream>
where
    ClientAssociation<TcpStream>: Release,
{
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
        use std::io::{BufRead, BufReader, Cursor};

        let mut reader = BufReader::new(&mut self.socket);

        loop {
            let mut buf = Cursor::new(&self.read_buffer[..]);
            match read_pdu(&mut buf, self.acceptor_max_pdu_length, self.strict)
                .context(ReceiveResponseSnafu)?
            {
                Some(pdu) => {
                    self.read_buffer.advance(buf.position() as usize);
                    return Ok(pdu);
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
                .context(ReceiveSnafu)?
                .to_vec();
            reader.consume(recv.len());
            self.read_buffer.extend_from_slice(&recv);
            ensure!(!recv.is_empty(), ConnectionClosedSnafu);
        }
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
        let pdu = self.receive()?;

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
impl<T> Drop for ClientAssociation<T>
where
    T: CloseSocket,
    ClientAssociation<T>: Release,
{
    fn drop(&mut self) {
        let _ = self.release();
        let _ = self.socket.close();
    }
}

#[cfg(feature = "async")]
pub mod non_blocking {
    use std::{convert::TryInto, io::Cursor, net::ToSocketAddrs};

    use crate::{
        association::{
            client::{
                ConnectSnafu, ConnectionClosedSnafu, MissingAbstractSyntaxSnafu,
                NoAcceptedPresentationContextsSnafu, ProtocolVersionMismatchSnafu,
                ReceiveResponseSnafu, ReceiveSnafu, RejectedSnafu, SendRequestSnafu,
                UnexpectedResponseSnafu, UnknownResponseSnafu,
            },
            pdata::non_blocking::{AsyncPDataWriter, PDataReader},
        },
        pdu::{
            AbortRQSource, AssociationAC, AssociationRQ, PresentationContextProposed,
            PresentationContextResultReason, ReadPduSnafu, UserVariableItem, DEFAULT_MAX_PDU,
            MAXIMUM_PDU_SIZE,
        },
        read_pdu, write_pdu, AeAddr, Pdu, IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME,
    };

    use super::{
        ClientAssociation, ClientAssociationOptions, CloseSocket, Release, Result, SendSnafu,
        SendTooLongPduSnafu, WireSendSnafu,
    };
    use bytes::{Buf, BytesMut};
    use snafu::{ensure, ResultExt};
    use tokio::{
        io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    use tracing::warn;

    pub async fn get_client_pdu_async<R: AsyncRead + Unpin>(
        reader: &mut R,
        max_pdu_length: u32,
        strict: bool,
    ) -> Result<Pdu> {
        // receive response
        use tokio::io::AsyncReadExt;
        let mut read_buffer = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);

        let msg = loop {
            let mut buf = Cursor::new(&read_buffer[..]);
            match read_pdu(&mut buf, max_pdu_length, strict).context(ReceiveResponseSnafu)? {
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
                .read_buf(&mut read_buffer)
                .await
                .context(ReadPduSnafu)
                .context(ReceiveSnafu)?;
            ensure!(recv > 0, ConnectionClosedSnafu);
        };
        Ok(msg)
    }

    impl<'a> ClientAssociationOptions<'a> {
        async fn establish_impl_async<T>(
            self,
            ae_address: AeAddr<T>,
        ) -> Result<ClientAssociation<tokio::net::TcpStream>>
        where
            T: ToSocketAddrs,
        {
            let timeout = self.timeout;
            let task = async {
                let ClientAssociationOptions {
                    calling_ae_title,
                    called_ae_title,
                    application_context_name,
                    presentation_contexts,
                    protocol_version,
                    max_pdu_length,
                    strict,
                    username,
                    password,
                    kerberos_service_ticket,
                    saml_assertion,
                    jwt,
                    timeout,
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
                    UserVariableItem::MaxLength(max_pdu_length),
                    UserVariableItem::ImplementationClassUID(IMPLEMENTATION_CLASS_UID.to_string()),
                    UserVariableItem::ImplementationVersionName(
                        IMPLEMENTATION_VERSION_NAME.to_string(),
                    ),
                ];

                if let Some(user_identity) = Self::determine_user_identity(
                    username,
                    password,
                    kerberos_service_ticket,
                    saml_assertion,
                    jwt,
                ) {
                    user_variables.push(UserVariableItem::UserIdentityItem(user_identity));
                }

                let msg = Pdu::AssociationRQ(AssociationRQ {
                    protocol_version,
                    calling_ae_title: calling_ae_title.to_string(),
                    called_ae_title: called_ae_title.to_string(),
                    application_context_name: application_context_name.to_string(),
                    presentation_contexts,
                    user_variables,
                });
                let socket_addrs: Vec<_> = ae_address.to_socket_addrs().unwrap().collect();

                let mut socket = TcpStream::connect(socket_addrs.as_slice())
                    .await
                    .context(ConnectSnafu)?;
                let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
                // send request

                write_pdu(&mut buffer, &msg).context(SendRequestSnafu)?;
                socket.write_all(&buffer).await.context(WireSendSnafu)?;
                buffer.clear();

                // receive response
                let msg = get_client_pdu_async(&mut socket, MAXIMUM_PDU_SIZE, self.strict).await?;

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
                            let _ = socket.write_all(&buffer).await;
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
                            timeout,
                            read_buffer: BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize),
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
                        let _ = socket.write_all(&buffer).await;
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
                        let _ = socket.write_all(&buffer).await;
                        UnknownResponseSnafu { pdu }.fail()
                    }
                }
            };
            if let Some(timeout) = timeout {
                tokio::time::timeout(timeout, task)
                    .await
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))
                    .context(ConnectSnafu)?
            } else {
                warn!("No timeout set. It is highly recommended to set a timeout.");
                task.await
            }
        }

        /// Initiate the TCP connection to the given address
        /// and request a new DICOM association,
        /// negotiating the presentation contexts in the process.
        pub async fn establish_async<A: ToSocketAddrs>(
            self,
            address: A,
        ) -> Result<ClientAssociation<TcpStream>> {
            self.establish_impl_async(AeAddr::new_socket_addr(address))
                .await
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
        pub async fn establish_with_async(
            self,
            ae_address: &str,
        ) -> Result<ClientAssociation<TcpStream>> {
            match ae_address.try_into() {
                Ok(ae_address) => self.establish_impl_async(ae_address).await,
                Err(_) => {
                    self.establish_impl_async(AeAddr::new_socket_addr(ae_address))
                        .await
                }
            }
        }
    }

    impl ClientAssociation<TcpStream>
    where
        ClientAssociation<TcpStream>: Release,
    {
        /// Send a PDU message to the other intervenient.
        pub async fn send(&mut self, msg: &Pdu) -> Result<()> {
            let timeout = self.timeout;
            let task = async {
                self.buffer.clear();
                write_pdu(&mut self.buffer, msg).context(SendSnafu)?;
                if self.buffer.len() > self.acceptor_max_pdu_length as usize {
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
            let timeout = self.timeout;
            let task = async {
                loop {
                    let mut buf = Cursor::new(&self.read_buffer[..]);
                    match read_pdu(&mut buf, self.requestor_max_pdu_length, self.strict)
                        .context(ReceiveResponseSnafu)?
                    {
                        Some(pdu) => {
                            self.read_buffer.advance(buf.position() as usize);
                            return Ok(pdu);
                        }
                        None => {
                            // Reset position
                            buf.set_position(0)
                        }
                    }
                    let recv = self
                        .socket
                        .read_buf(&mut self.read_buffer)
                        .await
                        .context(ReadPduSnafu)
                        .context(ReceiveSnafu)?;
                    ensure!(recv > 0, ConnectionClosedSnafu);
                }
            };
            if let Some(timeout) = timeout {
                tokio::time::timeout(timeout, task)
                    .await
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))
                    .context(ReadPduSnafu)
                    .context(ReceiveSnafu)?
            } else {
                task.await
            }
        }

        /// Gracefully terminate the association by exchanging release messages
        /// and then shutting down the TCP connection.
        pub async fn release(mut self) -> Result<()> {
            let timeout = self.timeout;
            let task = async {
                let out = self.release_impl().await;
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

        /// Send an abort message and shut down the TCP connection,
        /// terminating the association.
        pub async fn abort(mut self) -> Result<()> {
            let timeout = self.timeout;
            let task = async {
                let pdu = Pdu::AbortRQ {
                    source: AbortRQSource::ServiceUser,
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

        /// Prepare a P-Data writer for sending
        /// one or more data items.
        ///
        /// Returns a writer which automatically
        /// splits the inner data into separate PDUs if necessary.
        pub async fn send_pdata(
            &mut self,
            presentation_context_id: u8,
        ) -> AsyncPDataWriter<&mut TcpStream> {
            AsyncPDataWriter::new(
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
        #[cfg(feature = "async")]
        pub fn receive_pdata(&mut self) -> PDataReader<&mut TcpStream> {
            PDataReader::new(&mut self.socket, self.requestor_max_pdu_length)
        }

        /// Release implementation function,
        /// which tries to send a release request and receive a release response.
        /// This is in a separate private function because
        /// terminating a connection should still close the connection
        /// if the exchange fails.
        async fn release_impl(&mut self) -> Result<()> {
            let pdu = Pdu::ReleaseRQ;
            self.send(&pdu).await?;
            use tokio::io::AsyncReadExt;
            let mut read_buffer = BytesMut::with_capacity(MAXIMUM_PDU_SIZE as usize);

            let pdu = loop {
                if let Ok(Some(pdu)) = read_pdu(&mut read_buffer, MAXIMUM_PDU_SIZE, self.strict) {
                    break pdu;
                }
                let recv = self
                    .socket
                    .read_buf(&mut read_buffer)
                    .await
                    .context(ReadPduSnafu)
                    .context(ReceiveSnafu)?;
                ensure!(recv > 0, ConnectionClosedSnafu);
            };
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

    impl Release for ClientAssociation<TcpStream> {
        fn release(&mut self) -> super::Result<()> {
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move { self.release_impl().await })
            })
        }
    }
    /// Automatically release the association and shut down the connection.
    impl CloseSocket for TcpStream {
        fn close(&mut self) -> std::io::Result<()> {
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move { self.shutdown().await })
            })
        }
    }
}
