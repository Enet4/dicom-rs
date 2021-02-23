use dicom::core::dicom_value;
use dicom::core::smallvec;
use dicom::encoding::transfer_syntax::TransferSyntaxIndex;
use dicom::object::open_file;
use dicom::transfer_syntax::TransferSyntaxRegistry;
use dicom::{
    core::{DataElement, Tag, VR},
    object::{mem::InMemDicomObject, StandardDataDictionary},
};
use dicom_ul::pdu::Pdu;
use dicom_ul::{
    association::ClientAssociationOptions,
    pdu::{PDataValue, PDataValueType},
};
use std::io::Write;
use std::path::PathBuf;
use structopt::StructOpt;

/// DICOM C-MOVE SCU
#[derive(Debug, StructOpt)]
struct App {
    /// socket address to MOVE SCP (example: "127.0.0.1:104")
    addr: String,
    /// verbose mode
    #[structopt(short = "v")]
    verbose: bool,
    /// the DICOM file to store
    file: PathBuf,
    /// the C-MOVE destination
    #[structopt(short = "mo", long = "move-destination", default_value = "")]
    move_destination: String,
    /// the C-MOVE message ID
    #[structopt(short = "m", long = "message-id", default_value = "1")]
    message_id: u16,
    /// the calling AE title
    #[structopt(long = "calling-ae-title", default_value = "MOVESCU")]
    calling_ae_title: String,
    /// the called AE title
    #[structopt(long = "called-ae-title", default_value = "ANY-SCP")]
    called_ae_title: String,

    #[structopt(long = "max-pdu-length", default_value = "16384")]
    max_pdu_length: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let App {
        addr,
        verbose,
        file,
        message_id,
        move_destination,
        calling_ae_title,
        called_ae_title,
        max_pdu_length,
    } = App::from_args();

    if verbose {
        println!("Establishing association with '{}'...", &addr);
    }

    let dicom_file = open_file(file)?;
    let meta = dicom_file.meta();

    let affected_sop_class_uid = "1.2.840.10008.5.1.4.1.2.2.2\u{0}";
    let sop_instance_uid = &meta.media_storage_sop_instance_uid;
    let transfer_syntax = "1.2.840.10008.1.2";

    let retrieve_level = "STUDY ";
    let study_instance_uid = dicom_file
        .element_by_name("StudyInstanceUID")?
        .to_clean_str()?;
    let series_instance_uid = dicom_file
        .element_by_name("SeriesInstanceUID")?
        .to_clean_str()?;

    let mut scu = ClientAssociationOptions::new()
        .with_abstract_syntax(affected_sop_class_uid)
        .with_transfer_syntax(transfer_syntax)
        .calling_ae_title(calling_ae_title)
        .called_ae_title(called_ae_title)
        .max_pdu_length(max_pdu_length)
        .establish(addr)?;

    if verbose {
        println!("Association established");
    }

    let ts = TransferSyntaxRegistry
        .get(&transfer_syntax)
        .expect("Poorly negotiated transfer syntax");

    if verbose {
        println!("Transfer Syntax: {}", ts.name());
    };

    let cmd = move_req_command(&affected_sop_class_uid, message_id, &move_destination);

    let mut cmd_data = Vec::with_capacity(128);
    cmd.write_dataset_with_ts(
        &mut cmd_data,
        &dicom::transfer_syntax::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
    )?;

    let obj = create_iod(
        &sop_instance_uid,
        &retrieve_level,
        &study_instance_uid,
        &series_instance_uid,
    );

    let mut object_data = Vec::with_capacity(128);
    obj.write_dataset_with_ts(
        &mut object_data,
        &dicom::transfer_syntax::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
    )?;

    let nbytes = cmd_data.len() + object_data.len();

    if verbose {
        println!("Sending payload (~ {} Kb)...", nbytes / 1024);
    }

    if nbytes < max_pdu_length as usize - 100 {
        let pdu = Pdu::PData {
            data: vec![
                PDataValue {
                    presentation_context_id: scu.presentation_context_id(),
                    value_type: PDataValueType::Command,
                    is_last: true,
                    data: cmd_data,
                },
                PDataValue {
                    presentation_context_id: scu.presentation_context_id(),
                    value_type: PDataValueType::Data,
                    is_last: true,
                    data: object_data,
                },
            ],
        };

        scu.send(&pdu)?;
    } else {
        let pdu = Pdu::PData {
            data: vec![PDataValue {
                presentation_context_id: scu.presentation_context_id(),
                value_type: PDataValueType::Command,
                is_last: true,
                data: cmd_data,
            }],
        };

        scu.send(&pdu)?;

        scu.send_pdata(scu.presentation_context_id())
            .write_all(&object_data)?;
    }

    if verbose {
        println!("Awaiting response...");
    }

    let rsp_pdu = scu.receive()?;

    match rsp_pdu {
        Pdu::PData { data } => {
            let data_value = &data[0];

            let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                &data_value.data[..],
                &dicom::transfer_syntax::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
            )?;

            if verbose {
                println!("Response: {:?}", cmd_obj);
            }

            let status = cmd_obj.element(Tag(0x0000, 0x0900))?.to_int::<u16>()?;
            if status == 0 {
                println!("Sucessfully moved instance '{}'", sop_instance_uid);
            } else {
                println!(
                    "Failed to move instance '{}' (status code {})",
                    sop_instance_uid, status
                );
            }

            scu.release()?;
        }

        pdu @ Pdu::Unknown { .. }
        | pdu @ Pdu::AssociationRQ { .. }
        | pdu @ Pdu::AssociationAC { .. }
        | pdu @ Pdu::AssociationRJ { .. }
        | pdu @ Pdu::ReleaseRQ
        | pdu @ Pdu::ReleaseRP
        | pdu @ Pdu::AbortRQ { .. } => {
            eprintln!("Unexpected SCP response: {:?}", pdu);
            std::process::exit(-2);
        }
    }

    Ok(())
}

fn move_req_command(
    affected_sop_class_uid: &str,
    message_id: u16,
    move_destination: &str,
) -> InMemDicomObject<StandardDataDictionary> {
    let mut obj = InMemDicomObject::create_empty();

    // SOP Class UID
    obj.put(DataElement::new(
        Tag(0x0000, 0x0000),
        VR::UL,
        dicom_value!(U32, [98]),
    ));

    // SOP Class UID
    obj.put(DataElement::new(
        Tag(0x0000, 0x0002),
        VR::UI,
        dicom_value!(Strs, [affected_sop_class_uid]),
    ));

    // command field
    obj.put(DataElement::new(
        Tag(0x0000, 0x0100),
        VR::US,
        dicom_value!(U16, [0x0021]),
    ));

    // message ID
    obj.put(DataElement::new(
        Tag(0x0000, 0x0110),
        VR::US,
        dicom_value!(U16, [message_id]),
    ));

    // move destination
    obj.put(DataElement::new(
        Tag(0x0000, 0x0600),
        VR::AE,
        dicom_value!(Strs, [move_destination]),
    ));

    //priority
    obj.put(DataElement::new(
        Tag(0x0000, 0x0700),
        VR::US,
        dicom_value!(U16, [0x0000]),
    ));

    // data set type
    obj.put(DataElement::new(
        Tag(0x0000, 0x0800),
        VR::US,
        dicom_value!(U16, [0x0000]),
    ));

    obj
}

fn create_iod(
    sop_instance_uid: &str,
    retrieve_level: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
) -> InMemDicomObject<StandardDataDictionary> {
    let mut obj = InMemDicomObject::create_empty();

    // SOP Instance UID
    obj.put(DataElement::new(
        Tag(0x0008, 0x0018),
        VR::UI,
        dicom_value!(Strs, [sop_instance_uid]),
    ));

    // retrieve level
    obj.put(DataElement::new(
        Tag(0x0008, 0x0052),
        VR::CS,
        dicom_value!(Strs, [retrieve_level]),
    ));

    // study instance UID
    obj.put(DataElement::new(
        Tag(0x0020, 0x000D),
        VR::UI,
        dicom_value!(Strs, [study_instance_uid]),
    ));

    // series instance UID
    obj.put(DataElement::new(
        Tag(0x0020, 0x000E),
        VR::UI,
        dicom_value!(Strs, [series_instance_uid]),
    ));

    obj
}
