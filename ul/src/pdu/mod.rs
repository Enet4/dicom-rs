//! Protocol Data Unit module
//!
//! This module comprises multiple data structures representing possible
//! protocol data units (PDUs) according to
//! the standard message exchange mechanisms,
//! as well as readers and writers of PDUs from arbitrary data sources.
pub mod reader;
pub mod writer;

use std::fmt::Display;

pub use reader::read_pdu;
pub use writer::write_pdu;

/// Message component for a proposed presentation context.
#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub struct PresentationContextProposed {
    /// the presentation context identifier
    pub id: u8,
    /// the expected abstract syntax UID
    /// (commonly referrering to the expected SOP class)
    pub abstract_syntax: String,
    /// a list of transfer syntax UIDs to support in this interaction
    pub transfer_syntaxes: Vec<String>,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub struct PresentationContextResult {
    pub id: u8,
    pub reason: PresentationContextResultReason,
    pub transfer_syntax: String,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum PresentationContextResultReason {
    Acceptance = 0,
    UserRejection = 1,
    NoReason = 2,
    AbstractSyntaxNotSupported = 3,
    TransferSyntaxesNotSupported = 4,
}

impl PresentationContextResultReason {
    fn from(reason: u8) -> Option<PresentationContextResultReason> {
        let result = match reason {
            0 => PresentationContextResultReason::Acceptance,
            1 => PresentationContextResultReason::UserRejection,
            2 => PresentationContextResultReason::NoReason,
            3 => PresentationContextResultReason::AbstractSyntaxNotSupported,
            4 => PresentationContextResultReason::TransferSyntaxesNotSupported,
            _ => {
                return None;
            }
        };

        Some(result)
    }
}

impl Display for PresentationContextResultReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            PresentationContextResultReason::Acceptance => "acceptance",
            PresentationContextResultReason::UserRejection => "user rejection",
            PresentationContextResultReason::NoReason => "no reason",
            PresentationContextResultReason::AbstractSyntaxNotSupported => {
                "abstract syntax not supported"
            }
            PresentationContextResultReason::TransferSyntaxesNotSupported => {
                "transfer syntaxes not supported"
            }
        };
        f.write_str(msg)
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJResult {
    Permanent = 1,
    Transient = 2,
}

impl AssociationRJResult {
    fn from(value: u8) -> Option<AssociationRJResult> {
        match value {
            1 => Some(AssociationRJResult::Permanent),
            2 => Some(AssociationRJResult::Transient),
            _ => None,
        }
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJSource {
    ServiceUser(AssociationRJServiceUserReason),
    ServiceProviderASCE(AssociationRJServiceProviderASCEReason),
    ServiceProviderPresentation(AssociationRJServiceProviderPresentationReason),
}

impl AssociationRJSource {
    fn from(source: u8, reason: u8) -> Option<AssociationRJSource> {
        let result = match (source, reason) {
            (1, 1) => {
                AssociationRJSource::ServiceUser(AssociationRJServiceUserReason::NoReasonGiven)
            }
            (1, 2) => AssociationRJSource::ServiceUser(
                AssociationRJServiceUserReason::ApplicationContextNameNotSupported,
            ),
            (1, 3) => AssociationRJSource::ServiceUser(
                AssociationRJServiceUserReason::CallingAETitleNotRecognized,
            ),
            (1, x) if x == 4 || x == 5 || x == 6 => {
                AssociationRJSource::ServiceUser(AssociationRJServiceUserReason::Reserved(x))
            }
            (1, 7) => AssociationRJSource::ServiceUser(
                AssociationRJServiceUserReason::CalledAETitleNotRecognized,
            ),
            //(1, 8) | (1, 9) | (1, 10) => {
            (1, x) if x == 8 || x == 9 || x == 10 => {
                AssociationRJSource::ServiceUser(AssociationRJServiceUserReason::Reserved(x))
            }
            (1, _) => {
                return None;
            }
            (2, 1) => AssociationRJSource::ServiceProviderASCE(
                AssociationRJServiceProviderASCEReason::NoReasonGiven,
            ),
            (2, 2) => AssociationRJSource::ServiceProviderASCE(
                AssociationRJServiceProviderASCEReason::ProtocolVersionNotSupported,
            ),
            (2, _) => {
                return None;
            }
            (3, 0) => AssociationRJSource::ServiceProviderPresentation(
                AssociationRJServiceProviderPresentationReason::Reserved(0),
            ),
            (3, 1) => AssociationRJSource::ServiceProviderPresentation(
                AssociationRJServiceProviderPresentationReason::TemporaryCongestion,
            ),
            (3, 2) => AssociationRJSource::ServiceProviderPresentation(
                AssociationRJServiceProviderPresentationReason::LocalLimitExceeded,
            ),
            (3, x) if x == 3 || x == 4 || x == 5 || x == 6 || x == 7 => {
                AssociationRJSource::ServiceProviderPresentation(
                    AssociationRJServiceProviderPresentationReason::Reserved(x),
                )
            }
            _ => {
                return None;
            }
        };
        Some(result)
    }
}

impl Display for AssociationRJSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssociationRJSource::ServiceUser(r) => Display::fmt(r, f),
            AssociationRJSource::ServiceProviderASCE(r) => Display::fmt(r, f),
            AssociationRJSource::ServiceProviderPresentation(r) => Display::fmt(r, f),
        }
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJServiceUserReason {
    NoReasonGiven,
    ApplicationContextNameNotSupported,
    CallingAETitleNotRecognized,
    CalledAETitleNotRecognized,
    Reserved(u8),
}

impl Display for AssociationRJServiceUserReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssociationRJServiceUserReason::NoReasonGiven => f.write_str("no reason given"),
            AssociationRJServiceUserReason::ApplicationContextNameNotSupported => {
                f.write_str("application context name not supported")
            }
            AssociationRJServiceUserReason::CallingAETitleNotRecognized => {
                f.write_str("calling AE title not recognized")
            }
            AssociationRJServiceUserReason::CalledAETitleNotRecognized => {
                f.write_str("called AE title not recognized")
            }
            AssociationRJServiceUserReason::Reserved(code) => write!(f, "reserved code {}", code),
        }
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJServiceProviderASCEReason {
    NoReasonGiven,
    ProtocolVersionNotSupported,
}

impl Display for AssociationRJServiceProviderASCEReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssociationRJServiceProviderASCEReason::NoReasonGiven => f.write_str("no reason given"),
            AssociationRJServiceProviderASCEReason::ProtocolVersionNotSupported => {
                f.write_str("protocol version not supported")
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJServiceProviderPresentationReason {
    TemporaryCongestion,
    LocalLimitExceeded,
    Reserved(u8),
}

impl Display for AssociationRJServiceProviderPresentationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssociationRJServiceProviderPresentationReason::TemporaryCongestion => {
                f.write_str("temporary congestion")
            }
            AssociationRJServiceProviderPresentationReason::LocalLimitExceeded => {
                f.write_str("local limit exceeded")
            }
            AssociationRJServiceProviderPresentationReason::Reserved(code) => {
                write!(f, "reserved code {}", code)
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub struct PDataValue {
    pub presentation_context_id: u8,
    pub value_type: PDataValueType,
    pub is_last: bool,
    pub data: Vec<u8>,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum PDataValueType {
    Command,
    Data,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AbortRQSource {
    ServiceUser,
    ServiceProvider(AbortRQServiceProviderReason),
    Reserved,
}

impl AbortRQSource {
    fn from(source: u8, reason: u8) -> Option<AbortRQSource> {
        let result = match (source, reason) {
            (0, _) => AbortRQSource::ServiceUser,
            (1, _) => AbortRQSource::Reserved,
            (2, 0) => {
                AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::ReasonNotSpecified)
            }
            (2, 1) => AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::UnrecognizedPdu),
            (2, 2) => AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::UnexpectedPdu),
            (2, 3) => AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::Reserved),
            (2, 4) => AbortRQSource::ServiceProvider(
                AbortRQServiceProviderReason::UnrecognizedPduParameter,
            ),
            (2, 5) => {
                AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::UnexpectedPduParameter)
            }
            (2, 6) => {
                AbortRQSource::ServiceProvider(AbortRQServiceProviderReason::InvalidPduParameter)
            }
            (_, _) => {
                return None;
            }
        };

        Some(result)
    }
}

/// An enumeration of supported A-ABORT PDU provider reasons.
#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AbortRQServiceProviderReason {
    /// Reason Not Specified
    ReasonNotSpecified,
    /// Unrecognized PDU
    UnrecognizedPdu,
    /// Unexpected PDU
    UnexpectedPdu,
    /// Reserved
    Reserved,
    /// Unrecognized PDU parameter
    UnrecognizedPduParameter,
    /// Unexpected PDU parameter
    UnexpectedPduParameter,
    /// Invalid PDU parameter
    InvalidPduParameter,
}

impl Display for AbortRQServiceProviderReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            AbortRQServiceProviderReason::ReasonNotSpecified => "reason not specified",
            AbortRQServiceProviderReason::UnrecognizedPdu => "unrecognized PDU",
            AbortRQServiceProviderReason::UnexpectedPdu => "unexpected PDU",
            AbortRQServiceProviderReason::Reserved => "reserved code",
            AbortRQServiceProviderReason::UnrecognizedPduParameter => "unrecognized PDU parameter",
            AbortRQServiceProviderReason::UnexpectedPduParameter => "unexpected PDU parameter",
            AbortRQServiceProviderReason::InvalidPduParameter => "invalid PDU parameter",
        };
        f.write_str(msg)
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum PduVariableItem {
    Unknown(u8),
    ApplicationContext(String),
    PresentationContextProposed(PresentationContextProposed),
    PresentationContextResult(PresentationContextResult),
    UserVariables(Vec<UserVariableItem>),
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum UserVariableItem {
    Unknown(u8, Vec<u8>),
    MaxLength(u32),
    ImplementationClassUID(String),
    ImplementationVersionName(String),
    SopClassExtendedNegotiationSubItem(String, Vec<u8>),
    UserIdentityItem(UserIdentity),
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub struct UserIdentity {
    positive_response_requested: bool,
    identity_type: UserIdentityType,
    primary_field: Vec<u8>,
    secondary_field: Vec<u8>,
}
impl UserIdentity {
    pub fn new(
        positive_response_requested: bool,
        identity_type: UserIdentityType,
        primary_field: Vec<u8>,
        secondary_field: Vec<u8>,
    ) -> Self {
        UserIdentity {
            positive_response_requested,
            identity_type,
            primary_field,
            secondary_field,
        }
    }

    pub fn positive_response_requested(&self) -> bool {
        self.positive_response_requested
    }

    pub fn identity_type(&self) -> UserIdentityType {
        self.identity_type.clone()
    }

    pub fn primary_field(&self) -> Vec<u8> {
        self.primary_field.clone()
    }

    pub fn secondary_field(&self) -> Vec<u8> {
        self.secondary_field.clone()
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
#[non_exhaustive]
pub enum UserIdentityType {
    Username,
    UsernamePassword,
    KerberosServiceTicket,
    SamlAssertion,
    Jwt,
}
impl UserIdentityType {
    fn from(user_identity_type: u8) -> Option<Self> {
        match user_identity_type {
            1 => Some(Self::Username),
            2 => Some(Self::UsernamePassword),
            3 => Some(Self::KerberosServiceTicket),
            4 => Some(Self::SamlAssertion),
            5 => Some(Self::Jwt),
            _ => None,
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            Self::Username => 1,
            Self::UsernamePassword => 2,
            Self::KerberosServiceTicket => 3,
            Self::SamlAssertion => 4,
            Self::Jwt => 5,
        }
    }
}

/// An in-memory representation of a full Protocol Data Unit (PDU).
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Hash)]
pub enum Pdu {
    /// Unrecognized PDU type
    Unknown { pdu_type: u8, data: Vec<u8> },
    /// Association request (A-ASSOCIATION-RQ)
    AssociationRQ(AssociationRQ),
    /// Association acknowledgement (A-ASSOCIATION-AC)
    AssociationAC(AssociationAC),
    /// Association rejection (A-ASSOCIATION-RJ)
    AssociationRJ(AssociationRJ),
    /// P-Data
    PData { data: Vec<PDataValue> },
    /// Association release request (A-RELEASE-RQ)
    ReleaseRQ,
    /// Association release reply (A-RELEASE-RP)
    ReleaseRP,
    /// Association abort request (A-ABORT-RQ)
    AbortRQ { source: AbortRQSource },
}

impl Pdu {
    /// Provide a short description of the PDU.
    pub fn short_description(&self) -> impl std::fmt::Display + '_ {
        PduShortDescription(self)
    }
}

struct PduShortDescription<'a>(&'a Pdu);

impl std::fmt::Display for PduShortDescription<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Pdu::Unknown { pdu_type, data } => {
                write!(
                    f,
                    "Unknown {{pdu_type: {}, data: {} bytes }}",
                    pdu_type,
                    data.len()
                )
            }
            Pdu::AssociationRQ { .. }
            | Pdu::AssociationAC { .. }
            | Pdu::AssociationRJ { .. }
            | Pdu::ReleaseRQ
            | Pdu::ReleaseRP
            | Pdu::AbortRQ { .. } => std::fmt::Debug::fmt(self.0, f),
            Pdu::PData { data } => {
                if data.len() == 1 {
                    write!(
                        f,
                        "PData [({:?}, {} bytes)]",
                        data[0].value_type,
                        data[0].data.len()
                    )
                } else if data.len() == 2 {
                    write!(
                        f,
                        "PData [({:?}, {} bytes), ({:?}, {} bytes)]",
                        data[0].value_type,
                        data[0].data.len(),
                        data[1].value_type,
                        data[1].data.len(),
                    )
                } else {
                    write!(f, "PData [{} p-data values]", data.len())
                }
            }
        }
    }
}

/// An in-memory representation of an association request
#[derive(Debug, Clone, Eq, Hash, PartialEq, PartialOrd)]
pub struct AssociationRQ {
    pub protocol_version: u16,
    pub calling_ae_title: String,
    pub called_ae_title: String,
    pub application_context_name: String,
    pub presentation_contexts: Vec<PresentationContextProposed>,
    pub user_variables: Vec<UserVariableItem>,
}

impl From<AssociationRQ> for Pdu {
    fn from(value: AssociationRQ) -> Self {
        Pdu::AssociationRQ(value)
    }
}

/// An in-memory representation of an association acknowledgement
#[derive(Debug, Clone, Eq, Hash, PartialEq, PartialOrd)]
pub struct AssociationAC {
    pub protocol_version: u16,
    pub calling_ae_title: String,
    pub called_ae_title: String,
    pub application_context_name: String,
    pub presentation_contexts: Vec<PresentationContextResult>,
    pub user_variables: Vec<UserVariableItem>,
}

impl From<AssociationAC> for Pdu {
    fn from(value: AssociationAC) -> Self {
        Pdu::AssociationAC(value)
    }
}

/// An in-memory representation of an association rejection.
#[derive(Debug, Clone, Eq, Hash, PartialEq, PartialOrd)]
pub struct AssociationRJ {
    pub result: AssociationRJResult,
    pub source: AssociationRJSource,
}

impl From<AssociationRJ> for Pdu {
    fn from(value: AssociationRJ) -> Self {
        Pdu::AssociationRJ(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::pdu::{PDataValue, PDataValueType};

    use super::Pdu;

    #[test]
    fn pdu_short_description() {
        let pdu = Pdu::AbortRQ {
            source: super::AbortRQSource::ServiceUser,
        };
        assert_eq!(
            &pdu.short_description().to_string(),
            "AbortRQ { source: ServiceUser }",
        );

        let pdu = Pdu::PData {
            data: vec![PDataValue {
                is_last: true,
                presentation_context_id: 2,
                value_type: PDataValueType::Data,
                data: vec![0x55; 384],
            }],
        };
        assert_eq!(
            &pdu.short_description().to_string(),
            "PData [(Data, 384 bytes)]",
        );
    }
}
