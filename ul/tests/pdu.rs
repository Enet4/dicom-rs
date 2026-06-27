use dicom_ul::pdu::reader::read_pdu;
use dicom_ul::pdu::writer::write_pdu;
use dicom_ul::pdu::{
    AssociationRQ, DEFAULT_MAX_PDU, PDataValue, PDataValueType, Pdu, PresentationContextProposed,
    UserIdentity, UserIdentityType, UserVariableItem,
};
use matches::matches;
use std::io::Cursor;

mod ae_title_wire_length_validation {
    use super::*;
    use dicom_ul::pdu::{
        AssociationAC, PresentationContextResult, PresentationContextResultReason,
        WriteChunkError, WriteError,
    };

    fn association_rq_with_ae_titles(called_ae_title: &str, calling_ae_title: &str) -> Pdu {
        AssociationRQ {
            protocol_version: 1,
            calling_ae_title: calling_ae_title.to_string(),
            called_ae_title: called_ae_title.to_string(),
            application_context_name: "1.2.840.10008.3.1.1.1".to_string(),
            presentation_contexts: vec![],
            user_variables: vec![],
        }
        .into()
    }

    fn association_ac_with_ae_titles(called_ae_title: &str, calling_ae_title: &str) -> Pdu {
        AssociationAC {
            protocol_version: 1,
            calling_ae_title: calling_ae_title.to_string(),
            called_ae_title: called_ae_title.to_string(),
            application_context_name: "1.2.840.10008.3.1.1.1".to_string(),
            presentation_contexts: vec![PresentationContextResult {
                id: 1,
                reason: PresentationContextResultReason::Acceptance,
                transfer_syntax: "1.2.840.10008.1.2".to_string(),
            }],
            user_variables: vec![UserVariableItem::MaxLength(16_384)],
        }
        .into()
    }

    fn padded_ae_title(value: &str) -> [u8; 16] {
        let mut bytes = [b' '; 16];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        bytes
    }

    fn assert_invalid_fixed_size_text_field(
        err: WriteError,
        expected_field: &'static str,
        expected_length: usize,
        expected_actual_length: usize,
    ) {
        match err {
            WriteError::InvalidFixedSizeTextField {
                field,
                length,
                actual_length,
                ..
            } => {
                assert_eq!(field, expected_field);
                assert_eq!(length, expected_length);
                assert_eq!(actual_length, expected_actual_length);
            }
            WriteError::WriteChunk { source, .. } => match source {
                WriteChunkError::BuildChunk { source } => assert_invalid_fixed_size_text_field(
                    *source,
                    expected_field,
                    expected_length,
                    expected_actual_length,
                ),
                other => panic!("unexpected chunk error: {other:?}"),
            },
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn associate_rq_writes_16_byte_ae_titles_without_padding() -> Result<(), WriteError> {
        let pdu = association_rq_with_ae_titles("CALLED_AE_123456", "CALLING_AE_12345");

        let mut bytes = Vec::new();
        write_pdu(&mut bytes, &pdu)?;

        assert_eq!(&bytes[10..26], b"CALLED_AE_123456");
        assert_eq!(&bytes[26..42], b"CALLING_AE_12345");

        Ok(())
    }

    #[test]
    fn associate_rq_pads_shorter_ae_titles_to_16_bytes() -> Result<(), WriteError> {
        let pdu = association_rq_with_ae_titles("SCP", "SCU");

        let mut bytes = Vec::new();
        write_pdu(&mut bytes, &pdu)?;

        assert_eq!(&bytes[10..26], &padded_ae_title("SCP"));
        assert_eq!(&bytes[26..42], &padded_ae_title("SCU"));

        Ok(())
    }

    #[test]
    fn associate_rq_rejects_empty_and_all_space_ae_titles() {
        for invalid_ae_title in ["", "   ", "                "] {
            let pdu = association_rq_with_ae_titles(invalid_ae_title, "CALLING");
            let mut bytes = Vec::new();
            let err = write_pdu(&mut bytes, &pdu).unwrap_err();
            assert_invalid_fixed_size_text_field(
                err,
                "Called-AE-title",
                16,
                invalid_ae_title.len(),
            );
            assert_eq!(bytes.as_slice(), &[0x01, 0x00]);

            let pdu = association_rq_with_ae_titles("CALLED", invalid_ae_title);
            let mut bytes = Vec::new();
            let err = write_pdu(&mut bytes, &pdu).unwrap_err();
            assert_invalid_fixed_size_text_field(
                err,
                "Calling-AE-title",
                16,
                invalid_ae_title.len(),
            );
            assert_eq!(bytes.as_slice(), &[0x01, 0x00]);
        }
    }

    #[test]
    fn associate_rq_rejects_17_byte_ae_titles() {
        let invalid_ae_title = "ABCDEFGHIJKLMNOPQ";

        let pdu = association_rq_with_ae_titles(invalid_ae_title, "CALLING");
        let mut bytes = Vec::new();
        let err = write_pdu(&mut bytes, &pdu).unwrap_err();
        assert_invalid_fixed_size_text_field(err, "Called-AE-title", 16, 17);
        assert_eq!(bytes.as_slice(), &[0x01, 0x00]);

        let pdu = association_rq_with_ae_titles("CALLED", invalid_ae_title);
        let mut bytes = Vec::new();
        let err = write_pdu(&mut bytes, &pdu).unwrap_err();
        assert_invalid_fixed_size_text_field(err, "Calling-AE-title", 16, 17);
        assert_eq!(bytes.as_slice(), &[0x01, 0x00]);
    }

    #[test]
    fn associate_ac_rejects_ae_title_fields_longer_than_fixed_length() {
        let invalid_ae_title = "ABCDEFGHIJKLMNOPQ";

        let pdu = association_ac_with_ae_titles(invalid_ae_title, "CALLING");
        let mut bytes = Vec::new();
        let err = write_pdu(&mut bytes, &pdu).unwrap_err();
        assert_invalid_fixed_size_text_field(err, "Called-AE-title", 16, 17);
        assert_eq!(bytes.as_slice(), &[0x02, 0x00]);

        let pdu = association_ac_with_ae_titles("CALLED", invalid_ae_title);
        let mut bytes = Vec::new();
        let err = write_pdu(&mut bytes, &pdu).unwrap_err();
        assert_invalid_fixed_size_text_field(err, "Calling-AE-title", 16, 17);
        assert_eq!(bytes.as_slice(), &[0x02, 0x00]);
    }

    #[test]
    fn associate_ac_allows_reserved_all_space_ae_title_fields() -> Result<(), WriteError> {
        let pdu = association_ac_with_ae_titles("                ", "");

        let mut bytes = Vec::new();
        write_pdu(&mut bytes, &pdu)?;

        assert_eq!(&bytes[10..26], &[b' '; 16]);
        assert_eq!(&bytes[26..42], &[b' '; 16]);

        Ok(())
    }

    #[test]
    fn valid_ae_titles_keep_associate_rq_fields_aligned() -> Result<(), Box<dyn std::error::Error>>
    {
        let association_rq = AssociationRQ {
            protocol_version: 1,
            calling_ae_title: "SCU".to_string(),
            called_ae_title: "CALLED_AE_123456".to_string(),
            application_context_name: "1.2.840.10008.3.1.1.1".to_string(),
            presentation_contexts: vec![PresentationContextProposed {
                id: 1,
                abstract_syntax: "1.2.840.10008.1.1".to_string(),
                transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            }],
            user_variables: vec![UserVariableItem::MaxLength(16_384)],
        };

        let mut bytes = Vec::new();
        write_pdu(&mut bytes, &association_rq.clone().into())?;

        assert_eq!(&bytes[10..26], b"CALLED_AE_123456");
        assert_eq!(&bytes[26..42], &padded_ae_title("SCU"));
        assert_eq!(&bytes[42..74], &[0; 32]);
        assert_eq!(bytes[74], 0x10);

        let result = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU, true)?.unwrap();
        assert_eq!(result, Pdu::AssociationRQ(association_rq));

        Ok(())
    }
}

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
