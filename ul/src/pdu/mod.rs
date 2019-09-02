#[derive(Debug)]
pub struct PresentationContextProposed {
    pub id: u8,
    pub abstract_syntax: String,
    pub transfer_syntaxes: Vec<String>,
}

#[derive(Debug)]
pub struct PresentationContextResult {
    pub id: u8,
    pub reason: PresentationContextResultReason,
    pub transfer_syntax: String,
}

#[derive(Debug)]
pub enum PresentationContextResultReason {
    Acceptance = 0,
    UserRejection = 1,
    NoReason = 2,
    AbstractSyntaxNotSupported = 3,
    TransferSyntaxesNotSupported = 4,
}

#[derive(Debug)]
pub enum AssociationRJResult {
    Permanent,
    Transient,
}

#[derive(Debug)]
pub enum AssociationRJSource {
    ServiceUser(AssociationRJServiceUserReason),
    ServiceProviderASCE(AssociationRJServiceProviderASCEReason),
    ServiceProviderPresentation(AssociationRJServiceProviderPresentationReason),
}

#[derive(Debug)]
pub enum AssociationRJServiceUserReason {
    NoReasonGiven,
    ApplicationContextNameNotSupported,
    CallingAETitleNotRecognized,
    CalledAETitleNotRecognized,
    Reserved(u8),
}

#[derive(Debug)]
pub enum AssociationRJServiceProviderASCEReason {
    NoReasonGiven,
    ProtocolVersionNotSupported,
}

#[derive(Debug)]
pub enum AssociationRJServiceProviderPresentationReason {
    TemporaryCongestion,
    LocalLimitExceeded,
    Reserved(u8),
}

#[derive(Debug)]
pub struct PDataValue {
    pub presentation_context_id: u8,
    pub value_type: PDataValueType,
    pub is_last: bool,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum PDataValueType {
    Command,
    Data,
}

#[derive(Debug)]
pub enum AbortRQSource {
    ServiceUser,
    ServiceProvider(AbortRQServiceProviderReason),
    Reserved,
}

#[derive(Debug)]
pub enum AbortRQServiceProviderReason {
    ReasonNotSpecifiedUnrecognizedPDU,
    UnexpectedPDU,
    Reserved,
    UnrecognizedPDUParameter,
    UnexpectedPDUParameter,
    InvalidPDUParameter,
}

#[derive(Debug)]
pub enum PDUVariableItem {
    Unknown(u8),
    ApplicationContext(String),
    PresentationContextProposed(PresentationContextProposed),
    PresentationContextResult(PresentationContextResult),
    UserVariables(Vec<UserVariableItem>),
}

#[derive(Debug)]
pub enum UserVariableItem {
    Unknown(u8),
    MaxLength(u32),
    ImplementationClassUID(String),
    ImplementationVersionName(String),
}

#[derive(Debug)]
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
