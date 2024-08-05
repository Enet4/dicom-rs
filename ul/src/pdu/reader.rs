/// PDU reader module
use crate::pdu::*;
use dicom_encoding::text::{DefaultCharacterSetCodec, TextCodec};
use snafu::{ensure, OptionExt, ResultExt};
use tracing::warn;
use bytes::Buf;

pub type Result<T> = std::result::Result<T, ReadError>;

pub fn read_pdu(mut buf: impl Buf, max_pdu_length: u32, strict: bool) -> Result<Option<Pdu>>
{
    ensure!(
        (MINIMUM_PDU_SIZE..=MAXIMUM_PDU_SIZE).contains(&max_pdu_length),
        InvalidMaxPduSnafu { max_pdu_length }
    );

    // If we can't read 2 bytes here, that means that there is no PDU
    // available. Normally, we want to just return the UnexpectedEof error. However,
    // this method can block and wake up when stream is closed, so in this case, we
    // want to know if we had trouble even beginning to read a PDU. We still return
    // UnexpectedEof if we get after we have already began reading a PDU message.
    if buf.remaining() < 2 {
        return Ok(None);
    }
    let bytes = buf.copy_to_bytes(2);
    let pdu_type = bytes[0];
    if buf.remaining() < 4 {
        return Ok(None);
    }
    let pdu_length = buf.get_u32();

    // Check max_pdu_length
    if strict {
        ensure!(
            pdu_length <= max_pdu_length,
            PduTooLargeSnafu {
                pdu_length,
                max_pdu_length
            }
        );
    } else if pdu_length > max_pdu_length {
        ensure!(
            pdu_length <= MAXIMUM_PDU_SIZE,
            PduTooLargeSnafu {
                pdu_length,
                max_pdu_length: MAXIMUM_PDU_SIZE
            }
        );
        tracing::warn!(
            "Incoming pdu was too large: length {}, maximum is {}",
            pdu_length,
            max_pdu_length
        );
    }
    if buf.remaining() < pdu_length as usize { return Ok(None); }
    let mut bytes = buf.copy_to_bytes(pdu_length as usize);
    let codec = DefaultCharacterSetCodec;

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
            if bytes.remaining() < 2 { return Ok(None) }
            let protocol_version = bytes.get_u16();

            // 9-10 - Reserved - This reserved field shall be sent with a value 0000H but not
            // tested to this value when received.
            if bytes.remaining() < 2 { return Ok(None) }
            bytes.get_u16();

            // 11-26 - Called-AE-title - Destination DICOM Application Name. It shall be encoded
            // as 16 characters as defined by the ISO 646:1990-Basic G0 Set with leading and
            // trailing spaces (20H) being non-significant. The value made of 16 spaces (20H)
            // meaning "no Application Name specified" shall not be used. For a complete
            // description of the use of this field, see Section 7.1.1.4.
            let ae_bytes = bytes.copy_to_bytes(16);
            let called_ae_title = codec
                .decode(ae_bytes.as_ref())
                .context(DecodeTextSnafu {
                    field: "Called-AE-title",
                })?
                .trim()
                .to_string();

            // 27-42 - Calling-AE-title - Source DICOM Application Name. It shall be encoded as
            // 16 characters as defined by the ISO 646:1990-Basic G0 Set with leading and
            // trailing spaces (20H) being non-significant. The value made of 16 spaces (20H)
            // meaning "no Application Name specified" shall not be used. For a complete
            // description of the use of this field, see Section 7.1.1.3.
            let ae_bytes = bytes.copy_to_bytes(16);
            let calling_ae_title = codec
                .decode(ae_bytes.as_ref())
                .context(DecodeTextSnafu {
                    field: "Calling-AE-title",
                })?
                .trim()
                .to_string();

            // 43-74 - Reserved - This reserved field shall be sent with a value 00H for all
            // bytes but not tested to this value when received
            bytes.advance(32);

            // 75-xxx - Variable items - This variable field shall contain the following items:
            // one Application Context Item, one or more Presentation Context Items and one User
            // Information Item. For a complete description of the use of these items see
            // Section 7.1.1.2, Section 7.1.1.13, and Section 7.1.1.6.
            while bytes.has_remaining() {
                match read_pdu_variable(&mut bytes, &codec)? {
                    Some(PduVariableItem::ApplicationContext(val)) => {
                        application_context_name = Some(val);
                    }
                    Some(PduVariableItem::PresentationContextProposed(val)) => {
                        presentation_contexts.push(val);
                    }
                    Some(PduVariableItem::UserVariables(val)) => {
                        user_variables = val;
                    }
                    Some(var_item) => {
                        return InvalidPduVariableSnafu { var_item }.fail();
                    },
                    None => {
                        println!("PDU variable none");
                        return Ok(None)
                    }
                }
            }

            Ok(Some(Pdu::AssociationRQ(AssociationRQ {
                protocol_version,
                application_context_name: application_context_name
                    .context(MissingApplicationContextNameSnafu)?,
                called_ae_title,
                calling_ae_title,
                presentation_contexts,
                user_variables,
            })))
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
            if bytes.remaining() < 2 { return Ok(None) }
            let protocol_version = bytes.get_u16();

            // 9-10 - Reserved - This reserved field shall be sent with a value 0000H but not
            // tested to this value when received.
            if bytes.remaining() < 2 { return Ok(None) }
            bytes.get_u16();

            // 11-26 - Reserved - This reserved field shall be sent with a value identical to
            // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
            // shall not be tested when received.
            let ae_bytes = bytes.copy_to_bytes(16);
            let called_ae_title = codec
                .decode(&ae_bytes)
                .context(DecodeTextSnafu {
                    field: "Called-AE-title",
                })?
                .trim()
                .to_string();

            // 27-42 - Reserved - This reserved field shall be sent with a value identical to
            // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
            // shall not be tested when received.
            let ae_bytes = bytes.copy_to_bytes(16);
            let calling_ae_title = codec
                .decode(&ae_bytes)
                .context(DecodeTextSnafu {
                    field: "Calling-AE-title",
                })?
                .trim()
                .to_string();

            // 43-74 - Reserved - This reserved field shall be sent with a value identical to
            // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
            // shall not be tested when received.
            bytes.advance(32);

            // 75-xxx - Variable items - This variable field shall contain the following items:
            // one Application Context Item, one or more Presentation Context Item(s) and one
            // User Information Item. For a complete description of these items see Section
            // 7.1.1.2, Section 7.1.1.14, and Section 7.1.1.6.
            while bytes.has_remaining() {
                match read_pdu_variable(bytes.clone(), &codec)? {
                    Some(PduVariableItem::ApplicationContext(val)) => {
                        application_context_name = Some(val);
                    }
                    Some(PduVariableItem::PresentationContextResult(val)) => {
                        presentation_contexts.push(val);
                    }
                    Some(PduVariableItem::UserVariables(val)) => {
                        user_variables = val;
                    }
                    Some(var_item) => {
                        return InvalidPduVariableSnafu { var_item }.fail();
                    },
                    None => return Ok(None)
                }
            }

            Ok(Some(Pdu::AssociationAC(AssociationAC {
                protocol_version,
                application_context_name: application_context_name
                    .context(MissingApplicationContextNameSnafu)?,
                called_ae_title,
                calling_ae_title,
                presentation_contexts,
                user_variables,
            })))
        }
        0x03 => {
            // A-ASSOCIATE-RJ PDU Structure

            // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            if bytes.remaining() < 1 { return Ok(None) }
            bytes.get_u8();

            // 8 - Result - This Result field shall contain an integer value encoded as an unsigned
            // binary number. One of the following values shall be used:
            //   1 - rejected-permanent
            //   2 - rejected-transient
            if bytes.remaining() < 1 { return Ok(None) }
            let result = AssociationRJResult::from(bytes.get_u8())
                .context(InvalidRejectSourceOrReasonSnafu)?;

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
            if bytes.remaining() < 2 { return Ok(None) }
            let source = AssociationRJSource::from(
                bytes.get_u8(),
                bytes.get_u8()
            )
            .context(InvalidRejectSourceOrReasonSnafu)?;

            Ok(Some(Pdu::AssociationRJ(AssociationRJ { result, source })))
        }
        0x04 => {
            // P-DATA-TF PDU Structure

            // 7-xxx - Presentation-data-value Item(s) - This variable data field shall contain one
            // or more Presentation-data-value Items(s). For a complete description of the use of
            // this field see Section 9.3.5.1
            let mut values = vec![];
            while bytes.has_remaining() {
                // Presentation Data Value Item Structure

                // 1-4 - Item-length - This Item-length shall be the number of bytes from the first
                // byte of the following field to the last byte of the Presentation-data-value
                // field. It shall be encoded as an unsigned binary number.
                if bytes.remaining() < 4 { return Ok(None) }
                let item_length = bytes.get_u32();

                ensure!(
                    item_length >= 2,
                    InvalidItemLengthSnafu {
                        length: item_length
                    }
                );

                // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd
                // integers between 1 and 255, encoded as an unsigned binary number. For a complete
                // description of the use of this field see Section 7.1.1.13.
                if bytes.remaining() < 1 { return Ok(None) }
                let presentation_context_id = bytes.get_u8();

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
                if bytes.remaining() < 1 { return Ok(None) }
                let header = bytes.get_u8();

                let value_type = if header & 0x01 > 0 {
                    PDataValueType::Command
                } else {
                    PDataValueType::Data
                };
                let is_last = (header & 0x02) > 0;
                if bytes.remaining() < (item_length - 2) as usize { return Ok(None) }
                values.push(PDataValue {
                    presentation_context_id,
                    value_type,
                    is_last,
                    data: bytes.copy_to_bytes((item_length - 2) as usize).to_vec(),
                });
            }

            Ok(Some(Pdu::PData { data: values }))
        }
        0x05 => {
            // A-RELEASE-RQ PDU Structure

            // 7-10 - Reserved - This reserved field shall be sent with a value 00000000H but not
            // tested to this value when received.
            bytes.advance(4);

            Ok(Some(Pdu::ReleaseRQ))
        }
        0x06 => {
            // A-RELEASE-RP PDU Structure

            // 7-10 - Reserved - This reserved field shall be sent with a value 00000000H but not
            // tested to this value when received.
            if bytes.remaining() < 4 { return Ok(None) }
            bytes.advance(4);

            Ok(Some(Pdu::ReleaseRP))
        }
        0x07 => {
            // A-ABORT PDU Structure

            // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            if bytes.remaining() < 2 { return Ok(None) }
            let _ = bytes.copy_to_bytes(2);

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
            if bytes.remaining() < 2 { return Ok(None) }
            let source = AbortRQSource::from(
                bytes.get_u8(),
                bytes.get_u8()
            )
            .context(InvalidAbortSourceOrReasonSnafu)?;

            Ok(Some(Pdu::AbortRQ { source }))
        }
        _ => {
            if bytes.remaining() < pdu_length as usize {return Ok(None);}   
            Ok(Some(Pdu::Unknown { 
                pdu_type, 
                data: bytes.copy_to_bytes(pdu_length as usize).to_vec() 
            }))
        }
    }
}

fn read_pdu_variable(mut buf: impl Buf, codec: &dyn TextCodec) -> Result<Option<PduVariableItem>>
{
    // 1 - Item-type - XXH
    if buf.remaining() < 1 { return Ok(None); }
    let item_type = buf.get_u8();

    // 2 - Reserved
    if buf.remaining() < 1 { return Ok(None); }
    buf.get_u8();

    // 3-4 - Item-length
    if buf.remaining() < 2 { return Ok(None); }
    let item_length = buf.get_u16();

    if buf.remaining() < item_length as usize { return Ok(None); }
    let mut bytes = buf.copy_to_bytes(item_length as usize);
    match item_type {
        0x10 => {
            // Application Context Item Structure

            // 5-xxx - Application-context-name - A valid Application-context-name shall be encoded
            // as defined in Annex F. For a description of the use of this field see Section
            // 7.1.1.2. Application-context-names are structured as UIDs as defined in PS3.5 (see
            // Annex A for an overview of this concept). DICOM Application-context-names are
            // registered in PS3.7.
            let val = codec
                .decode(&bytes.as_ref())
                .context(DecodeTextSnafu {
                    field: "Application-context-name",
                })?;
            Ok(Some(PduVariableItem::ApplicationContext(val)))
        }
        0x20 => {
            // Presentation Context Item Structure (proposed)

            let mut abstract_syntax: Option<String> = None;
            let mut transfer_syntaxes = vec![];

            // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd integers
            // between 1 and 255, encoded as an unsigned binary number. For a complete description
            // of the use of this field see Section 7.1.1.13.
            if bytes.remaining() < 1 { return Ok(None); }
            let presentation_context_id = bytes.get_u8();

            // 6 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            if bytes.remaining() < 1 { return Ok(None); }
            bytes.get_u8();

            // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            if bytes.remaining() < 1 { return Ok(None); }
            bytes.get_u8();

            // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            if bytes.remaining() < 1 { return Ok(None); }
            bytes.get_u8();

            // 9-xxx - Abstract/Transfer Syntax Sub-Items - This variable field shall contain the
            // following sub-items: one Abstract Syntax and one or more Transfer Syntax(es). For a
            // complete description of the use and encoding of these sub-items see Section 9.3.2.2.1
            // and Section 9.3.2.2.2.
            while bytes.has_remaining() {
                // 1 - Item-type - XXH
                if bytes.remaining() < 1 { return Ok(None); }
                let item_type = bytes.get_u8();

                // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested
                // to this value when received.
                if bytes.remaining() < 1 { return Ok(None); }
                bytes.get_u8();

                // 3-4 - Item-length
                if bytes.remaining() < 2 { return Ok(None); }
                let item_length = bytes.get_u16();

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
                        if bytes.remaining() < item_length as usize { return Ok(None); }
                        abstract_syntax = Some(
                            codec
                                .decode(bytes.copy_to_bytes(item_length as usize).as_ref())
                                .context(DecodeTextSnafu {
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
                        if bytes.remaining() < item_length as usize { return Ok(None); }
                        transfer_syntaxes.push(
                            codec
                                .decode(bytes.copy_to_bytes(item_length as usize).as_ref())
                                .context(DecodeTextSnafu {
                                    field: "Transfer-syntax-name",
                                })?
                                .trim()
                                .to_string(),
                        );
                    }
                    _ => {
                        return UnknownPresentationContextSubItemSnafu.fail();
                    }
                }
            }

            Ok(Some(PduVariableItem::PresentationContextProposed(
                PresentationContextProposed {
                    id: presentation_context_id,
                    abstract_syntax: abstract_syntax.context(MissingAbstractSyntaxSnafu)?,
                    transfer_syntaxes,
                },
            )))
        }
        0x21 => {
            // Presentation Context Item Structure (result)

            let mut transfer_syntax: Option<String> = None;

            // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd integers
            // between 1 and 255, encoded as an unsigned binary number. For a complete description
            // of the use of this field see Section 7.1.1.13.
            if bytes.remaining() < 1 { return Ok(None); }
            let presentation_context_id = bytes.get_u8();

            // 6 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            if bytes.remaining() < 1 { return Ok(None); }
            bytes.get_u8();

            // 7 - Result/Reason - This Result/Reason field shall contain an integer value encoded
            // as an unsigned binary number. One of the following values shall be used:
            //   0 - acceptance
            //   1 - user-rejection
            //   2 - no-reason (provider rejection)
            //   3 - abstract-syntax-not-supported (provider rejection)
            //   4 - transfer-syntaxes-not-supported (provider rejection)
            if bytes.remaining() < 1 { return Ok(None); }
            let reason = PresentationContextResultReason::from(bytes.get_u8())
                .context(InvalidPresentationContextResultReasonSnafu)?;

            // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            if bytes.remaining() < 1 { return Ok(None); }
            bytes.get_u8();

            // 9-xxx - Transfer syntax sub-item - This variable field shall contain one Transfer
            // Syntax Sub-Item. When the Result/Reason field has a value other than acceptance (0),
            // this field shall not be significant and its value shall not be tested when received.
            // For a complete description of the use and encoding of this item see Section
            // 9.3.3.2.1.
            while bytes.has_remaining() {
                // 1 - Item-type - XXH
                if bytes.remaining() < 1 { return Ok(None); }
                let item_type = bytes.get_u8();

                // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested
                // to this value when received.
                if bytes.remaining() < 1 { return Ok(None); }
                bytes.get_u8();

                // 3-4 - Item-length
                if bytes.remaining() < 2 { return Ok(None); }
                let item_length = bytes.get_u16();

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
                                return MultipleTransferSyntaxesAcceptedSnafu.fail();
                            }
                            None => {
                                if bytes.remaining() < item_length as usize { return Ok(None); }
                                transfer_syntax = Some(
                                    codec
                                        .decode(bytes.copy_to_bytes(item_length as usize).as_ref())
                                        .context(DecodeTextSnafu {
                                            field: "Transfer-syntax-name",
                                        })?
                                        .trim()
                                        .to_string(),
                                );
                            }
                        }
                    }
                    _ => {
                        return InvalidTransferSyntaxSubItemSnafu.fail();
                    }
                }
            }

            Ok(Some(PduVariableItem::PresentationContextResult(
                PresentationContextResult {
                    id: presentation_context_id,
                    reason,
                    transfer_syntax: transfer_syntax.context(MissingTransferSyntaxSnafu)?,
                },
            )))
        }
        0x50 => {
            // User Information Item Structure

            let mut user_variables = vec![];

            // 5-xxx - User-data - This variable field shall contain User-data sub-items as defined
            // by the DICOM Application Entity. The structure and content of these sub-items is
            // defined in Annex D.
            while bytes.has_remaining(){
                // 1 - Item-type - XXH
                if bytes.remaining() < 1 { return Ok(None); }
                let item_type = bytes.get_u8();

                // 2 - Reserved
                if bytes.remaining() < 1 { return Ok(None); }
                bytes.get_u8();

                // 3-4 - Item-length
                if bytes.remaining() < 2 { return Ok(None); }
                let item_length = bytes.get_u16();

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
                        if bytes.remaining() < 4 { return Ok(None); }
                        user_variables.push(UserVariableItem::MaxLength(
                            bytes.get_u32()
                        ));
                    }
                    0x52 => {
                        // Implementation Class UID Sub-Item Structure

                        // 5 - xxx - Implementation-class-uid - This variable field shall contain
                        // the Implementation-class-uid of the Association-acceptor as defined in
                        // Section D.3.3.2. The Implementation-class-uid field is structured as a
                        // UID as defined in PS3.5.
                        if bytes.remaining() < item_length as usize { return Ok(None); }
                        let implementation_class_uid = codec
                            .decode(bytes.copy_to_bytes(item_length as usize).as_ref())
                            .context(DecodeTextSnafu {
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
                        if bytes.remaining() < item_length as usize { return Ok(None); }
                        let implementation_version_name = codec
                            .decode(bytes.copy_to_bytes(item_length as usize).as_ref())
                            .context(DecodeTextSnafu {
                                field: "Implementation-version-name",
                            })?
                            .trim()
                            .to_string();
                        user_variables.push(UserVariableItem::ImplementationVersionName(
                            implementation_version_name,
                        ));
                    }
                    0x56 => {
                        // SOP Class Extended Negotiation Sub-Item

                        // 5-6 - SOP-class-uid-length - The SOP-class-uid-length shall be the number
                        // of bytes from the first byte of the following field to the last byte of the
                        // SOP-class-uid field. It shall be encoded as an unsigned binary number.
                        if bytes.remaining() < 2 { return Ok(None); }
                        let sop_class_uid_length = bytes.get_u16();

                        // 7 - xxx - SOP-class-uid - The SOP Class or Meta SOP Class identifier
                        // encoded as a UID as defined in Section 9 “Unique Identifiers (UIDs)” in PS3.5.
                        if bytes.remaining() < sop_class_uid_length as usize { return Ok(None); }
                        let sop_class_uid = codec
                            .decode(bytes.copy_to_bytes(sop_class_uid_length as usize).as_ref())
                            .context(DecodeTextSnafu {
                                field: "SOP-class-uid",
                            })?
                            .trim()
                            .to_string();

                        if bytes.remaining() < 2 { return Ok(None); }
                        let data_length = bytes.get_u16();

                        // xxx-xxx - Service-class-application-information -This field shall contain
                        // the application information specific to the Service Class specification
                        // identified by the SOP-class-uid. The semantics and value of this field
                        // is defined in the identified Service Class specification.
                        if bytes.remaining() < data_length as usize { return Ok(None); }
                        let data = bytes.copy_to_bytes(data_length as usize);
                        user_variables.push(UserVariableItem::SopClassExtendedNegotiationSubItem(
                            sop_class_uid,
                            data.to_vec(),
                        ));
                    }
                    0x58 => {
                        // User Identity Negotiation

                        // 5 - User Identity Type
                        if bytes.remaining() < 1 { return Ok(None); }
                        let user_identity_type = bytes.get_u8();

                        // 6 - Positive-response-requested
                        if bytes.remaining() < 1 { return Ok(None); }
                        let positive_response_requested = bytes.get_u8();

                        // 7-8 - Primary Field Length
                        if bytes.remaining() < 2 { return Ok(None); }
                        let primary_field_length = bytes.get_u16();

                        // 9-n - Primary Field
                        if bytes.remaining() < primary_field_length as usize { return Ok(None); }
                        let primary_field = bytes.copy_to_bytes(primary_field_length as usize);
                        // n+1-n+2 - Secondary Field Length
                        // Only non-zero if user identity type is 2 (username and password)
                        if bytes.remaining() < 2 { return Ok(None); }
                        let secondary_field_length = bytes.get_u16();

                        // n+3-m - Secondary Field
                        if bytes.remaining() < secondary_field_length as usize { return Ok(None); }
                        let secondary_field = bytes.copy_to_bytes(secondary_field_length as usize);

                        match UserIdentityType::from(user_identity_type) {
                            Some(user_identity_type) => {
                                user_variables.push(UserVariableItem::UserIdentityItem(
                                    UserIdentity::new(
                                        positive_response_requested == 1,
                                        user_identity_type,
                                        primary_field.to_vec(),
                                        secondary_field.to_vec(),
                                    ),
                                ));
                            }
                            None => {
                                warn!("Unknown User Identity Type code {}", user_identity_type);
                            }
                        }
                    }
                    _ => {
                        if bytes.remaining() < item_length as usize { return Ok(None); }
                        user_variables.push(UserVariableItem::Unknown(
                            item_type,
                            bytes.copy_to_bytes(item_length as usize).to_vec()
                        ));
                    }
                }
            }

            Ok(Some(PduVariableItem::UserVariables(user_variables)))
        }
        _ => Ok(Some(PduVariableItem::Unknown(item_type))),
    }
}
