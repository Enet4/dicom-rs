use dicom_ul::pdu::reader::{read_pdu, DEFAULT_MAX_PDU};
use dicom_ul::pdu::writer::write_pdu;
use dicom_ul::pdu::{PDataValue, PDataValueType, Pdu, PresentationContextProposed, UserVariableItem};
use matches::matches;
use std::io::Cursor;

#[test]
fn can_read_write_associate_rq() -> Result<(), Box<dyn std::error::Error>> {
    let association_rq = Pdu::AssociationRQ {
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
        ],
    };

    let mut bytes = vec![0u8; 0];
    write_pdu(&mut bytes, &association_rq)?;

    let result = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU)?;

    if let Pdu::AssociationRQ {
        protocol_version,
        calling_ae_title,
        called_ae_title,
        application_context_name,
        presentation_contexts,
        user_variables,
    } = result
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
        assert_eq!(user_variables.len(), 3);
        matches!(
            user_variables[0],
            UserVariableItem::ImplementationClassUID(_)
        );
        matches!(
            user_variables[1],
            UserVariableItem::ImplementationVersionName(_)
        );
        matches!(user_variables[2], UserVariableItem::MaxLength(_));
    } else {
        assert!(false, "invalid pdu type");
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

    let result = read_pdu(&mut Cursor::new(&bytes), DEFAULT_MAX_PDU)?;

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
