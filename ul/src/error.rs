use quick_error::quick_error;
use std::error::Error as BaseError;

/// Type alias for a result from this crate.
pub type Result<T> = std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: std::io::Error) {
            from()
            display(self_) -> ("io error: {}", err)
        }
        NoPduAvailable {
            description("no pdu was available")
            display(self_) -> ("{}", self_.description())
        }
        InvalidMaxPdu {
            description("invalid max pdu")
            display(self_) -> ("{}", self_.description())
        }
        PduTooLarge {
            description("the incoming pdu was too large")
            display(self_) -> ("{}", self_.description())
        }
        InvalidPduVariable {
            description("the pdu contained an invalid value")
            display(self_) -> ("{}", self_.description())
        }
        MultipleTransferSyntaxesAccepted {
            description("multiple transfer syntaxes were accepted")
            display(self_) -> ("{}", self_.description())
        }
        InvalidRejectSourceOrReason {
            description("the reject source or reason was invalid")
            display(self_) -> ("{}", self_.description())
        }
        InvalidAbortSourceOrReason {
            description("the abort service provider reason was invalid")
            display(self_) -> ("{}", self_.description())
        }
        InvalidPresentationContextResultReason {
            description("the presentation context result reason was invalid")
            display(self_) -> ("{}", self_.description())
        }
        InvalidTransferSyntaxSubItem {
            description("invalid transfer syntax sub-item")
            display(self_) -> ("{}", self_.description())
        }
        UnknownPresentationContextSubItem {
            description("unknown presentation context sub-item")
            display(self_) -> ("{}", self_.description())
        }
        EncodingError(err: dicom_encoding::error::Error) {
            from()
            description("encoding error")
            display(self_) -> ("{} {}", err, self_.description())
        }
        MissingApplicationContextName {
            description("missing application context name")
            display(self_) -> ("{}", self_.description())
        }
        MissingAbstractSyntax {
            description("missing abstract syntax")
            display(self_) -> ("{}", self_.description())
        }
        MissingTransferSyntax {
            description("missing transfer syntax")
            display(self_) -> ("{}", self_.description())
        }
    }
}
