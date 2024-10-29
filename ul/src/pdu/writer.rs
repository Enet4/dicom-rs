/// PDU writer module
use crate::pdu::*;
use byteordered::byteorder::{BigEndian, WriteBytesExt};
use dicom_encoding::text::TextCodec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::io::Write;

pub type Error = crate::pdu::WriteError;

pub type Result<T> = std::result::Result<T, WriteError>;

#[derive(Debug, Snafu)]
pub enum WriteChunkError {
    #[snafu(display("Failed to build chunk"))]
    BuildChunk {
        #[snafu(backtrace)]
        source: Box<WriteError>,
    },
    #[snafu(display("Failed to write chunk length"))]
    WriteLength {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Failed to write chunk data"))]
    WriteData {
        backtrace: Backtrace,
        source: std::io::Error,
    },
}

fn write_chunk_u32<F>(writer: &mut dyn Write, func: F) -> std::result::Result<(), WriteChunkError>
where
    F: FnOnce(&mut Vec<u8>) -> Result<()>,
{
    let mut data = vec![];
    func(&mut data)
        .map_err(Box::from)
        .context(BuildChunkSnafu)?;

    let length = data.len() as u32;
    writer
        .write_u32::<BigEndian>(length)
        .context(WriteLengthSnafu)?;

    writer.write_all(&data).context(WriteDataSnafu)?;

    Ok(())
}

fn write_chunk_u16<F>(writer: &mut dyn Write, func: F) -> std::result::Result<(), WriteChunkError>
where
    F: FnOnce(&mut Vec<u8>) -> Result<()>,
{
    let mut data = vec![];
    func(&mut data)
        .map_err(Box::from)
        .context(BuildChunkSnafu)?;

    let length = data.len() as u16;
    writer
        .write_u16::<BigEndian>(length)
        .context(WriteLengthSnafu)?;

    writer.write_all(&data).context(WriteDataSnafu)?;

    Ok(())
}

pub fn write_pdu<W>(writer: &mut W, pdu: &Pdu) -> Result<()>
where
    W: Write,
{
    let codec = dicom_encoding::text::DefaultCharacterSetCodec;
    match pdu {
        Pdu::AssociationRQ(AssociationRQ {
            protocol_version,
            calling_ae_title,
            called_ae_title,
            application_context_name,
            presentation_contexts,
            user_variables,
        }) => {
            // A-ASSOCIATE-RQ PDU Structure

            // 1 - PDU-type - 01H
            writer
                .write_u8(0x01)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not
            // tested to this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                // 7-8  Protocol-version - This two byte field shall use one bit to identify
                // each version of the DICOM UL protocol supported by the calling end-system.
                // This is Version 1 and shall be identified with bit 0 set. A receiver of this
                // PDU implementing only this version of the DICOM UL protocol shall only test
                // that bit 0 is set.
                writer
                    .write_u16::<BigEndian>(*protocol_version)
                    .context(WriteFieldSnafu {
                        field: "Protocol-version",
                    })?;

                // 9-10 - Reserved - This reserved field shall be sent with a value 0000H but
                // not tested to this value when received.
                writer
                    .write_u16::<BigEndian>(0x00)
                    .context(WriteReservedSnafu { bytes: 2_u32 })?;

                // 11-26 - Called-AE-title - Destination DICOM Application Name. It shall be
                // encoded as 16 characters as defined by the ISO 646:1990-Basic G0 Set with
                // leading and trailing spaces (20H) being non-significant. The value made of 16
                // spaces (20H) meaning "no Application Name specified" shall not be used. For a
                // complete description of the use of this field, see Section 7.1.1.4.
                let mut ae_title_bytes =
                    codec.encode(called_ae_title).context(EncodeFieldSnafu {
                        field: "Called-AE-title",
                    })?;
                ae_title_bytes.resize(16, b' ');
                writer.write_all(&ae_title_bytes).context(WriteFieldSnafu {
                    field: "Called-AE-title",
                })?;

                // 27-42 - Calling-AE-title - Source DICOM Application Name. It shall be encoded
                // as 16 characters as defined by the ISO 646:1990-Basic G0 Set with leading and
                // trailing spaces (20H) being non-significant. The value made of 16 spaces
                // (20H) meaning "no Application Name specified" shall not be used. For a
                // complete description of the use of this field, see Section 7.1.1.3.
                let mut ae_title_bytes =
                    codec.encode(calling_ae_title).context(EncodeFieldSnafu {
                        field: "Calling-AE-title",
                    })?;
                ae_title_bytes.resize(16, b' ');
                writer.write_all(&ae_title_bytes).context(WriteFieldSnafu {
                    field: "Called-AE-title",
                })?;

                // 43-74 - Reserved - This reserved field shall be sent with a value 00H for all
                // bytes but not tested to this value when received
                writer
                    .write_all(&[0; 32])
                    .context(WriteReservedSnafu { bytes: 32_u32 })?;

                write_pdu_variable_application_context_name(
                    writer,
                    application_context_name,
                    &codec,
                )?;

                for presentation_context in presentation_contexts {
                    write_pdu_variable_presentation_context_proposed(
                        writer,
                        presentation_context,
                        &codec,
                    )?;
                }

                write_pdu_variable_user_variables(writer, user_variables, &codec)?;

                Ok(())
            })
            .context(WriteChunkSnafu {
                name: "A-ASSOCIATE-RQ",
            })?;

            Ok(())
        }
        Pdu::AssociationAC(AssociationAC {
            protocol_version,
            application_context_name,
            called_ae_title,
            calling_ae_title,
            presentation_contexts,
            user_variables,
        }) => {
            // A-ASSOCIATE-AC PDU Structure

            // 1 - PDU-type - 02H
            writer
                .write_u8(0x02)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                // 7-8 - Protocol-version - This two byte field shall use one bit to identify each
                // version of the DICOM UL protocol supported by the calling end-system. This is
                // Version 1 and shall be identified with bit 0 set. A receiver of this PDU
                // implementing only this version of the DICOM UL protocol shall only test that bit
                // 0 is set.
                writer
                    .write_u16::<BigEndian>(*protocol_version)
                    .context(WriteFieldSnafu {
                        field: "Protocol-version",
                    })?;

                // 9-10 - Reserved - This reserved field shall be sent with a value 0000H but not
                // tested to this value when received.
                writer
                    .write_u16::<BigEndian>(0x00)
                    .context(WriteReservedSnafu { bytes: 2_u32 })?;

                // 11-26 - Reserved - This reserved field shall be sent with a value identical to
                // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
                // shall not be tested when received.
                let mut ae_title_bytes =
                    codec.encode(called_ae_title).context(EncodeFieldSnafu {
                        field: "Called-AE-title",
                    })?;
                ae_title_bytes.resize(16, b' ');
                writer.write_all(&ae_title_bytes).context(WriteFieldSnafu {
                    field: "Called-AE-title",
                })?;
                // 27-42 - Reserved - This reserved field shall be sent with a value identical to
                // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
                // shall not be tested when received.
                let mut ae_title_bytes =
                    codec.encode(calling_ae_title).context(EncodeFieldSnafu {
                        field: "Calling-AE-title",
                    })?;
                ae_title_bytes.resize(16, b' ');
                writer.write_all(&ae_title_bytes).context(WriteFieldSnafu {
                    field: "Calling-AE-title",
                })?;

                // 43-74 - Reserved - This reserved field shall be sent with a value identical to
                // the value received in the same field of the A-ASSOCIATE-RQ PDU, but its value
                // shall not be tested when received.
                writer
                    .write_all(&[0; 32])
                    .context(WriteReservedSnafu { bytes: 32_u32 })?;

                // 75-xxx - Variable items - This variable field shall contain the following items:
                // one Application Context Item, one or more Presentation Context Item(s) and one
                // User Information Item. For a complete description of these items see Section
                // 7.1.1.2, Section 7.1.1.14, and Section 7.1.1.6.
                write_pdu_variable_application_context_name(
                    writer,
                    application_context_name,
                    &codec,
                )?;

                for presentation_context in presentation_contexts {
                    write_pdu_variable_presentation_context_result(
                        writer,
                        presentation_context,
                        &codec,
                    )?;
                }

                write_pdu_variable_user_variables(writer, user_variables, &codec)?;

                Ok(())
            })
            .context(WriteChunkSnafu {
                name: "A-ASSOCIATE-AC",
            })
        }
        Pdu::AssociationRJ(AssociationRJ { result, source }) => {
            // 1 - PDU-type - 03H
            writer
                .write_u8(0x03)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested to this value when received.
                writer.write_u8(0x00).context(WriteReservedSnafu { bytes: 1_u32 })?;

                // 8 - Result - This Result field shall contain an integer value encoded as an unsigned binary number. One of the following values shall be used:
                // - 1 - rejected-permanent
                // - 2 - rejected-transient
                writer.write_u8(match result {
                    AssociationRJResult::Permanent => {
                        0x01
                    }
                    AssociationRJResult::Transient => {
                        0x02
                    }
                }).context(WriteFieldSnafu { field: "AssociationRJResult" })?;

                // 9 - Source - This Source field shall contain an integer value encoded as an unsigned binary number. One of the following values shall be used:
                // - 1 - DICOM UL service-user
                // - 2 - DICOM UL service-provider (ACSE related function)
                // - 3 - DICOM UL service-provider (Presentation related function)
                // 10 - Reason/Diag - This field shall contain an integer value encoded as an unsigned binary number.
                // If the Source field has the value (1) "DICOM UL service-user", it shall take one of the following:
                // - 1 - no-reason-given
                // - 2 - application-context-name-not-supported
                // - 3 - calling-AE-title-not-recognized
                // - 4-6 - reserved
                // - 7 - called-AE-title-not-recognized
                // - 8-10 - reserved
                // If the Source field has the value (2) "DICOM UL service provided (ACSE related function)", it shall take one of the following:
                // - 1 - no-reason-given
                // - 2 - protocol-version-not-supported
                // If the Source field has the value (3) "DICOM UL service provided (Presentation related function)", it shall take one of the following:
                // 0 - reserved
                // 1 - temporary-congestion
                // 2 - local-limit-exceeded
                // 3-7 - reserved
                match source {
                    AssociationRJSource::ServiceUser(reason) => {
                        writer.write_u8(0x01).context(WriteFieldSnafu { field: "AssociationRJServiceUserReason" })?;
                        writer.write_u8(match reason {
                            AssociationRJServiceUserReason::NoReasonGiven => {
                                0x01
                            }
                            AssociationRJServiceUserReason::ApplicationContextNameNotSupported => {
                                0x02
                            }
                            AssociationRJServiceUserReason::CallingAETitleNotRecognized => {
                                0x03
                            }
                            AssociationRJServiceUserReason::CalledAETitleNotRecognized => {
                                0x07
                            }
                            AssociationRJServiceUserReason::Reserved(data) => {
                                *data
                            }
                        }).context(WriteFieldSnafu { field: "AssociationRJServiceUserReason (2)" })?;
                    }
                    AssociationRJSource::ServiceProviderASCE(reason) => {
                        writer.write_u8(0x02).context(WriteFieldSnafu { field: "AssociationRJServiceProvider" })?;
                        writer.write_u8(match reason {
                            AssociationRJServiceProviderASCEReason::NoReasonGiven => {
                                0x01
                            }
                            AssociationRJServiceProviderASCEReason::ProtocolVersionNotSupported => {
                                0x02
                            }
                        }).context(WriteFieldSnafu { field: "AssociationRJServiceProvider (2)" })?;
                    }
                    AssociationRJSource::ServiceProviderPresentation(reason) => {
                        writer.write_u8(0x03).context(WriteFieldSnafu { field: "AssociationRJServiceProviderPresentationReason" })?;
                        writer.write_u8(match reason {
                            AssociationRJServiceProviderPresentationReason::TemporaryCongestion => {
                                0x01
                            }
                            AssociationRJServiceProviderPresentationReason::LocalLimitExceeded => {
                                0x02
                            }
                            AssociationRJServiceProviderPresentationReason::Reserved(data) => {
                                *data
                            }
                        }).context(WriteFieldSnafu { field: "AssociationRJServiceProviderPresentationReason (2)" })?;
                    }
                }

                Ok(())
            }).context(WriteChunkSnafu { name: "AssociationRJ" })?;

            Ok(())
        }
        Pdu::PData { data } => {
            // 1 - PDU-type - 04H
            writer
                .write_u8(0x04)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                // 7-xxx - Presentation-data-value Item(s) - This variable data field shall contain
                // one or more Presentation-data-value Items(s). For a complete description of the
                // use of this field see Section 9.3.5.1

                for presentation_data_value in data {
                    write_chunk_u32(writer, |writer| {
                        // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd
                        // integers between 1 and 255, encoded as an unsigned binary number. For a
                        // complete description of the use of this field see Section 7.1.1.13.
                        writer.push(presentation_data_value.presentation_context_id);

                        // 6-xxx - Presentation-data-value - This Presentation-data-value field
                        // shall contain DICOM message information (command and/or data set) with a
                        // message control header. For a complete description of the use of this
                        // field see Annex E.

                        // The Message Control Header shall be made of one byte with the least
                        // significant bit (bit 0) taking one of the following values:
                        // - If bit 0 is set to 1, the following fragment shall contain Message
                        //   Command information.
                        // - If bit 0 is set to 0, the following fragment shall contain Message Data
                        //   Set information.
                        // The next least significant bit (bit 1) shall be defined by the following
                        // rules: If bit 1 is set to 1, the following fragment shall contain the
                        // last fragment of a Message Data Set or of a Message Command.
                        // - If bit 1 is set to 0, the following fragment does not contain the last
                        //   fragment of a Message Data Set or of a Message Command.
                        let mut message_header = 0x00;
                        if let PDataValueType::Command = presentation_data_value.value_type {
                            message_header |= 0x01;
                        }
                        if presentation_data_value.is_last {
                            message_header |= 0x02;
                        }
                        writer.push(message_header);

                        // Message fragment
                        writer.extend(&presentation_data_value.data);

                        Ok(())
                    })
                    .context(WriteChunkSnafu {
                        name: "Presentation-data-value item",
                    })?;
                }

                Ok(())
            })
            .context(WriteChunkSnafu { name: "PData" })
        }
        Pdu::ReleaseRQ => {
            // 1 - PDU-type - 05H
            writer
                .write_u8(0x05)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                writer.extend([0u8; 4]);
                Ok(())
            })
            .context(WriteChunkSnafu { name: "ReleaseRQ" })?;

            Ok(())
        }
        Pdu::ReleaseRP => {
            // 1 - PDU-type - 06H
            writer
                .write_u8(0x06)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                writer.extend([0u8; 4]);
                Ok(())
            })
            .context(WriteChunkSnafu { name: "ReleaseRP" })?;

            Ok(())
        }
        Pdu::AbortRQ { source } => {
            // 1 - PDU-type - 07H
            writer
                .write_u8(0x07)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                // 7 - Reserved - This reserved field shall be sent with a value 00H but not tested
                // to this value when received.
                writer.push(0);
                // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested
                // to this value when received.
                writer.push(0);

                // 9 - Source - This Source field shall contain an integer value encoded as an
                // unsigned binary number. One of the following values shall be used:
                // - 0 - DICOM UL service-user (initiated abort)
                // - 1 - reserved
                // - 2 - DICOM UL service-provider (initiated abort)
                // 10 - Reason/Diag - This field shall contain an integer value encoded as an
                // unsigned binary number. If the Source field has the value (2) "DICOM UL
                // service-provider", it shall take one of the following:
                // - 0 - reason-not-specified1 - unrecognized-PDU
                // - 2 - unexpected-PDU
                // - 3 - reserved
                // - 4 - unrecognized-PDU parameter
                // - 5 - unexpected-PDU parameter
                // - 6 - invalid-PDU-parameter value
                // If the Source field has the value (0) "DICOM UL service-user", this reason field
                // shall not be significant. It shall be sent with a value 00H but not tested to
                // this value when received.
                let source_word = match source {
                    AbortRQSource::ServiceUser => [0x00; 2],
                    AbortRQSource::Reserved => [0x01, 0x00],
                    AbortRQSource::ServiceProvider(reason) => match reason {
                        AbortRQServiceProviderReason::ReasonNotSpecified => [0x02, 0x00],
                        AbortRQServiceProviderReason::UnrecognizedPdu => [0x02, 0x01],
                        AbortRQServiceProviderReason::UnexpectedPdu => [0x02, 0x02],
                        AbortRQServiceProviderReason::Reserved => [0x02, 0x03],
                        AbortRQServiceProviderReason::UnrecognizedPduParameter => [0x02, 0x04],
                        AbortRQServiceProviderReason::UnexpectedPduParameter => [0x02, 0x05],
                        AbortRQServiceProviderReason::InvalidPduParameter => [0x02, 0x06],
                    },
                };
                writer.extend(source_word);

                Ok(())
            })
            .context(WriteChunkSnafu { name: "AbortRQ" })?;

            Ok(())
        }
        Pdu::Unknown { pdu_type, data } => {
            // 1 - PDU-type - XXH
            writer
                .write_u8(*pdu_type)
                .context(WriteFieldSnafu { field: "PDU-type" })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to
            // this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u32(writer, |writer| {
                writer.extend(data);
                Ok(())
            })
            .context(WriteChunkSnafu { name: "Unknown" })?;

            Ok(())
        }
    }
}

fn write_pdu_variable_application_context_name(
    writer: &mut dyn Write,
    application_context_name: &str,
    codec: &dyn TextCodec,
) -> Result<()> {
    // Application Context Item Structure
    // 1 - Item-type - 10H
    writer
        .write_u8(0x10)
        .context(WriteFieldSnafu { field: "Item-type" })?;

    // 2 - Reserved - This reserved field shall be sent with a value 00H but not
    // tested to this value when received.
    writer
        .write_u8(0x00)
        .context(WriteReservedSnafu { bytes: 1_u32 })?;

    write_chunk_u16(writer, |writer| {
        // 5-xxx - Application-context-name -A valid Application-context-name shall
        // be encoded as defined in Annex F. For a description of the use of this
        // field see Section 7.1.1.2. Application-context-names are structured as
        // UIDs as defined in PS3.5 (see Annex A for an overview of this concept).
        // DICOM Application-context-names are registered in PS3.7.
        writer
            .write_all(
                &codec
                    .encode(application_context_name)
                    .context(EncodeFieldSnafu {
                        field: "Application-context-name",
                    })?,
            )
            .context(WriteFieldSnafu {
                field: "Application-context-name",
            })
    })
    .context(WriteChunkSnafu {
        name: "Application Context Item",
    })?;

    Ok(())
}

fn write_pdu_variable_presentation_context_proposed(
    writer: &mut dyn Write,
    presentation_context: &PresentationContextProposed,
    codec: &dyn TextCodec,
) -> Result<()> {
    // Presentation Context Item Structure
    // 1 - tem-type - 20H
    writer
        .write_u8(0x20)
        .context(WriteFieldSnafu { field: "Item-type" })?;

    // 2 - Reserved - This reserved field shall be sent with a value 00H but not
    // tested to this value when received.
    writer
        .write_u8(0x00)
        .context(WriteReservedSnafu { bytes: 1_u32 })?;

    write_chunk_u16(writer, |writer| {
        // 5 - Presentation-context-ID - Presentation-context-ID values shall be
        // odd integers between 1 and 255, encoded as an unsigned binary number.
        // For a complete description of the use of this field see Section
        // 7.1.1.13.
        writer
            .write_u8(presentation_context.id)
            .context(WriteFieldSnafu {
                field: "Presentation-context-ID",
            })?;

        // 6 - Reserved - This reserved field shall be sent with a value 00H but
        // not tested to this value when received.
        writer
            .write_u8(0x00)
            .context(WriteReservedSnafu { bytes: 1_u32 })?;

        // 7 - Reserved - This reserved field shall be sent with a value 00H but
        // not tested to this value when received
        writer
            .write_u8(0x00)
            .context(WriteReservedSnafu { bytes: 1_u32 })?;

        // 8 - Reserved - This reserved field shall be sent with a value 00H but
        // not tested to this value when received.
        writer
            .write_u8(0x00)
            .context(WriteReservedSnafu { bytes: 1_u32 })?;

        // 9-xxx - Abstract/Transfer Syntax Sub-Items - This variable field
        // shall contain the following sub-items: one Abstract Syntax and one or
        // more Transfer Syntax(es). For a complete description of the use and
        // encoding of these sub-items see Section 9.3.2.2.1 and Section
        // 9.3.2.2.2.

        // Abstract Syntax Sub-Item Structure
        // 1 - Item-type 30H
        writer
            .write_u8(0x30)
            .context(WriteFieldSnafu { field: "Item-type" })?;

        // 2 - Reserved - This reserved field shall be sent with a value 00H
        // but not tested to this value when
        // received.
        writer
            .write_u8(0x00)
            .context(WriteReservedSnafu { bytes: 1_u32 })?;

        write_chunk_u16(writer, |writer| {
            // 5-xxx - Abstract-syntax-name - This variable field shall
            // contain
            // the Abstract-syntax-name related to the proposed presentation
            // context. A valid Abstract-syntax-name shall be encoded as
            // defined in Annex F. For a
            // description of the use of this field see
            // Section 7.1.1.13. Abstract-syntax-names are structured as
            // UIDs as defined in PS3.5
            // (see Annex B for an overview of this concept).
            // DICOM Abstract-syntax-names are registered in PS3.4.
            writer
                .write_all(
                    &codec
                        .encode(&presentation_context.abstract_syntax)
                        .context(EncodeFieldSnafu {
                            field: "Abstract-syntax-name",
                        })?,
                )
                .context(WriteFieldSnafu {
                    field: "Abstract-syntax-name",
                })
        })
        .context(WriteChunkSnafu {
            name: "Abstract Syntax Item",
        })?;

        for transfer_syntax in &presentation_context.transfer_syntaxes {
            // Transfer Syntax Sub-Item Structure
            // 1 - Item-type - 40H
            writer.write_u8(0x40).context(WriteFieldSnafu {
                field: "Presentation-context Item-type",
            })?;

            // 2 - Reserved - This reserved field shall be sent with a value 00H
            // but not tested to this value when received.
            writer
                .write_u8(0x00)
                .context(WriteReservedSnafu { bytes: 1_u32 })?;

            write_chunk_u16(writer, |writer| {
                // 5-xxx - Transfer-syntax-name(s) - This variable field shall
                // contain the Transfer-syntax-name proposed for this
                // presentation context. A valid Transfer-syntax-name shall be
                // encoded as defined in Annex F. For a description of the use
                // of this field see Section 7.1.1.13. Transfer-syntax-names are
                // structured as UIDs as defined in PS3.5 (see Annex B for an
                // overview of this concept). DICOM Transfer-syntax-names are
                // registered in PS3.5.
                writer
                    .write_all(&codec.encode(transfer_syntax).context(EncodeFieldSnafu {
                        field: "Transfer-syntax-name",
                    })?)
                    .context(WriteFieldSnafu {
                        field: "Transfer-syntax-name",
                    })
            })
            .context(WriteChunkSnafu {
                name: "Transfer Syntax Sub-Item",
            })?;
        }

        Ok(())
    })
    .context(WriteChunkSnafu {
        name: "Presentation Context Item",
    })?;

    Ok(())
}

fn write_pdu_variable_presentation_context_result(
    writer: &mut dyn Write,
    presentation_context: &PresentationContextResult,
    codec: &dyn TextCodec,
) -> Result<()> {
    // 1 - Item-type - 21H
    writer
        .write_u8(0x21)
        .context(WriteFieldSnafu { field: "Item-type" })?;

    // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to this
    // value when received.
    writer
        .write_u8(0x00)
        .context(WriteReservedSnafu { bytes: 1_u32 })?;

    write_chunk_u16(writer, |writer| {
        // 5 - Presentation-context-ID - Presentation-context-ID values shall be odd integers
        // between 1 and 255, encoded as an unsigned binary number. For a complete description of
        // the use of this field see Section 7.1.1.13.
        writer
            .write_u8(presentation_context.id)
            .context(WriteFieldSnafu {
                field: "Presentation-context-ID",
            })?;

        // 6 - Reserved - This reserved field shall be sent with a value 00H but not tested to this
        // value when received.
        writer
            .write_u8(0x00)
            .context(WriteReservedSnafu { bytes: 1_u32 })?;

        // 7 - Result/Reason - This Result/Reason field shall contain an integer value encoded as an
        // unsigned binary number. One of the following values shall be used:
        //   0 - acceptance
        //   1 - user-rejection
        //   2 - no-reason (provider rejection)
        //   3 - abstract-syntax-not-supported (provider rejection)
        //   4 - transfer-syntaxes-not-supported (provider rejection)
        writer
            .write_u8(match &presentation_context.reason {
                PresentationContextResultReason::Acceptance => 0,
                PresentationContextResultReason::UserRejection => 1,
                PresentationContextResultReason::NoReason => 2,
                PresentationContextResultReason::AbstractSyntaxNotSupported => 3,
                PresentationContextResultReason::TransferSyntaxesNotSupported => 4,
            })
            .context(WriteFieldSnafu {
                field: "Presentation Context Result/Reason",
            })?;

        // 8 - Reserved - This reserved field shall be sent with a value 00H but not tested to this
        // value when received.
        writer
            .write_u8(0x00)
            .context(WriteReservedSnafu { bytes: 1_u32 })?;

        // 9-xxx - Transfer syntax sub-item - This variable field shall contain one Transfer Syntax
        // Sub-Item. When the Result/Reason field has a value other than acceptance (0), this field
        // shall not be significant and its value shall not be tested when received. For a complete
        // description of the use and encoding of this item see Section 9.3.3.2.1.

        // 1 - Item-type - 40H
        writer
            .write_u8(0x40)
            .context(WriteFieldSnafu { field: "Item-type" })?;

        // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to this
        // value when received.
        writer
            .write_u8(0x40)
            .context(WriteReservedSnafu { bytes: 1_u32 })?;

        write_chunk_u16(writer, |writer| {
            // 5-xxx - Transfer-syntax-name - This variable field shall contain the
            // Transfer-syntax-name proposed for this presentation context. A valid
            // Transfer-syntax-name shall be encoded as defined in Annex F. For a description of the
            // use of this field see Section 7.1.1.14. Transfer-syntax-names are structured as UIDs
            // as defined in PS3.5 (see Annex B for an overview of this concept). DICOM
            // Transfer-syntax-names are registered in PS3.5.
            writer
                .write_all(
                    &codec
                        .encode(&presentation_context.transfer_syntax)
                        .context(EncodeFieldSnafu {
                            field: "Transfer-syntax-name",
                        })?,
                )
                .context(WriteFieldSnafu {
                    field: "Transfer-syntax-name",
                })?;

            Ok(())
        })
        .context(WriteChunkSnafu {
            name: "Transfer Syntax sub-item",
        })?;

        Ok(())
    })
    .context(WriteChunkSnafu {
        name: "Presentation-context",
    })
}

fn write_pdu_variable_user_variables(
    writer: &mut dyn Write,
    user_variables: &[UserVariableItem],
    codec: &dyn TextCodec,
) -> Result<()> {
    if user_variables.is_empty() {
        return Ok(());
    }

    // 1 - Item-type - 50H
    writer
        .write_u8(0x50)
        .context(WriteFieldSnafu { field: "Item-type" })?;

    // 2 - Reserved - This reserved field shall be sent with a value 00H but not tested to this
    // value when received.
    writer
        .write_u8(0x00)
        .context(WriteReservedSnafu { bytes: 1_u32 })?;

    write_chunk_u16(writer, |writer| {
        // 5-xxx - User-data - This variable field shall contain User-data sub-items as defined by
        // the DICOM Application Entity. The structure and content of these sub-items is defined in
        // Annex D.
        for user_variable in user_variables {
            match user_variable {
                UserVariableItem::MaxLength(max_length) => {
                    // 1 - Item-type - 51H
                    writer
                        .write_u8(0x51)
                        .context(WriteFieldSnafu { field: "Item-type" })?;

                    // 2 - Reserved - This reserved field shall be sent with a value 00H but not
                    // tested to this value when received.
                    writer
                        .write_u8(0x00)
                        .context(WriteReservedSnafu { bytes: 1_u32 })?;

                    write_chunk_u16(writer, |writer| {
                        // 5-8 - Maximum-length-received - This parameter allows the
                        // association-requestor to restrict the maximum length of the variable
                        // field of the P-DATA-TF PDUs sent by the acceptor on the association once
                        // established. This length value is indicated as a number of bytes encoded
                        // as an unsigned binary number. The value of (0) indicates that no maximum
                        // length is specified. This maximum length value shall never be exceeded by
                        // the PDU length values used in the PDU-length field of the P-DATA-TF PDUs
                        // received by the association-requestor. Otherwise, it shall be a protocol
                        // error.
                        writer
                            .write_u32::<BigEndian>(*max_length)
                            .context(WriteFieldSnafu {
                                field: "Maximum-length-received",
                            })
                    })
                    .context(WriteChunkSnafu {
                        name: "Maximum-length-received",
                    })?;
                }
                UserVariableItem::ImplementationVersionName(implementation_version_name) => {
                    // 1 - Item-type - 55H
                    writer
                        .write_u8(0x55)
                        .context(WriteFieldSnafu { field: "Item-type" })?;

                    // 2 - Reserved - This reserved field shall be sent with a value 00H but not
                    // tested to this value when received.
                    writer
                        .write_u8(0x00)
                        .context(WriteReservedSnafu { bytes: 1_u32 })?;

                    write_chunk_u16(writer, |writer| {
                        // 5 - xxx - Implementation-version-name - This variable field shall contain
                        // the Implementation-version-name of the Association-acceptor as defined in
                        // Section D.3.3.2. It shall be encoded as a string of 1 to 16 ISO 646:1990
                        // (basic G0 set) characters.
                        writer
                            .write_all(&codec.encode(implementation_version_name).context(
                                EncodeFieldSnafu {
                                    field: "Implementation-version-name",
                                },
                            )?)
                            .context(WriteFieldSnafu {
                                field: "Implementation-version-name",
                            })
                    })
                    .context(WriteChunkSnafu {
                        name: "Implementation-version-name",
                    })?;
                }
                UserVariableItem::ImplementationClassUID(implementation_class_uid) => {
                    // 1 - Item-type - 52H
                    writer
                        .write_u8(0x52)
                        .context(WriteFieldSnafu { field: "Item-type" })?;

                    // 2 - Reserved - This reserved field shall be sent with a value 00H but not
                    // tested to this value when received.
                    writer
                        .write_u8(0x00)
                        .context(WriteReservedSnafu { bytes: 1_u32 })?;

                    write_chunk_u16(writer, |writer| {
                        //5 - xxx - Implementation-class-uid - This variable field shall contain
                        // the Implementation-class-uid of the Association-acceptor as defined in
                        // Section D.3.3.2. The Implementation-class-uid field is structured as a
                        // UID as defined in PS3.5.
                        writer
                            .write_all(&codec.encode(implementation_class_uid).context(
                                EncodeFieldSnafu {
                                    field: "Implementation-class-uid",
                                },
                            )?)
                            .context(WriteFieldSnafu {
                                field: "Implementation-class-uid",
                            })
                    })
                    .context(WriteChunkSnafu {
                        name: "Implementation-class-uid",
                    })?;
                }
                UserVariableItem::SopClassExtendedNegotiationSubItem(sop_class_uid, data) => {
                    // 1 - Item-type - 56H
                    writer
                        .write_u8(0x56)
                        .context(WriteFieldSnafu { field: "Item-type" })?;
                    // 2 - Reserved - This reserved field shall be sent with a value 00H but not
                    // tested to this value when received.
                    writer
                        .write_u8(0x00)
                        .context(WriteReservedSnafu { bytes: 1_u32 })?;

                    write_chunk_u16(writer, |writer| {
                        write_chunk_u16(writer, |writer| {
                            //  7-xxx - The SOP Class or Meta SOP Class identifier encoded as a UID
                            //  as defined in Section 9 “Unique Identifiers (UIDs)” in PS3.5.
                            writer
                                .write_all(&codec.encode(sop_class_uid).context(
                                    EncodeFieldSnafu {
                                        field: "SOP-class-uid",
                                    },
                                )?)
                                .context(WriteFieldSnafu {
                                    field: "SOP-class-uid",
                                })
                        })
                        .context(WriteChunkSnafu {
                            name: "SOP-class-uid",
                        })?;

                        write_chunk_u16(writer, |writer| {
                            // xxx-xxx Service-class-application-information - This field shall contain
                            // the application information specific to the Service Class specification
                            // identified by the SOP-class-uid. The semantics and value of this field is
                            // defined in the identified Service Class specification.
                            writer.write_all(data).context(WriteFieldSnafu {
                                field: "Service-class-application-information",
                            })
                        })
                        .context(WriteChunkSnafu {
                            name: "Service-class-application-information",
                        })
                    })
                    .context(WriteChunkSnafu { name: "Sub-item" })?;
                }
                UserVariableItem::UserIdentityItem(user_identity) => {
                    // 1 - Item-type - 58H
                    writer
                        .write_u8(0x58)
                        .context(WriteFieldSnafu { field: "Item-type" })?;

                    // 2 - Reserved - This reserved field shall be sent with a value 00H but not
                    // tested to this value when received.
                    writer
                        .write_u8(0x00)
                        .context(WriteReservedSnafu { bytes: 1_u32 })?;

                    // 3-4 - Item-length
                    write_chunk_u16(writer, |writer| {
                        // 5 - User-Identity-Type
                        writer
                            .write_u8(user_identity.identity_type().to_u8())
                            .context(WriteFieldSnafu {
                                field: "User-Identity-Type",
                            })?;

                        // 6 - Positive-response-requested
                        let positive_response_requested_out: u8 =
                            if user_identity.positive_response_requested() {
                                1
                            } else {
                                0
                            };
                        writer.write_u8(positive_response_requested_out).context(
                            WriteFieldSnafu {
                                field: "Positive-response-requested",
                            },
                        )?;

                        // 7-8 - Primary-field-length
                        write_chunk_u16(writer, |writer| {
                            // 9-n - Primary-field
                            writer
                                .write_all(user_identity.primary_field().as_slice())
                                .context(WriteFieldSnafu {
                                    field: "Primary-field",
                                })
                        })
                        .context(WriteChunkSnafu {
                            name: "Primary-field",
                        })?;

                        // n+1-n+2 - Secondary-field-length
                        write_chunk_u16(writer, |writer| {
                            // n+3-m - Secondary-field
                            writer
                                .write_all(user_identity.secondary_field().as_slice())
                                .context(WriteFieldSnafu {
                                    field: "Secondary-field",
                                })
                        })
                        .context(WriteChunkSnafu {
                            name: "Secondary-field",
                        })
                    })
                    .context(WriteChunkSnafu {
                        name: "Item-length",
                    })?;
                }
                UserVariableItem::Unknown(item_type, data) => {
                    writer
                        .write_u8(*item_type)
                        .context(WriteFieldSnafu { field: "Item-type" })?;

                    writer
                        .write_u8(0x00)
                        .context(WriteReservedSnafu { bytes: 1_u32 })?;

                    write_chunk_u16(writer, |writer| {
                        writer.write_all(data).context(WriteFieldSnafu {
                            field: "Unknown Data",
                        })
                    })
                    .context(WriteChunkSnafu { name: "Unknown" })?;
                }
            }
        }

        Ok(())
    })
    .context(WriteChunkSnafu { name: "User-data" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_write_chunks_with_preceding_u32_length() -> Result<()> {
        let mut bytes = vec![0u8; 0];
        write_chunk_u32(&mut bytes, |writer| {
            writer
                .write_u8(0x02)
                .context(WriteFieldSnafu { field: "Field1" })?;
            write_chunk_u32(writer, |writer| {
                writer
                    .write_u8(0x03)
                    .context(WriteFieldSnafu { field: "Field2" })?;
                Ok(())
            })
            .context(WriteChunkSnafu { name: "Chunk2" })
        })
        .context(WriteChunkSnafu { name: "Chunk1" })?;

        assert_eq!(bytes.len(), 10);
        assert_eq!(bytes, &[0, 0, 0, 6, 2, 0, 0, 0, 1, 3]);

        Ok(())
    }

    #[test]
    fn can_write_chunks_with_preceding_u16_length() -> Result<()> {
        let mut bytes = vec![0u8; 0];
        write_chunk_u16(&mut bytes, |writer| {
            writer
                .write_u8(0x02)
                .context(WriteFieldSnafu { field: "Field1" })?;
            write_chunk_u16(writer, |writer| {
                writer
                    .write_u8(0x03)
                    .context(WriteFieldSnafu { field: "Field2" })?;
                Ok(())
            })
            .context(WriteChunkSnafu { name: "Chunk2" })
        })
        .context(WriteChunkSnafu { name: "Chunk1" })?;

        assert_eq!(bytes.len(), 6);
        assert_eq!(bytes, &[0, 4, 2, 0, 1, 3]);

        Ok(())
    }

    #[test]
    fn write_abort_rq() {
        let mut out = vec![];

        // abort by request of SCU
        let pdu = Pdu::AbortRQ {
            source: AbortRQSource::ServiceUser,
        };
        write_pdu(&mut out, &pdu).unwrap();
        assert_eq!(
            &out,
            &[
                // code 7 + reserved byte
                0x07, 0x00, //
                // PDU length: 4 bytes
                0x00, 0x00, 0x00, 0x04, //
                // reserved 2 bytes + source: service user (0) + reason (0)
                0x00, 0x00, 0x00, 0x00,
            ]
        );
        out.clear();

        // Reserved
        let pdu = Pdu::AbortRQ {
            source: AbortRQSource::Reserved,
        };
        write_pdu(&mut out, &pdu).unwrap();
        assert_eq!(
            &out,
            &[
                // code 7 + reserved byte
                0x07, 0x00, //
                // PDU length: 4 bytes
                0x00, 0x00, 0x00, 0x04, //
                // reserved 2 bytes + source: reserved (1) + reason (0)
                0x00, 0x00, 0x01, 0x00,
            ]
        );
        out.clear();

        // abort by request of SCP
        let pdu = Pdu::AbortRQ {
            source: AbortRQSource::ServiceProvider(
                AbortRQServiceProviderReason::InvalidPduParameter,
            ),
        };
        write_pdu(&mut out, &pdu).unwrap();
        assert_eq!(
            &out,
            &[
                // code 7 + reserved byte
                0x07, 0x00, //
                // PDU length: 4 bytes
                0x00, 0x00, 0x00, 0x04, //
                // reserved 2 bytes
                0x00, 0x00, //
                // source: service provider (2), invalid parameter value (6)
                0x02, 0x06,
            ]
        );
    }
}
