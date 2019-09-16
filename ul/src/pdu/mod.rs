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

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJResult {
    Permanent,
    Transient,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AssociationRJSource {
    ServiceUser(AssociationRJServiceUserReason),
    ServiceProviderASCE(AssociationRJServiceProviderASCEReason),
    ServiceProviderPresentation(AssociationRJServiceProviderPresentationReason),
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

#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Debug)]
pub enum AbortRQServiceProviderReason {
    ReasonNotSpecifiedUnrecognizedPDU,
    UnexpectedPDU,
    Reserved,
    UnrecognizedPDUParameter,
    UnexpectedPDUParameter,
    InvalidPDUParameter,
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
pub enum PDU {
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

pub mod reader;
pub mod writer;

#[cfg(test)]
mod test;
