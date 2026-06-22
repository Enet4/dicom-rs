use dicom_ul::pdu::reader::read_pdu;
use dicom_ul::pdu::writer::write_pdu;
use dicom_ul::pdu::{
    AssociationRQ, PDataValue, PDataValueType, Pdu, PresentationContextProposed, UserIdentity,
    UserIdentityType, UserVariableItem, DEFAULT_MAX_PDU,
};
use matches::matches;
use std::io::Cursor;

#[test]
fn can_read_write_associate_rq() -> Result<(), Box<dyn std::error::Error>> {
    let association_rq = AssociationRQ {
        protocol_version: 2,
        calling_ae_title: "calling ae".to_string(),
        called_ae_title: "called ae".to_string(),
        application_context_name: "application context name".to_string(),
        presentation_contexts: vec![
            PresentationContextProposed {
                id: 1,
                abstract_syntax: "abstract 1".to_string(),
                transfer_syntaxes: vec!["transfer 1".to_string(), "transfer 2".to_string()],
            },
            PresentationContextProposed {
                id: 3,
                abstract_syntax: "abstract 2".to_string(),
                transfer_syntaxes: vec!["transfer 3".to_string(), "transfer 4".to_string()],
            },
        ],
        user_variables: vec![
            UserVariableItem::ImplementationClassUID("class uid".to_string()),
            UserVariableItem::ImplementationVersionName("version name".to_string()),
            UserVariableItem::MaxLength(23),
            UserVariableItem::SopClassExtendedNegotiationSubItem(
                "abstract 1".to_string(),
                vec![1, 1, 0, 1, 1, 0, 1],
            ),
            UserVariableItem::UserIdentityItem(UserIdentity::new(
                false,
                UserIdentityType::UsernamePassword,
                b"MyUsername".to_vec(),
                b"MyPassword".to_vec(),
            )),
        ],
    };

    let mut bytes = vec![0u8; 0];
    write_pdu(&mut bytes, &association_rq.into())?;

    let result = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU, true)?.unwrap();

    if let Pdu::AssociationRQ(AssociationRQ {
        protocol_version,
        calling_ae_title,
        called_ae_title,
        application_context_name,
        presentation_contexts,
        user_variables,
    }) = result
    {
        assert_eq!(protocol_version, 2);
        assert_eq!(calling_ae_title, "calling ae");
        assert_eq!(called_ae_title, "called ae");
        assert_eq!(
            application_context_name,
            "application context name".to_string()
        );
        assert_eq!(presentation_contexts.len(), 2);
        assert_eq!(presentation_contexts[0].abstract_syntax, "abstract 1");
        assert_eq!(presentation_contexts[0].transfer_syntaxes.len(), 2);
        assert_eq!(presentation_contexts[0].transfer_syntaxes[0], "transfer 1");
        assert_eq!(presentation_contexts[0].transfer_syntaxes[1], "transfer 2");
        assert_eq!(presentation_contexts[1].abstract_syntax, "abstract 2");
        assert_eq!(presentation_contexts[1].transfer_syntaxes.len(), 2);
        assert_eq!(presentation_contexts[1].transfer_syntaxes[0], "transfer 3");
        assert_eq!(presentation_contexts[1].transfer_syntaxes[1], "transfer 4");
        assert_eq!(user_variables.len(), 5);
        assert!(matches!(
            &user_variables[0],
            UserVariableItem::ImplementationClassUID(u) if u == "class uid"
        ));
        assert!(matches!(
            &user_variables[1],
            UserVariableItem::ImplementationVersionName(v) if v == "version name"
        ));
        assert!(matches!(user_variables[2], UserVariableItem::MaxLength(l) if l == 23));
        assert!(matches!(&user_variables[3],
            UserVariableItem::SopClassExtendedNegotiationSubItem(sop_class_uid, data)
            if sop_class_uid ==  "abstract 1" &&
            data.as_slice() == [1,1,0,1,1,0,1]
        ));
        assert!(matches!(&user_variables[4],
            UserVariableItem::UserIdentityItem(user_identity)
            if !user_identity.positive_response_requested() &&
            user_identity.identity_type() == UserIdentityType::UsernamePassword &&
            user_identity.primary_field() == [77,121,85,115,101,114,110,97,109,101] &&
            user_identity.secondary_field() == [77,121,80,97,115,115,119,111,114,100]
        ));
    } else {
        panic!("invalid pdu type");
    }

    Ok(())
}

#[test]
fn can_read_write_primary_field_only_user_identity() -> Result<(), Box<dyn std::error::Error>> {
    let association_rq = AssociationRQ {
        protocol_version: 2,
        calling_ae_title: "calling ae".to_string(),
        called_ae_title: "called ae".to_string(),
        application_context_name: "application context name".to_string(),
        presentation_contexts: vec![PresentationContextProposed {
            id: 1,
            abstract_syntax: "abstract 1".to_string(),
            transfer_syntaxes: vec!["transfer 1".to_string()],
        }],
        user_variables: vec![
            UserVariableItem::ImplementationClassUID("class uid".to_string()),
            UserVariableItem::ImplementationVersionName("version name".to_string()),
            UserVariableItem::MaxLength(23),
            UserVariableItem::SopClassExtendedNegotiationSubItem(
                "abstract 1".to_string(),
                vec![1, 1, 0, 1, 1, 0, 1],
            ),
            UserVariableItem::UserIdentityItem(UserIdentity::new(
                false,
                UserIdentityType::Username,
                b"MyUsername".to_vec(),
                vec![],
            )),
        ],
    };

    let mut bytes = vec![0u8; 0];
    write_pdu(&mut bytes, &association_rq.into())?;

    let result = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU, true)?.unwrap();

    if let Pdu::AssociationRQ(AssociationRQ {
        protocol_version,
        calling_ae_title,
        called_ae_title,
        application_context_name,
        presentation_contexts,
        user_variables,
    }) = result
    {
        assert_eq!(protocol_version, 2);
        assert_eq!(calling_ae_title, "calling ae");
        assert_eq!(called_ae_title, "called ae");
        assert_eq!(
            application_context_name,
            "application context name".to_string()
        );
        assert_eq!(presentation_contexts.len(), 1);
        assert_eq!(presentation_contexts[0].abstract_syntax, "abstract 1");
        assert_eq!(presentation_contexts[0].transfer_syntaxes.len(), 1);
        assert_eq!(presentation_contexts[0].transfer_syntaxes[0], "transfer 1");
        assert_eq!(user_variables.len(), 5);
        assert!(matches!(
            &user_variables[0],
            UserVariableItem::ImplementationClassUID(u) if u == "class uid"
        ));
        assert!(matches!(
            &user_variables[1],
            UserVariableItem::ImplementationVersionName(v) if v == "version name"
        ));
        assert!(matches!(user_variables[2], UserVariableItem::MaxLength(l) if l == 23));
        assert!(matches!(&user_variables[3],
            UserVariableItem::SopClassExtendedNegotiationSubItem(sop_class_uid, data)
            if sop_class_uid ==  "abstract 1" &&
            data.as_slice() == [1,1,0,1,1,0,1]
        ));
        assert!(matches!(&user_variables[4],
            UserVariableItem::UserIdentityItem(user_identity)
            if !user_identity.positive_response_requested() &&
            user_identity.identity_type() == UserIdentityType::Username &&
            user_identity.primary_field() == [77,121,85,115,101,114,110,97,109,101] &&
            user_identity.secondary_field().is_empty()
        ));
    } else {
        panic!("invalid pdu type");
    }

    Ok(())
}

#[test]
fn can_read_write_pdata() -> Result<(), Box<dyn std::error::Error>> {
    let pdata_rq = Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: 3,
            value_type: PDataValueType::Command,
            is_last: true,
            data: vec![0, 0, 0, 0],
        }],
    };

    let mut bytes = Vec::new();
    write_pdu(&mut bytes, &pdata_rq)?;

    let result = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU, true)?.unwrap();

    if let Pdu::PData { data } = result {
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].presentation_context_id, 3);
        assert!(matches!(data[0].value_type, PDataValueType::Command));
        assert!(data[0].is_last);
        assert_eq!(data[0].data, vec![0, 0, 0, 0])
    } else {
        panic!("invalid pdu type");
    }

    Ok(())
}

mod pdu_length_validation {
    use super::{
        read_pdu, write_pdu, AssociationRQ, Cursor, Pdu, PresentationContextProposed,
        UserVariableItem, DEFAULT_MAX_PDU,
    };
    use dicom_ul::pdu::{
        AbortRQSource, AssociationRJ, AssociationRJResult, AssociationRJServiceUserReason,
        AssociationRJSource, ReadError, WriteChunkError, WriteError,
    };

    #[test]
    fn fixed_length_pdu_declared_lengths_are_validated() -> Result<(), Box<dyn std::error::Error>> {
        let fixed_pdus = [
            (
                0x03,
                [0, 1, 1, 1],
                Pdu::AssociationRJ(AssociationRJ {
                    result: AssociationRJResult::Permanent,
                    source: AssociationRJSource::ServiceUser(
                        AssociationRJServiceUserReason::NoReasonGiven,
                    ),
                }),
            ),
            (0x05, [0, 0, 0, 0], Pdu::ReleaseRQ),
            (0x06, [0, 0, 0, 0], Pdu::ReleaseRP),
            (
                0x07,
                [0, 0, 0, 0],
                Pdu::AbortRQ {
                    source: AbortRQSource::ServiceUser,
                },
            ),
        ];

        for (pdu_type, valid_body, expected_pdu) in fixed_pdus {
            let bytes = pdu_bytes(pdu_type, 4, &valid_body);
            let result = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU, true)?.unwrap();
            assert_eq!(result, expected_pdu);

            for invalid_length in [3, 5] {
                let body = if invalid_length == 3 {
                    valid_body[..3].to_vec()
                } else {
                    let mut body = valid_body.to_vec();
                    body.push(0);
                    body
                };
                let bytes = pdu_bytes(pdu_type, invalid_length, &body);
                let error = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU, true).unwrap_err();

                assert!(matches!(
                    error,
                    ReadError::InvalidFixedPduLength {
                        pdu_type: actual_pdu_type,
                        pdu_length,
                        expected_length,
                    } if actual_pdu_type == pdu_type
                        && pdu_length == invalid_length
                        && expected_length == 4
                ));
            }
        }

        Ok(())
    }

    #[test]
    fn write_pdu_rejects_oversized_u16_chunk_payload() {
        let pdu = Pdu::AssociationRQ(AssociationRQ {
            protocol_version: 1,
            calling_ae_title: "CALLING".to_string(),
            called_ae_title: "CALLED".to_string(),
            application_context_name: "1.2.840.10008.3.1.1.1".to_string(),
            presentation_contexts: vec![PresentationContextProposed {
                id: 1,
                abstract_syntax: "1.2.840.10008.1.1".to_string(),
                transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            }],
            user_variables: vec![UserVariableItem::Unknown(0xff, vec![0; 65_536])],
        });

        let mut bytes = Vec::new();
        let error = write_pdu(&mut bytes, &pdu).unwrap_err();

        assert!(contains_encoded_length_overflow(&error));
    }

    fn pdu_bytes(pdu_type: u8, pdu_length: u32, body: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(6 + body.len());
        bytes.push(pdu_type);
        bytes.push(0);
        bytes.extend(pdu_length.to_be_bytes());
        bytes.extend(body);
        bytes
    }

    fn contains_encoded_length_overflow(error: &WriteError) -> bool {
        match error {
            WriteError::EncodedLengthOverflow {
                length,
                maximum_length,
                length_field,
            } => *length == 65_536 && *maximum_length == 65_535 && *length_field == "u16",
            WriteError::WriteChunk { source, .. } => match source {
                WriteChunkError::BuildChunk { source } => contains_encoded_length_overflow(source),
                _ => false,
            },
            _ => false,
        }
    }
}
