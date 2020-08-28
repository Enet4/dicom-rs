pub mod reader;
pub mod writer;

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub struct PresentationContextProposed {
    pub id: u8,
    pub abstract_syntax: String,
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

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJResult {
    Permanent,
    Transient,
}

impl AssociationRJResult {
    fn from(value: u8) -> Option<AssociationRJResult> {
        match value {
            0 => Some(AssociationRJResult::Permanent),
            1 => Some(AssociationRJResult::Transient),
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

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJServiceUserReason {
    NoReasonGiven,
    ApplicationContextNameNotSupported,
    CallingAETitleNotRecognized,
    CalledAETitleNotRecognized,
    Reserved(u8),
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJServiceProviderASCEReason {
    NoReasonGiven,
    ProtocolVersionNotSupported,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJServiceProviderPresentationReason {
    TemporaryCongestion,
    LocalLimitExceeded,
    Reserved(u8),
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
            (2, 0) => AbortRQSource::ServiceProvider(
                AbortRQServiceProviderReason::ReasonNotSpecifiedUnrecognizedPdu,
            ),
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

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AbortRQServiceProviderReason {
    ReasonNotSpecifiedUnrecognizedPdu,
    UnexpectedPdu,
    Reserved,
    UnrecognizedPduParameter,
    UnexpectedPduParameter,
    InvalidPduParameter,
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
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum Pdu {
    Unknown {
        pdu_type: u8,
        data: Vec<u8>,
    },
    AssociationRQ {
        protocol_version: u16,
        calling_ae_title: String,
        called_ae_title: String,
        application_context_name: String,
        presentation_contexts: Vec<PresentationContextProposed>,
        user_variables: Vec<UserVariableItem>,
    },
    AssociationAC {
        protocol_version: u16,
        application_context_name: String,
        presentation_contexts: Vec<PresentationContextResult>,
        user_variables: Vec<UserVariableItem>,
    },
    AssociationRJ {
        result: AssociationRJResult,
        source: AssociationRJSource,
    },
    PData {
        data: Vec<PDataValue>,
    },
    ReleaseRQ,
    ReleaseRP,
    AbortRQ {
        source: AbortRQSource,
    },
}
