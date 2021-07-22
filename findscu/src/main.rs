use dicom_core::{dicom_value, smallvec};
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_encoding::transfer_syntax;
use dicom_object::{mem::InMemDicomObject, open_file, StandardDataDictionary};
use dicom_transfer_syntax_registry::{entries, TransferSyntaxRegistry};
use dicom_ul::pdu::Pdu;
use dicom_ul::{
    association::ClientAssociationOptions,
    pdu::{PDataValue, PDataValueType},
};
use smallvec::smallvec;
use std::path::PathBuf;
use structopt::StructOpt;
use transfer_syntax::TransferSyntaxIndex;

/// DICOM C-FIND SCU
#[derive(Debug, StructOpt)]
struct App {
    /// socket address to FIND SCP (example: "127.0.0.1:1045")
    addr: String,
    /// the DICOM file representing the query object
    file: PathBuf,
    /// verbose mode
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
    /// the C-FIND-RQ message ID
    #[structopt(short = "m", long = "message-id", default_value = "1")]
    message_id: u16,
    /// the calling AE title
    #[structopt(long = "calling-ae-title", default_value = "FIND-SCU")]
    calling_ae_title: String,
    /// the called AE title
    #[structopt(long = "called-ae-title", default_value = "ANY-SCP")]
    called_ae_title: String,
    /// the maximum PDU length
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

    // Study Root Query/Retrieve Information Model – FIND
    let abstract_syntax = "1.2.840.10008.5.1.4.1.2.2.1";

    if verbose {
        println!("Establishing association with '{}'...", &addr);
    }

    let mut scu = ClientAssociationOptions::new()
        .with_abstract_syntax(abstract_syntax)
        .calling_ae_title(calling_ae_title)
        .called_ae_title(called_ae_title)
        .max_pdu_length(max_pdu_length)
        .establish(addr)?;

    if verbose {
        println!("Association established");
    }

    let pc_selected = if let Some(pc_selected) = scu.presentation_contexts().first() {
        pc_selected
    } else {
        eprintln!("Could not choose a presentation context");
        let _ = scu.abort();
        std::process::exit(-2);
    };
    let pc_selected_id = pc_selected.id;

    let ts = if let Some(ts) = TransferSyntaxRegistry.get(&pc_selected.transfer_syntax) {
        ts
    } else {
        eprintln!("Poorly negotiated transfer syntax");
        let _ = scu.abort();
        std::process::exit(-2);
    };

    if verbose {
        println!("Transfer Syntax: {}", ts.name());
    }

    let cmd = find_req_command("1.2.840.10008.5.1.4.1.2.2.1\0", message_id);

    let mut cmd_data = Vec::with_capacity(128);
    cmd.write_dataset_with_ts(&mut cmd_data, &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased())?;

    let implicit_vr_le = entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    let mut iod_data = Vec::with_capacity(128);
    dicom_file.write_dataset_with_ts(&mut iod_data, &implicit_vr_le)?;

    let nbytes = cmd_data.len() + iod_data.len();

    if verbose {
        println!("Sending query (~ {} Kb)...", nbytes / 1024);
    }

    let pdu = Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: pc_selected_id,
            value_type: PDataValueType::Command,
            is_last: true,
            data: cmd_data,
        }],
    };
    scu.send(&pdu)?;

    let pdu = Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: pc_selected_id,
            value_type: PDataValueType::Data,
            is_last: true,
            data: iod_data,
        }],
    };
    scu.send(&pdu)?;

    if verbose {
        println!("Awaiting response...");
    }

    loop {
        let rsp_pdu = scu.receive()?;

        match rsp_pdu {
            Pdu::PData { data } => {
                let data_value = &data[0];

                let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                    &data_value.data[..],
                    &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
                )?;
                if verbose {
                    println!("Response: {:?}", cmd_obj);
                }
                let status = cmd_obj.element(tags::STATUS)?.to_int::<u16>()?;
                if status == 0 {
                    if verbose {
                        println!("Matching is complete");
                    }
                    break;
                } else if status == 0xFF00 || status == 0xFF01 {
                    if verbose {
                        println!("Operation pending: {:x}", status);
                    }

                    // fetch DICOM data
                    // !!! does not handle last = false
                    let rsp_iod = scu.receive()?;
                    match rsp_iod {
                        Pdu::PData { data } => {
                            // !!! handle multiple PDataValue's
                            let data = &data[0];
                            let dcm = InMemDicomObject::read_dataset_with_ts(
                                &data.data[..],
                                &implicit_vr_le,
                            )?;

                            println!("> {:?}", dcm);
                        }
                        _ => {
                            eprintln!("Unexpected SCP response: {:?}", pdu);
                            let _ = scu.abort();
                            std::process::exit(-2);
                        }
                    }
                } else {
                    println!("Operation failed (status code {})", status);
                    break;
                }
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
    }
    scu.release()?;

    Ok(())
}

fn find_req_command(
    sop_class_uid: &str,
    message_id: u16,
) -> InMemDicomObject<StandardDataDictionary> {
    let mut obj = InMemDicomObject::new_empty();

    // group length
    obj.put(DataElement::new(
        tags::COMMAND_GROUP_LENGTH,
        VR::UL,
        PrimitiveValue::from(
            8 + even_len(sop_class_uid.len())   // SOP Class UID
            + 8 + 2 // command field
            + 8 + 2 // message ID
            + 8 + 2 // priority
            + 8 + 2, // data set type
        ),
    ));

    // SOP Class UID
    obj.put(DataElement::new(
        tags::AFFECTED_SOP_CLASS_UID,
        VR::UI,
        PrimitiveValue::from(sop_class_uid),
    ));

    // command field
    obj.put(DataElement::new(
        tags::COMMAND_FIELD,
        VR::US,
        // 0020H: C-FIND-RQ message
        dicom_value!(U16, [0x0020]),
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
        // medium
        dicom_value!(U16, [0x0000]),
    ));

    // data set type
    obj.put(DataElement::new(
        tags::COMMAND_DATA_SET_TYPE,
        VR::US,
        dicom_value!(U16, [0x0001]),
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
