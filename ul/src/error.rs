use quick_error::quick_error;

/// Type alias for a result from this crate.
pub type Result<T> = std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: std::io::Error) {
            from()
            display("io error: {}", err)
        }
        NoPduAvailable {
            display("no pdu was available")
        }
        InvalidMaxPdu {
            display("invalid max pdu")
        }
        PduTooLarge {
            display("the incoming pdu was too large")
        }
        InvalidPduVariable {
            display("the pdu contained an invalid value")
        }
        MultipleTransferSyntaxesAccepted {
            display("multiple transfer syntaxes were accepted")
        }
        InvalidRejectSourceOrReason {
            display("the reject source or reason was invalid")
        }
        InvalidAbortSourceOrReason {
            display("the abort service provider reason was invalid")
        }
        InvalidPresentationContextResultReason {
            display("the presentation context result reason was invalid")
        }
        InvalidTransferSyntaxSubItem {
            display("invalid transfer syntax sub-item")
        }
        UnknownPresentationContextSubItem {
            display("unknown presentation context sub-item")
        }
        EncodingError(err: dicom_encoding::error::Error) {
            from()
            display("{} encoding error", err)
        }
        MissingApplicationContextName {
            display("missing application context name")
        }
        MissingAbstractSyntax {
            display("missing abstract syntax")
        }
        MissingTransferSyntax {
            display("missing transfer syntax")
        }
    }
}
