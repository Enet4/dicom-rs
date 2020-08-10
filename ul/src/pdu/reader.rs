use crate::pdu::*;
use byteordered::byteorder::{BigEndian, ReadBytesExt};
use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
use snafu::{ensure, Backtrace, OptionExt, ResultExt, Snafu};
use std::io::{Cursor, ErrorKind, Read, Seek, SeekFrom};

pub const DEFAULT_MAX_PDU: u32 = 16_384;
pub const MINIMUM_PDU_SIZE: u32 = 4_096;
pub const MAXIMUM_PDU_SIZE: u32 = 131_072;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Invalid max PDU length {}", max_pdu_length))]
    InvalidMaxPdu {
        max_pdu_length: u32,
        backtrace: Backtrace,
    },

    #[snafu(display("No PDU available"))]
    NoPduAvailable { backtrace: Backtrace },

    #[snafu(display("Could not read PDU: {}", source))]
    ReadPdu {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Could not read PDU item: {}", source))]
    ReadPduItem {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Could not read PDU field `{}`: {}", field, source))]
    ReadPduField {
        field: &'static str,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Could not read {} reserved bytes: {}", bytes, source))]
    ReadReserved {
        bytes: u32,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Incoming pdu was too large: length {}, maximum is {}",
        pdu_length,
        max_pdu_length
    ))]
    PduTooLarge {
        pdu_length: u32,
        max_pdu_length: u32,
        backtrace: Backtrace,
    },
    #[snafu(display("PDU contained an invalid value {:?}", var_item))]
    InvalidPduVariable {
        var_item: PduVariableItem,
        backtrace: Backtrace,
    },
    #[snafu(display("Multiple transfer syntaxes were accepted"))]
    MultipleTransferSyntaxesAccepted { backtrace: Backtrace },
    #[snafu(display("Invalid reject source or reason"))]
    InvalidRejectSourceOrReason { backtrace: Backtrace },
    #[snafu(display("Invalid abort service provider"))]
    InvalidAbortSourceOrReason { backtrace: Backtrace },
    #[snafu(display("Invalid presentation context result reason"))]
    InvalidPresentationContextResultReason { backtrace: Backtrace },
    #[snafu(display("invalid transfer syntax sub-item"))]
    InvalidTransferSyntaxSubItem { backtrace: Backtrace },
    #[snafu(display("unknown presentation context sub-item"))]
    UnknownPresentationContextSubItem { backtrace: Backtrace },
    #[snafu(display("Could not decode text field `{}`: {}", field, source))]
    DecodeText {
        field: &'static str,
        source: dicom_encoding::error::TextEncodingError,
    },
    #[snafu(display("Could not encode data: {}", source))]
    Encode {
        source: dicom_encoding::error::Error,
    },
    #[snafu(display("Missing application context name"))]
    MissingApplicationContextName { backtrace: Backtrace },
    #[snafu(display("Missing abstract syntax"))]
    MissingAbstractSyntax { backtrace: Backtrace },
    #[snafu(display("Missing transfer syntax"))]
    MissingTransferSyntax { backtrace: Backtrace },
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn read_pdu<R>(reader: &mut R, max_pdu_length: u32) -> Result<Pdu>
where
    R: Read,
{
    ensure!(
        max_pdu_length >= MINIMUM_PDU_SIZE && max_pdu_length <= MAXIMUM_PDU_SIZE,
        InvalidMaxPdu { max_pdu_length }
    );

    // If we can't read 2 bytes here, that means that there is no PDU
    // available. Normally, we want to just return the UnexpectedEof error. However,
    // this method can block and wake up when stream is closed, so in this case, we
    // want to know if we had trouble even beginning to read a PDU. We still return
    // UnexpectedEof if we get after we have already began reading a PDU message.
    let mut bytes = [0; 2];
    if let Err(e) = reader.read_exact(&mut bytes) {
        ensure!(e.kind() != ErrorKind::UnexpectedEof, NoPduAvailable);
        return Err(e).context(ReadPduField { field: "type" });
    }

    let pdu_type = bytes[0];
    let pdu_length = reader
        .read_u32::<BigEndian>()
        .context(ReadPduField { field: "length" })?;

    ensure!(
        pdu_length <= max_pdu_length,
        PduTooLarge {
            pdu_length,
            max_pdu_length
        }
    );

    let bytes = read_n(reader, pdu_length as usize).context(ReadPdu)?;
    let mut cursor = Cursor::new(bytes);
    let codec = SpecificCharacterSet::Default
        .codec()
        .expect("Support for the default character set is mandatory");

    match pdu_type {
        0x01 => {
            // A-ASSOCIATE-RQ PDU Structure

            let mut application_context_name: Option<String> = None;
            let mut presentation_contexts = vec![];
            let mut user_variables = vec![];

            // 7-8 - Protocol-version - This two byte field shall use one bit to identify each
            // version of the DICOM UL protocol supported by the calling end-system. This is
            // Version 1 and shall be identified with bit 0 set. A receiver of this PDU
            // implementing only this version of the DICOM UL protocol shall only test that bit
            // 0 is set.
            let protocol_version = cursor.read_u16::<BigEndian>().context(ReadPduField {
                field: "Protocol-version",
            })?;

            // 9-10 - Reserved - This reserved field shall be sent with a value 0000H but not
            // tested to this value when received.
            cursor
                .read_u16::<BigEndian>()
                .context(ReadReserved { bytes: 2_u32 })?;

            // 11-26 - Called-AE-title - Destination DICOM Application Name. It shall be encoded
            // as 16 characters as defined by the ISO 646:1990-Basic G0 Set with leading and
            // trailing spaces (20H) being non-significant. The value made of 16 spaces (20H)
            // meaning "no Application Name specified" shall not be used. For a complete
            // description of the use of this field, see Section 7.1.1.4.
            let mut ae_bytes = [0; 16];
            cursor.read_exact(&mut ae_bytes).context(ReadPduField {
                field: "Called-AE-title",
            })?;
            let called_ae_title = codec
                .decode(&ae_bytes)
                .context(DecodeText {
                    field: "Called-AE-title",
                })?
                .trim()
                .to_string();

            // 27-42 - Calling-AE-title - Source DICOM Application Name. It shall be encoded as
            // 16 characters as defined by the ISO 646:1990-Basic G0 Set with leading and
            // trailing spaces (20H) being non-significant. The value made of 16 spaces (20H)
            // meaning "no Application Name specified" shall not be used. For a complete
            // description of the use of this field, see Section 7.1.1.3.
            let mut ae_bytes = [0; 16];
            cursor.read_exact(&mut ae_bytes).context(ReadPduField {
                field: "Calling-AE-title",
            })?;
            let calling_ae_title = codec
                .decode(&ae_bytes)
                .context(DecodeText {
                    field: "Calling-AE-title",
                })?
                .trim()
                .to_string();

            // 43-74 - Reserved - This reserved field shall be sent with a value 00H for all
            // bytes but not tested to this value when received
            cursor
                .seek(SeekFrom::Current(32))
                .context(ReadReserved { bytes: 32_u32 })?;

            // 75-xxx - Variable items - This variable field shall contain the following items:
            // one Application Context Item, one or more Presentation Context Items and one User
            // Information Item. For a complete description of the use of these items see
            // Section 7.1.1.2, Section 7.1.1.13, and Section 7.1.1.6.
            while cursor.position() < cursor.get_ref().len() as u64 {
                match read_pdu_variable(&mut cursor, &codec)? {
                    PduVariableItem::ApplicationContext(val) => {
                        application_context_name = Some(val);
                    }
                    PduVariableItem::PresentationContextProposed(val) => {
                        presentation_contexts.push(val);
                    }
                    PduVariableItem::UserVariables(val) => {
                        user_variables = val;
                    }
                    var_item => {
                        return InvalidPduVariable { var_item }.fail();
                    }
                }
            }

            Ok(Pdu::AssociationRQ {
                protocol_version,
                application_context_name: application_context_name
                    .context(MissingApplicationContextName)?,
                called_ae_title,
                calling_ae_title,
                presentation_contexts,
                user_variables,
            })
        }
        0x02 => {
            // A-ASSOCIATE-AC PDU Structure

            let mut application_context_name: Option<String> = None;
            let mut presentation_contexts = vec![];
            let mut user_variables = vec![];

            // 7-8 - Protocol-version - This two byte field shall use one bit to identify each
            // version of the DICOM UL protocol supported by the calling end-system. This is
            // Version 1 and shall be identified with bit 0 set. A receiver of this PDU
            // implementing only this version of the DICOM UL protocol shall only test that bit
            // 0 is set.
            let protocol_version = cursor.read_u16::<BigEndian>().context(ReadPduField {
                field: "Protocol-version",
            })?;

            // 9-10 - Reserved - This reserved field shall be sent with a value 0000H but not
            // tested to this value when received.
            cursor
                .read_u16::<BigEndian>()
                .context(ReadReserved { bytes: 2_u32 })?;

            // 11-26 - Reserved - This reserved field shall be sent with a value identical to
            // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
            // shall not be tested when received.
            // 27-42 - Reserved - This reserved field shall be sent with a value identical to
            // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
            // shall not be tested when received.
            // 43-74 - Reserved - This reserved field shall be sent with a value identical to
            // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
            // shall not be tested when received.
            cursor
                .seek(SeekFrom::Current(16 + 16 + 32))
                .context(ReadReserved { bytes: 64_u32 })?;

            // 75-xxx - Variable items - This variable field shall contain the following items:
            // one Application Context Item, one or more Presentation Context Item(s) and one
            // User Information Item. For a complete description of these items see Section
            // 7.1.1.2, Section 7.1.1.14, and Section 7.1.1.6.
            while cursor.position() < cursor.get_ref().len() as u64 {
                match read_pdu_variable(&mut cursor, &codec)? {
                    PduVariableItem::ApplicationContext(val) => {
                        application_context_name = Some(val);
                    }
                    PduVariableItem::PresentationContextResult(val) => {
                        presentation_contexts.push(val);
                    }
                    PduVariableItem::UserVariables(val) => {
                        user_variables = val;
                    }
                    var_item => {
                        return InvalidPduVariable { var_item }.fail();
                    }
                }
            }

            Ok(Pdu::AssociationAC {
                protocol_version,
                application_context_name: application_context_name
                    .context(MissingApplicationContextName)?,
                presentation_contexts,
                user_variables,
            })
        }
        0x03 => {
            // A-ASSOCIATE-RJ PDU Structure

            // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

            // 8 - Result - This Result field shall contain an integer value encoded as an unsigned
            // binary number. One of the following values shall be used:
            //   1 - rejected-permanent
            //   2 - rejected-transient
            let result = AssociationRJResult::from(
                cursor.read_u8().context(ReadPduField { field: "Result" })?,
            )
            .context(InvalidRejectSourceOrReason)?;

            // 9 - Source - This Source field shall contain an integer value encoded as an unsigned
            // binary number. One of the following values shall be used:   1 - DICOM UL
            // service-user   2 - DICOM UL service-provider (ACSE related function)
            //   3 - DICOM UL service-provider (Presentation related function)
            // 10 - Reason/Diag. - This field shall contain an integer value encoded as an unsigned
            // binary number.   If the Source field has the value (1) "DICOM UL
            // service-user", it shall take one of the following:
            //     1 - no-reason-given
            //     2 - application-context-name-not-supported
            //     3 - calling-AE-title-not-recognized
            //     4-6 - reserved
            //     7 - called-AE-title-not-recognized
            //     8-10 - reserved
            //   If the Source field has the value (2) "DICOM UL service provided (ACSE related
            // function)", it shall take one of the following:     1 - no-reason-given
            //     2 - protocol-version-not-supported
            //   If the Source field has the value (3) "DICOM UL service provided (Presentation
            // related function)", it shall take one of the following:     0 - reserved
            //     1 - temporary-congestio
            //     2 - local-limit-exceeded
            //     3-7 - reserved
            let source = AssociationRJSource::from(
                cursor.read_u8().context(ReadPduField { field: "Source" })?,
                cursor.read_u8().context(ReadPduField {
                    field: "Reason/Diag.",
                })?,
            )
            .context(InvalidRejectSourceOrReason)?;

            Ok(Pdu::AssociationRJ { result, source })
        }
        0x04 => {
            // P-DATA-TF PDU Structure

            // 7-xxx - Presentation-data-value Item(s) - This variable data field shall contain one
            // or more Presentation-data-value Items(s). For a complete description of the use of
            // this field see Section 9.3.5.1
            let mut values = vec![];
            while cursor.position() < cursor.get_ref().len() as u64 {
                // Presentation Data Value Item Structure

                // 1-4 - Item-length - This Item-length shall be the number of bytes from the first
                // byte of the following field to the last byte of the Presentation-data-value
                // field. It shall be encoded as an unsigned binary number.
                let item_length = cursor.read_u32::<BigEndian>().context(ReadPduField {
                    field: "Item-Length",
                })?;

                // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd
                // integers between 1 and 255, encoded as an unsigned binary number. For a complete
                // description of the use of this field see Section 7.1.1.13.
                let presentation_context_id = cursor.read_u8().context(ReadPduField {
                    field: "Presentation-context-ID",
                })?;

                // 6-xxx - Presentation-data-value - This Presentation-data-value field shall
                // contain DICOM message information (command and/or data set) with a message
                // control header. For a complete description of the use of this field see Annex E.

                // The Message Control Header shall be made of one byte with the least significant
                // bit (bit 0) taking one of the following values: If bit 0 is set
                // to 1, the following fragment shall contain Message Command information.
                // If bit 0 is set to 0, the following fragment shall contain Message Data Set
                // information. The next least significant bit (bit 1) shall be
                // defined by the following rules: If bit 1 is set to 1, the
                // following fragment shall contain the last fragment of a Message Data Set or of a
                // Message Command. If bit 1 is set to 0, the following fragment
                // does not contain the last fragment of a Message Data Set or of a Message Command.
                let value_type;
                let is_last;
                let header = cursor.read_u8().context(ReadPduField {
                    field: "Message Control Header",
                })?;

                if header & 0x01 > 0 {
                    value_type = PDataValueType::Command;
                } else {
                    value_type = PDataValueType::Data;
                }
                if header & 0x02 > 0 {
                    is_last = true;
                } else {
                    is_last = false;
                }

                let data =
                    read_n(&mut cursor, (item_length - 2) as usize).context(ReadPduField {
                        field: "Presentation-data-value",
                    })?;

                values.push(PDataValue {
                    presentation_context_id,
                    value_type,
                    is_last,
                    data,
                })
            }

            Ok(Pdu::PData { data: values })
        }
        0x05 => {
            // A-RELEASE-RQ PDU Structure

            // 7-10 - Reserved - This reserved field shall be sent with a value 00000000H but not
            // tested to this value when received.
            cursor
                .seek(SeekFrom::Current(4))
                .context(ReadPduField { field: "Reserved" })?;

            Ok(Pdu::ReleaseRQ)
        }
        0x06 => {
            // A-RELEASE-RP PDU Structure

            // 7-10 - Reserved - This reserved field shall be sent with a value 00000000H but not
            // tested to this value when received.
            cursor
                .seek(SeekFrom::Current(4))
                .context(ReadPduField { field: "Reserved" })?;

            Ok(Pdu::ReleaseRP)
        }
        0x07 => {
            // A-ABORT PDU Structure

            // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor
                .read_u8()
                .context(ReadPduField { field: "Reserved" })?;

            // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor
                .read_u8()
                .context(ReadPduField { field: "Reserved" })?;

            // 9 - Source - This Source field shall contain an integer value encoded as an unsigned
            // binary number. One of the following values shall be used:
            // - 0 - DICOM UL service-user (initiated abort)
            // - 1 - reserved
            // - 2 - DICOM UL service-provider (initiated abort)
            // 10 - Reason/Diag - This field shall contain an integer value encoded as an unsigned
            // binary number. If the Source field has the value (2) "DICOM UL
            // service-provider", it shall take one of the following:
            // - 0 - reason-not-specified1 - unrecognized-PDU
            // - 2 - unexpected-PDU
            // - 3 - reserved
            // - 4 - unrecognized-PDU parameter
            // - 5 - unexpected-PDU parameter
            // - 6 - invalid-PDU-parameter value
            let source = AbortRQSource::from(
                cursor.read_u8().context(ReadPduField { field: "Source" })?,
                cursor.read_u8().context(ReadPduField {
                    field: "Reason/Diag",
                })?,
            )
            .context(InvalidAbortSourceOrReason)?;

            Ok(Pdu::AbortRQ { source })
        }
        _ => {
            let data = read_n(&mut cursor, pdu_length as usize)
                .context(ReadPduField { field: "Unknown" })?;
            Ok(Pdu::Unknown { pdu_type, data })
        }
    }
}

fn read_n<R>(reader: &mut R, bytes_to_read: usize) -> std::io::Result<Vec<u8>>
where
    R: Read,
{
    let mut result = Vec::new();
    reader.take(bytes_to_read as u64).read_to_end(&mut result)?;
    Ok(result)
}

fn read_pdu_variable<R>(reader: &mut R, codec: &dyn TextCodec) -> Result<PduVariableItem>
where
    R: Read,
{
    // 1 - Item-type - XXH
    let item_type = reader
        .read_u8()
        .context(ReadPduField { field: "Item-type" })?;

    // 2 - Reserved
    reader.read_u8().context(ReadReserved { bytes: 1_u32 })?;

    // 3-4 - Item-length
    let item_length = reader.read_u16::<BigEndian>().context(ReadPduField {
        field: "Item-length",
    })?;

    let bytes = read_n(reader, item_length as usize).context(ReadPduItem)?;
    let mut cursor = Cursor::new(bytes);

    match item_type {
        0x10 => {
            // Application Context Item Structure

            // 5-xxx - Application-context-name - A valid Application-context-name shall be encoded
            // as defined in Annex F. For a description of the use of this field see Section
            // 7.1.1.2. Application-context-names are structured as UIDs as defined in PS3.5 (see
            // Annex A for an overview of this concept). DICOM Application-context-names are
            // registered in PS3.7.
            let val = codec.decode(&cursor.into_inner()).context(DecodeText {
                field: "Application-context-name",
            })?;
            Ok(PduVariableItem::ApplicationContext(val))
        }
        0x20 => {
            // Presentation Context Item Structure (proposed)

            let mut abstract_syntax: Option<String> = None;
            let mut transfer_syntaxes = vec![];

            // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd integers
            // between 1 and 255, encoded as an unsigned binary number. For a complete description
            // of the use of this field see Section 7.1.1.13.
            let presentation_context_id = cursor.read_u8().context(ReadPduField {
                field: "Presentation-context-ID",
            })?;

            // 6 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

            // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

            // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

            // 9-xxx - Abstract/Transfer Syntax Sub-Items - This variable field shall contain the
            // following sub-items: one Abstract Syntax and one or more Transfer Syntax(es). For a
            // complete description of the use and encoding of these sub-items see Section 9.3.2.2.1
            // and Section 9.3.2.2.2.
            while cursor.position() < cursor.get_ref().len() as u64 {
                // 1 - Item-type - XXH
                let item_type = cursor
                    .read_u8()
                    .context(ReadPduField { field: "Item-type" })?;

                // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested
                // to this value when received.
                cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

                // 3-4 - Item-length
                let item_length = cursor.read_u16::<BigEndian>().context(ReadPduField {
                    field: "Item-length",
                })?;

                match item_type {
                    0x30 => {
                        // Abstract Syntax Sub-Item Structure

                        // 5-xxx - Abstract-syntax-name - This variable field shall contain the
                        // Abstract-syntax-name related to the proposed presentation context. A
                        // valid Abstract-syntax-name shall be encoded as defined in Annex F. For a
                        // description of the use of this field see Section 7.1.1.13.
                        // Abstract-syntax-names are structured as UIDs as defined in PS3.5 (see
                        // Annex B for an overview of this concept). DICOM Abstract-syntax-names are
                        // registered in PS3.4.
                        abstract_syntax = Some(
                            codec
                                .decode(&read_n(&mut cursor, item_length as usize).context(
                                    ReadPduField {
                                        field: "Abstract-syntax-name",
                                    },
                                )?)
                                .context(DecodeText {
                                    field: "Abstract-syntax-name",
                                })?
                                .trim()
                                .to_string(),
                        );
                    }
                    0x40 => {
                        // Transfer Syntax Sub-Item Structure

                        // 5-xxx - Transfer-syntax-name(s) - This variable field shall contain the
                        // Transfer-syntax-name proposed for this presentation context. A valid
                        // Transfer-syntax-name shall be encoded as defined in Annex F. For a
                        // description of the use of this field see Section 7.1.1.13.
                        // Transfer-syntax-names are structured as UIDs as defined in PS3.5 (see
                        // Annex B for an overview of this concept). DICOM Transfer-syntax-names are
                        // registered in PS3.5.
                        transfer_syntaxes.push(
                            codec
                                .decode(&read_n(&mut cursor, item_length as usize).context(
                                    ReadPduField {
                                        field: "Transfer-syntax-name",
                                    },
                                )?)
                                .context(DecodeText {
                                    field: "Transfer-syntax-name",
                                })?
                                .trim()
                                .to_string(),
                        );
                    }
                    _ => {
                        return UnknownPresentationContextSubItem.fail();
                    }
                }
            }

            Ok(PduVariableItem::PresentationContextProposed(
                PresentationContextProposed {
                    id: presentation_context_id,
                    abstract_syntax: abstract_syntax.context(MissingAbstractSyntax)?,
                    transfer_syntaxes,
                },
            ))
        }
        0x21 => {
            // Presentation Context Item Structure (result)

            let mut transfer_syntax: Option<String> = None;

            // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd integers
            // between 1 and 255, encoded as an unsigned binary number. For a complete description
            // of the use of this field see Section 7.1.1.13.
            let presentation_context_id = cursor.read_u8().context(ReadPduField {
                field: "Presentation-context-ID",
            })?;

            // 6 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

            // 7 - Result/Reason - This Result/Reason field shall contain an integer value encoded
            // as an unsigned binary number. One of the following values shall be used:
            //   0 - acceptance
            //   1 - user-rejection
            //   2 - no-reason (provider rejection)
            //   3 - abstract-syntax-not-supported (provider rejection)
            //   4 - transfer-syntaxes-not-supported (provider rejection)
            let reason =
                PresentationContextResultReason::from(cursor.read_u8().context(ReadPduField {
                    field: "Result/Reason",
                })?)
                .context(InvalidPresentationContextResultReason)?;

            // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

            // 9-xxx - Transfer syntax sub-item - This variable field shall contain one Transfer
            // Syntax Sub-Item. When the Result/Reason field has a value other than acceptance (0),
            // this field shall not be significant and its value shall not be tested when received.
            // For a complete description of the use and encoding of this item see Section
            // 9.3.3.2.1.
            while cursor.position() < cursor.get_ref().len() as u64 {
                // 1 - Item-type - XXH
                let item_type = cursor
                    .read_u8()
                    .context(ReadPduField { field: "Item-type" })?;

                // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested
                // to this value when received.
                cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

                // 3-4 - Item-length
                let item_length = cursor.read_u16::<BigEndian>().context(ReadPduField {
                    field: "Item-length",
                })?;

                match item_type {
                    0x40 => {
                        // Transfer Syntax Sub-Item Structure

                        // 5-xxx - Transfer-syntax-name(s) - This variable field shall contain the
                        // Transfer-syntax-name proposed for this presentation context. A valid
                        // Transfer-syntax-name shall be encoded as defined in Annex F. For a
                        // description of the use of this field see Section 7.1.1.13.
                        // Transfer-syntax-names are structured as UIDs as defined in PS3.5 (see
                        // Annex B for an overview of this concept). DICOM Transfer-syntax-names are
                        // registered in PS3.5.
                        match transfer_syntax {
                            Some(_) => {
                                // Multiple transfer syntax values cannot be proposed.
                                return MultipleTransferSyntaxesAccepted.fail();
                            }
                            None => {
                                transfer_syntax = Some(
                                    codec
                                        .decode(
                                            &read_n(&mut cursor, item_length as usize).context(
                                                ReadPduField {
                                                    field: "Transfer-syntax-name",
                                                },
                                            )?,
                                        )
                                        .context(DecodeText {
                                            field: "Transfer-syntax-name",
                                        })?
                                        .trim()
                                        .to_string(),
                                );
                            }
                        }
                    }
                    _ => {
                        return InvalidTransferSyntaxSubItem.fail();
                    }
                }
            }

            Ok(PduVariableItem::PresentationContextResult(
                PresentationContextResult {
                    id: presentation_context_id,
                    reason,
                    transfer_syntax: transfer_syntax.context(MissingTransferSyntax)?,
                },
            ))
        }
        0x50 => {
            // User Information Item Structure

            let mut user_variables = vec![];

            // 5-xxx - User-data - This variable field shall contain User-data sub-items as defined
            // by the DICOM Application Entity. The structure and content of these sub-items is
            // defined in Annex D.
            while cursor.position() < cursor.get_ref().len() as u64 {
                // 1 - Item-type - XXH
                let item_type = cursor
                    .read_u8()
                    .context(ReadPduField { field: "Item-type" })?;

                // 2 - Reserved
                cursor.read_u8().context(ReadReserved { bytes: 1_u32 })?;

                // 3-4 - Item-length
                let item_length = cursor.read_u16::<BigEndian>().context(ReadPduField {
                    field: "Item-length",
                })?;

                match item_type {
                    0x51 => {
                        // Maximum Length Sub-Item Structure

                        // 5-8 - Maximum-length-received - This parameter allows the
                        // association-requestor to restrict the maximum length of the variable
                        // field of the P-DATA-TF PDUs sent by the acceptor on the association once
                        // established. This length value is indicated as a number of bytes encoded
                        // as an unsigned binary number. The value of (0) indicates that no maximum
                        // length is specified. This maximum length value shall never be exceeded by
                        // the PDU length values used in the PDU-length field of the P-DATA-TF PDUs
                        // received by the association-requestor. Otherwise, it shall be a protocol
                        // error.
                        user_variables.push(UserVariableItem::MaxLength(
                            cursor.read_u32::<BigEndian>().context(ReadPduField {
                                field: "Maximum-length-received",
                            })?,
                        ));
                    }
                    0x52 => {
                        // Implementation Class UID Sub-Item Structure

                        // 5 - xxx - Implementation-class-uid - This variable field shall contain
                        // the Implementation-class-uid of the Association-acceptor as defined in
                        // Section D.3.3.2. The Implementation-class-uid field is structured as a
                        // UID as defined in PS3.5.
                        let implementation_class_uid = codec
                            .decode(&read_n(&mut cursor, item_length as usize).context(
                                ReadPduField {
                                    field: "Implementation-class-uid",
                                },
                            )?)
                            .context(DecodeText {
                                field: "Implementation-class-uid",
                            })?
                            .trim()
                            .to_string();
                        user_variables.push(UserVariableItem::ImplementationClassUID(
                            implementation_class_uid,
                        ));
                    }
                    0x55 => {
                        // Implementation Version Name Structure

                        // 5 - xxx - Implementation-version-name - This variable field shall contain
                        // the Implementation-version-name of the Association-acceptor as defined in
                        // Section D.3.3.2. It shall be encoded as a string of 1 to 16 ISO 646:1990
                        // (basic G0 set) characters.
                        let implementation_version_name = codec
                            .decode(&read_n(&mut cursor, item_length as usize).context(
                                ReadPduField {
                                    field: "Implementation-version-name",
                                },
                            )?)
                            .context(DecodeText {
                                field: "Implementation-version-name",
                            })?
                            .trim()
                            .to_string();
                        user_variables.push(UserVariableItem::ImplementationVersionName(
                            implementation_version_name,
                        ));
                    }
                    _ => {
                        user_variables.push(UserVariableItem::Unknown(
                            item_type,
                            read_n(&mut cursor, item_length as usize)
                                .context(ReadPduField { field: "Unknown" })?,
                        ));
                    }
                }
            }

            Ok(PduVariableItem::UserVariables(user_variables))
        }
        _ => Ok(PduVariableItem::Unknown(item_type)),
    }
}
