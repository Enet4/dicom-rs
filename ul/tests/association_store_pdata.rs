//! Test suite for sending and receiving data
//! using `send_pdata` and `receive_pdata`.
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::{
    tags,
    uids::{self, SECONDARY_CAPTURE_IMAGE_STORAGE},
};
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN;
use dicom_ul::{
    association::{client::ClientAssociationOptions, Association, SyncAssociation},
    pdu::{
        PDataValue, PDataValueType, Pdu, PresentationContextNegotiated,
        PresentationContextResultReason,
    },
    ServerAssociation,
};
use std::{io::Write as _, net::SocketAddr};

use dicom_ul::association::server::ServerAssociationOptions;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

static SCU_AE_TITLE: &str = "STORE-SCU";
static SCP_AE_TITLE: &str = "STORE-SCP";

static IMPLICIT_VR_LE: &str = uids::IMPLICIT_VR_LITTLE_ENDIAN;
static SC_IMAGE_STORAGE: &str = uids::SECONDARY_CAPTURE_IMAGE_STORAGE;

/// Create a store SCP which accepts one C-STORE interaction
fn spawn_store_scp() -> Result<(
    std::thread::JoinHandle<Result<ServerAssociation<std::net::TcpStream>>>,
    SocketAddr,
)> {
    let listener = std::net::TcpListener::bind("localhost:0")?;
    let addr = listener.local_addr()?;
    let scp = ServerAssociationOptions::new()
        .accept_called_ae_title()
        .ae_title(SCP_AE_TITLE)
        .with_abstract_syntax(SC_IMAGE_STORAGE);

    let h = std::thread::spawn(move || -> Result<_> {
        let (stream, _addr) = listener.accept()?;
        let mut association = scp.establish(stream)?;

        assert_eq!(
            association.presentation_contexts(),
            &[PresentationContextNegotiated {
                id: 1,
                reason: PresentationContextResultReason::Acceptance,
                abstract_syntax: SC_IMAGE_STORAGE.to_string(),
                transfer_syntax: IMPLICIT_VR_LE.to_string(),
            }],
        );

        // handle a full C-STORE-RQ interaction

        let Pdu::PData { data } = association.receive()? else {
            panic!("Unexpected PDU type");
        };

        assert!(data[0].is_last);
        let cstore_cmd = from_bytes_implicit_vr_le(&data[0].data);

        let message_id: u16 = cstore_cmd.get(tags::MESSAGE_ID).unwrap().to_int().unwrap();
        let affected_sop = cstore_cmd
            .get(tags::AFFECTED_SOP_INSTANCE_UID)
            .unwrap()
            .to_str()
            .unwrap();

        // receive and accumulate PData for the C-STORE main data set

        let mut dcm_data = Vec::new();
        {
            let mut pdata = association.receive_pdata();
            std::io::copy(&mut pdata, &mut dcm_data).unwrap();
        }

        // inspect some attributes to validate that it's the expected object
        let dcm_obj = from_bytes_implicit_vr_le(dcm_data);
        assert_eq!(
            dcm_obj.get(tags::SOP_CLASS_UID).unwrap().to_str().unwrap(),
            SECONDARY_CAPTURE_IMAGE_STORAGE,
        );
        assert_eq!(
            dcm_obj
                .get(tags::SOP_INSTANCE_UID)
                .unwrap()
                .to_str()
                .unwrap(),
            affected_sop,
        );
        assert_eq!(
            dcm_obj.get(tags::ROWS).unwrap().to_int::<u16>().unwrap(),
            300,
        );

        // send a C-STORE-RSP
        let cmd = create_cstore_response(message_id, SC_IMAGE_STORAGE, &affected_sop);
        let pdu = Pdu::PData {
            data: vec![PDataValue {
                presentation_context_id: 1,
                value_type: PDataValueType::Command,
                is_last: true,
                data: to_bytes_implicit_vr_le(&cmd),
            }],
        };
        association.send(&pdu).unwrap();

        // handle one release request
        let pdu = association.receive()?;
        assert_eq!(pdu, Pdu::ReleaseRQ);
        association.send(&Pdu::ReleaseRP)?;

        Ok(association)
    });
    Ok((h, addr))
}

/// Run an SCP and an SCU concurrently,
/// negotiate an association,
/// make a single C-STORE interaction,
/// and release the association.
#[test]
fn store_scu_scp_association_test() {
    let (scp_handle, scp_addr) = spawn_store_scp().unwrap();

    let mut association = ClientAssociationOptions::new()
        .calling_ae_title(SCU_AE_TITLE)
        .called_ae_title(SCP_AE_TITLE)
        // secondary capture, implicit VR LE
        .with_presentation_context(SECONDARY_CAPTURE_IMAGE_STORAGE, vec![IMPLICIT_VR_LE])
        .establish(scp_addr)
        .unwrap();

    assert_eq!(
        association.presentation_contexts(),
        &[PresentationContextNegotiated {
            id: 1,
            abstract_syntax: SECONDARY_CAPTURE_IMAGE_STORAGE.to_string(),
            transfer_syntax: IMPLICIT_VR_LE.to_string(),
            reason: PresentationContextResultReason::Acceptance,
        }]
    );

    // send a store SCU in multiple PDUs (1 command + N data)
    let iuid = "2.25.74320942257366560001029850331948705672";

    // build DICOM dataset for command
    let cmd_data =
        to_bytes_implicit_vr_le(&store_req_command(SECONDARY_CAPTURE_IMAGE_STORAGE, iuid, 1));

    let pdu = Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: 1,
            value_type: PDataValueType::Command,
            is_last: true,
            data: cmd_data,
        }],
    };
    association.send(&pdu).unwrap();

    // send

    let obj = InMemDicomObject::from_element_iter([
        DataElement::new(tags::SOP_CLASS_UID, VR::UI, SECONDARY_CAPTURE_IMAGE_STORAGE),
        DataElement::new(tags::SOP_INSTANCE_UID, VR::UI, iuid),
        DataElement::new(
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            "2.25.272620270218608159498737797752592743030",
        ),
        DataElement::new(
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            "2.25.325162285992071091624723217127749500558",
        ),
        DataElement::new(tags::SAMPLES_PER_PIXEL, VR::US, PrimitiveValue::from(3_u16)),
        DataElement::new(tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "RGB"),
        DataElement::new(tags::ROWS, VR::US, PrimitiveValue::from(300_u16)),
        DataElement::new(tags::COLUMNS, VR::US, PrimitiveValue::from(400_u16)),
        DataElement::new(tags::BITS_ALLOCATED, VR::US, PrimitiveValue::from(8_u16)),
        DataElement::new(tags::BITS_STORED, VR::US, PrimitiveValue::from(8_u16)),
        DataElement::new(tags::HIGH_BIT, VR::US, PrimitiveValue::from(8_u16)),
        DataElement::new(
            tags::PIXEL_DATA,
            VR::OW,
            PrimitiveValue::U8(vec![0x5c_u8; 400 * 300 * 3].into()),
        ),
    ]);
    let obj_data = to_bytes_implicit_vr_le(&obj);

    {
        let mut pdata = association.send_pdata(1);
        pdata.write_all(&obj_data).unwrap();
    }

    // expect 1 PDU (C-STORE-RSP)

    let pdu = association.receive().unwrap();

    let Pdu::PData { data: pdata } = pdu else {
        panic!("Unexpected PDU type")
    };

    let pdv = &pdata[0];

    // check PData
    assert!(matches!(
        &pdv,
        PDataValue {
            presentation_context_id: 1,
            value_type: PDataValueType::Command,
            is_last: true,
            ..
        }
    ));

    let rsp = from_bytes_implicit_vr_le(&pdv.data);
    assert_eq!(
        rsp.get(tags::AFFECTED_SOP_INSTANCE_UID)
            .unwrap()
            .to_str()
            .unwrap(),
        iuid,
    );

    assert_eq!(
        rsp.get(tags::STATUS).unwrap().to_int::<u16>().unwrap(),
        0x0000
    );

    // release

    association
        .release()
        .expect("did not have a peaceful release");

    scp_handle
        .join()
        .expect("SCP panicked")
        .expect("Error at the SCP");
}

/// Write a DICOM object into a new vector and return it
fn to_bytes_implicit_vr_le(obj: &InMemDicomObject) -> Vec<u8> {
    let mut cmd_data = Vec::new();
    obj.write_dataset_with_ts(&mut cmd_data, &IMPLICIT_VR_LITTLE_ENDIAN.erased())
        .unwrap();
    cmd_data
}

/// Read a DICOM object from a byte slice
fn from_bytes_implicit_vr_le(dicom_data: impl AsRef<[u8]>) -> InMemDicomObject {
    InMemDicomObject::read_dataset_with_ts(dicom_data.as_ref(), &IMPLICIT_VR_LITTLE_ENDIAN.erased())
        .unwrap()
}

/// build a data set for the C-STORE-RQ command
fn store_req_command(
    storage_sop_class_uid: &str,
    storage_sop_instance_uid: &str,
    message_id: u16,
) -> InMemDicomObject {
    InMemDicomObject::command_from_element_iter([
        // SOP Class UID
        DataElement::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, storage_sop_class_uid),
        // command field
        DataElement::new(tags::COMMAND_FIELD, VR::US, PrimitiveValue::from(0x0001)),
        // message ID
        DataElement::new(tags::MESSAGE_ID, VR::US, PrimitiveValue::from(message_id)),
        //priority
        DataElement::new(tags::PRIORITY, VR::US, PrimitiveValue::from(0x0000)),
        // data set type
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            PrimitiveValue::from(0x0000),
        ),
        // affected SOP Instance UID
        DataElement::new(
            tags::AFFECTED_SOP_INSTANCE_UID,
            VR::UI,
            storage_sop_instance_uid,
        ),
    ])
}

/// build a data set for the C-STORE-RSP command
fn create_cstore_response(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
) -> InMemDicomObject {
    InMemDicomObject::command_from_element_iter([
        DataElement::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, sop_class_uid),
        DataElement::new(tags::COMMAND_FIELD, VR::US, PrimitiveValue::from(0x8001)),
        DataElement::new(
            tags::MESSAGE_ID_BEING_RESPONDED_TO,
            VR::US,
            PrimitiveValue::from(message_id),
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            PrimitiveValue::from(0x0101),
        ),
        DataElement::new(tags::STATUS, VR::US, PrimitiveValue::from(0x0000)),
        DataElement::new(tags::AFFECTED_SOP_INSTANCE_UID, VR::UI, sop_instance_uid),
    ])
}
