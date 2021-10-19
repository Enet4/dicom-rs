use dicom::core::smallvec;
use dicom::{core::dicom_value, dictionary_std::tags};
use dicom::{
    core::{DataElement, PrimitiveValue, VR},
    encoding::transfer_syntax,
    object::{mem::InMemDicomObject, open_file, StandardDataDictionary},
    transfer_syntax::TransferSyntaxRegistry,
};
use dicom_ul::pdu::Pdu;
use dicom_ul::{
    association::ClientAssociationOptions,
    pdu::{PDataValue, PDataValueType},
};
use smallvec::smallvec;
use std::io::Write;
use std::path::PathBuf;
use structopt::StructOpt;
use transfer_syntax::TransferSyntaxIndex;

/// DICOM C-STORE SCU
#[derive(Debug, StructOpt)]
struct App {
    /// socket address to STORE SCP (example: "127.0.0.1:104")
    addr: String,
    /// the DICOM file to store
    file: PathBuf,
    /// verbose mode
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
    /// the C-STORE message ID
    #[structopt(short = "m", long = "message-id", default_value = "1")]
    message_id: u16,
    /// the calling AE title
    #[structopt(long = "calling-ae-title", default_value = "STORE-SCU")]
    calling_ae_title: String,
    /// the called AE title
    #[structopt(long = "called-ae-title", default_value = "ANY-SCP")]
    called_ae_title: String,
    /// the maximum PDU length accepted by the SCU
    #[structopt(long = "max-pdu-length", default_value = "16384")]
    max_pdu_length: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let App {
        addr,
        file,
        verbose,
        message_id,
        calling_ae_title,
        called_ae_title,
        max_pdu_length,
    } = App::from_args();

    if verbose {
        println!("Opening file '{}'...", file.display());
    }

    let dicom_file = open_file(file)?;

    let meta = dicom_file.meta();

    let storage_sop_class_uid = &meta.media_storage_sop_class_uid;
    let storage_sop_instance_uid = &meta.media_storage_sop_instance_uid;
    let transfer_syntax = &meta.transfer_syntax.trim_end_matches('\0');

    let file_ts_desc = TransferSyntaxRegistry
        .get(transfer_syntax)
        .ok_or("Unsupported file transfer syntax")?;

    if verbose {
        println!("Establishing association with '{}'...", &addr);
    }

    let mut scu = ClientAssociationOptions::new()
        .with_abstract_syntax(storage_sop_class_uid.to_string())
        .with_transfer_syntax(transfer_syntax.to_string())
        .calling_ae_title(calling_ae_title)
        .called_ae_title(called_ae_title)
        .max_pdu_length(max_pdu_length)
        .establish(addr)?;

    if verbose {
        println!("Association established");
    }

    // TODO(#106) transfer syntax conversion is currently not supported
    let pc_selected = if let Some(pc) = scu.presentation_contexts().iter().find(|pc| {
        // Check support for this transfer syntax.
        // If it is the same as the file, we're good.
        // Otherwise, uncompressed data set encoding
        // and native pixel data is required on both ends.
        let ts = &pc.transfer_syntax;
        ts == transfer_syntax
            || TransferSyntaxRegistry
                .get(&pc.transfer_syntax)
                .filter(|ts| file_ts_desc.is_codec_free() && ts.is_codec_free())
                .map(|_| true)
                .unwrap_or(false)
    }) {
        pc.clone()
    } else {
        eprintln!("Could not choose a transfer syntax");
        let _ = scu.abort();
        std::process::exit(-2);
    };

    let ts = TransferSyntaxRegistry
        .get(&pc_selected.transfer_syntax)
        .ok_or("Poorly negotiated transfer syntax")?;

    if verbose {
        println!("Transfer Syntax: {}", ts.name());
    }

    let cmd = store_req_command(
        storage_sop_class_uid,
        storage_sop_instance_uid,
        message_id,
    );

    let mut cmd_data = Vec::with_capacity(128);
    cmd.write_dataset_with_ts(
        &mut cmd_data,
        &dicom::transfer_syntax::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
    )?;

    let mut object_data = Vec::with_capacity(2048);
    dicom_file.write_dataset_with_ts(&mut object_data, ts)?;

    let nbytes = cmd_data.len() + object_data.len();

    if verbose {
        println!("Sending payload (~ {} kB)...", nbytes / 1_000);
    }

    if nbytes < scu.acceptor_max_pdu_length() as usize - 100 {
        let pdu = Pdu::PData {
            data: vec![
                PDataValue {
                    presentation_context_id: pc_selected.id,
                    value_type: PDataValueType::Command,
                    is_last: true,
                    data: cmd_data,
                },
                PDataValue {
                    presentation_context_id: pc_selected.id,
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
                presentation_context_id: pc_selected.id,
                value_type: PDataValueType::Command,
                is_last: true,
                data: cmd_data,
            }],
        };

        scu.send(&pdu)?;

        {
            let mut pdata = scu.send_pdata(pc_selected.id);
            pdata.write_all(&object_data)?;
        }
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
            let status = cmd_obj.element(tags::STATUS)?.to_int::<u16>()?;
            let storage_sop_instance_uid =
                storage_sop_instance_uid.trim_end_matches(|c: char| c.is_whitespace() || c == '\0');
            if status == 0 {
                println!("Sucessfully stored instance `{}`", storage_sop_instance_uid);
            } else {
                println!(
                    "Failed to store instance `{}` (status code {})",
                    storage_sop_instance_uid, status
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
            let _ = scu.abort();
            std::process::exit(-2);
        }
    }

    Ok(())
}

fn store_req_command(
    storage_sop_class_uid: &str,
    storage_sop_instance_uid: &str,
    message_id: u16,
) -> InMemDicomObject<StandardDataDictionary> {
    let mut obj = InMemDicomObject::create_empty();

    // group length
    obj.put(DataElement::new(
        tags::COMMAND_GROUP_LENGTH,
        VR::UL,
        PrimitiveValue::from(
            12 + 8
                + even_len(storage_sop_class_uid.len())
                + 10
                + 10
                + 10
                + 10
                + 8
                + even_len(storage_sop_instance_uid.len()),
        ),
    ));

    // SOP Class UID
    obj.put(DataElement::new(
        tags::AFFECTED_SOP_CLASS_UID,
        VR::UI,
        dicom_value!(Str, storage_sop_class_uid),
    ));

    // command field
    obj.put(DataElement::new(
        tags::COMMAND_FIELD,
        VR::US,
        dicom_value!(U16, [0x0001]),
    ));

    // message ID
    obj.put(DataElement::new(
        tags::MESSAGE_ID,
        VR::US,
        dicom_value!(U16, [message_id]),
    ));

    //priority
    obj.put(DataElement::new(
        tags::PRIORITY,
        VR::US,
        dicom_value!(U16, [0x0000]),
    ));

    // data set type
    obj.put(DataElement::new(
        tags::COMMAND_DATA_SET_TYPE,
        VR::US,
        dicom_value!(U16, [0x0000]),
    ));

    // affected SOP Instance UID
    obj.put(DataElement::new(
        tags::AFFECTED_SOP_INSTANCE_UID,
        VR::UI,
        dicom_value!(Str, storage_sop_instance_uid),
    ));

    obj
}

fn even_len(l: usize) -> u32 {
    ((l + 1) & !1) as u32
}

#[cfg(test)]
mod tests {
    use super::even_len;
    #[test]
    fn test_even_len() {
        assert_eq!(even_len(0), 0);
        assert_eq!(even_len(1), 2);
        assert_eq!(even_len(2), 2);
        assert_eq!(even_len(3), 4);
        assert_eq!(even_len(4), 4);
        assert_eq!(even_len(5), 6);
    }
}
