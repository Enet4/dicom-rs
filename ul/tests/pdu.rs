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
            user_identity.secondary_field() == []
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
        matches!(data[0].value_type, PDataValueType::Command);
        matches!(data[0].is_last, true);
        assert_eq!(data[0].data, vec![0, 0, 0, 0])
    } else {
        assert!(false, "invalid pdu type");
    }

    Ok(())
}
