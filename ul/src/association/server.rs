//! Association acceptor module
use std::{borrow::Cow, net::TcpStream};

use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use snafu::{ensure, ResultExt, Snafu};

use crate::pdu::{
    reader::read_pdu, writer::write_pdu, AssociationRJResult, AssociationRJServiceUserReason,
    AssociationRJSource, Pdu, PresentationContextResult, PresentationContextResultReason,
};

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// missing at least one abstract syntax to accept negotiations
    MissingAbstractSyntax,

    /// failed to receive association request
    ReceiveRequest { source: crate::pdu::reader::Error },

    /// failed to send association response
    SendResponse { source: crate::pdu::writer::Error },

    /// failed to send PDU
    Send { source: crate::pdu::writer::Error },

    /// failed to receive PDU
    Receive { source: crate::pdu::reader::Error },

    #[snafu(display("unexpected request from SCU `{:?}`", pdu))]
    #[non_exhaustive]
    UnexpectedRequest {
        /// the PDU obtained from the server
        pdu: Pdu,
    },

    #[snafu(display("unknown request from SCU `{:?}`", pdu))]
    #[non_exhaustive]
    UnknownRequest {
        /// the PDU obtained from the server, of variant Unknown
        pdu: Pdu,
    },

    /// association rejected
    Rejected,

    /// association aborted
    Aborted,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Common interface for application entity access control policies.
///
/// Existing implementations include [`AcceptAny`] and [`AcceptCalledAeTitle`],
/// but users are free to implement their own.
pub trait AccessControl {
    /// Obtain the decision of whether to accept an incoming association request
    /// based on the recorded application entity titles.
    ///
    /// Returns Ok(()) if the requester node should be given clearance.
    /// Otherwise, a concrete association RJ service user reason is given.
    fn check_access(
        &self,
        this_ae_title: &str,
        calling_ae_title: &str,
        called_ae_title: &str,
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
/// supported by the main [transfer syntax registry][1].
///
/// Access control logic is provided through
///
/// [1]: dicom_transfer_syntax_registry
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
            max_pdu_length: crate::pdu::reader::DEFAULT_MAX_PDU,
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
    pub fn ae_access_control<P>(self, access_control: P) -> ServerAssociationOptions<'a, P> {
        let ServerAssociationOptions {
            ae_title,
            application_context_name,
            abstract_syntax_uids,
            transfer_syntax_uids,
            protocol_version,
            max_pdu_length,
            ..
        } = self;

        ServerAssociationOptions {
            ae_access_control: access_control,
            ae_title,
            application_context_name,
            abstract_syntax_uids,
            transfer_syntax_uids,
            protocol_version,
            max_pdu_length,
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

    /// Negotiate an association with the given TCP stream.
    pub fn establish(&self, mut socket: TcpStream) -> Result<ServerAssociation> {
        ensure!(!self.abstract_syntax_uids.is_empty(), MissingAbstractSyntax);

        let max_pdu_length = self.max_pdu_length;

        let pdu = read_pdu(&mut socket, max_pdu_length).context(ReceiveRequest)?;

        match pdu {
            Pdu::AssociationRQ {
                protocol_version,
                calling_ae_title,
                called_ae_title,
                application_context_name,
                presentation_contexts,
                user_variables: _,
            } => {
                if protocol_version != self.protocol_version {
                    write_pdu(
                        &mut socket,
                        &Pdu::AssociationRJ {
                            result: AssociationRJResult::Permanent,
                            source: AssociationRJSource::ServiceUser(
                                AssociationRJServiceUserReason::NoReasonGiven,
                            ),
                        },
                    )
                    .context(SendResponse)?;
                    return Rejected.fail();
                }

                if application_context_name != self.application_context_name {
                    write_pdu(
                        &mut socket,
                        &Pdu::AssociationRJ {
                            result: AssociationRJResult::Permanent,
                            source: AssociationRJSource::ServiceUser(
                                AssociationRJServiceUserReason::ApplicationContextNameNotSupported,
                            ),
                        },
                    )
                    .context(SendResponse)?;
                    return Rejected.fail();
                }

                self.ae_access_control
                    .check_access(&self.ae_title, &calling_ae_title, &called_ae_title)
                    .map(Ok)
                    .unwrap_or_else(|reason| {
                        write_pdu(
                            &mut socket,
                            &Pdu::AssociationRJ {
                                result: AssociationRJResult::Permanent,
                                source: AssociationRJSource::ServiceUser(reason),
                            },
                        )
                        .context(SendResponse)?;
                        Rejected.fail()
                    })?;

                let presentation_contexts: Vec<_> = presentation_contexts
                    .into_iter()
                    .map(|pc| {
                        if !self
                            .abstract_syntax_uids
                            .contains(&Cow::from(pc.abstract_syntax))
                        {
                            return PresentationContextResult {
                                id: pc.id,
                                reason: PresentationContextResultReason::AbstractSyntaxNotSupported,
                                transfer_syntax: "1.2.840.10008.1.2".to_string(),
                            };
                        }

                        let mut reason = PresentationContextResultReason::Acceptance;
                        let transfer_syntax = choose_supported(pc.transfer_syntaxes)
                            .unwrap_or_else(|| {
                                reason =
                                    PresentationContextResultReason::TransferSyntaxesNotSupported;
                                "1.2.840.10008.1.2".to_string()
                            });

                        PresentationContextResult {
                            id: pc.id,
                            reason,
                            transfer_syntax,
                        }
                    })
                    .collect();

                write_pdu(
                    &mut socket,
                    &Pdu::AssociationAC {
                        protocol_version: self.protocol_version,
                        application_context_name,
                        presentation_contexts: presentation_contexts.clone(),
                        user_variables: vec![],
                    },
                )
                .context(SendResponse)?;

                Ok(ServerAssociation {
                    presentation_contexts,
                    max_pdu_length,
                    socket,
                })
            }
            Pdu::ReleaseRQ => {
                write_pdu(&mut socket, &Pdu::ReleaseRP).context(SendResponse)?;
                Aborted.fail()
            }
            pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::PData { .. }
            | pdu @ Pdu::ReleaseRP
            | pdu @ Pdu::AbortRQ { .. } => UnexpectedRequest { pdu }.fail(),
            pdu @ Pdu::Unknown { .. } => UnknownRequest { pdu }.fail(),
        }
    }
}

/// A DICOM upper level association from the perspective
/// of a service class provider (SCP).
#[derive(Debug)]
pub struct ServerAssociation {
    /// The accorded presentation contexts
    presentation_contexts: Vec<PresentationContextResult>,
    /// The maximum PDU length
    max_pdu_length: u32,
    /// The TCP stream to the other DICOM node
    socket: TcpStream,
}

impl ServerAssociation {

    /// Obtain a view of the negotiated presentation contexts.
    pub fn presentation_contexts(&self) -> &[PresentationContextResult] {
        &self.presentation_contexts
    }

    /// Send a PDU message to the other intervenient.
    pub fn send(&mut self, msg: &Pdu) -> Result<()> {
        write_pdu(&mut self.socket, &msg).context(Send)
    }

    /// Read a PDU message from the other intervenient.
    pub fn receive(&mut self) -> Result<Pdu> {
        read_pdu(&mut self.socket, self.max_pdu_length).context(Receive)
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
    ts_repo.get(ts_uid).filter(|ts| !ts.unsupported()).is_some()
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
