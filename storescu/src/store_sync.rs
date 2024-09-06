use std::{collections::HashSet, io::Write, net::TcpStream};

use dicom_dictionary_std::tags;
use dicom_encoding::TransferSyntaxIndex;
use dicom_object::{open_file, InMemDicomObject};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::{
    pdu::{PDataValue, PDataValueType},
    ClientAssociation, ClientAssociationOptions, Pdu,
};
use indicatif::ProgressBar;
use snafu::{OptionExt, ResultExt};
use tracing::{debug, error, info, warn};

use crate::{
    into_ts, store_req_command, CreateCommandSnafu, DicomFile, Error, InitScuSnafu,
    UnsupportedFileTransferSyntaxSnafu,
};

#[allow(clippy::too_many_arguments)]
pub fn get_scu(
    addr: String,
    calling_ae_title: String,
    called_ae_title: Option<String>,
    max_pdu_length: u32,
    username: Option<String>,
    password: Option<String>,
    kerberos_service_ticket: Option<String>,
    saml_assertion: Option<String>,
    jwt: Option<String>,
    presentation_contexts: HashSet<(String, String)>,
) -> Result<ClientAssociation<TcpStream>, Error> {
    let mut scu_init = ClientAssociationOptions::new()
        .calling_ae_title(calling_ae_title)
        .max_pdu_length(max_pdu_length);

    for (storage_sop_class_uid, transfer_syntax) in &presentation_contexts {
        scu_init = scu_init.with_presentation_context(storage_sop_class_uid, vec![transfer_syntax]);
    }

    if let Some(called_ae_title) = called_ae_title {
        scu_init = scu_init.called_ae_title(called_ae_title);
    }

    if let Some(username) = username {
        scu_init = scu_init.username(username);
    }

    if let Some(password) = password {
        scu_init = scu_init.password(password);
    }

    if let Some(kerberos_service_ticket) = kerberos_service_ticket {
        scu_init = scu_init.kerberos_service_ticket(kerberos_service_ticket);
    }

    if let Some(saml_assertion) = saml_assertion {
        scu_init = scu_init.saml_assertion(saml_assertion);
    }

    if let Some(jwt) = jwt {
        scu_init = scu_init.jwt(jwt);
    }

    scu_init.establish_with(&addr).context(InitScuSnafu)
}

pub fn send_file(
    mut scu: ClientAssociation<TcpStream>,
    file: DicomFile,
    message_id: u16,
    progress_bar: Option<&ProgressBar>,
    verbose: bool,
    fail_first: bool,
) -> Result<ClientAssociation<TcpStream>, Error> {
    if let (Some(pc_selected), Some(ts_uid_selected)) = (file.pc_selected, file.ts_selected) {
        if let Some(pb) = &progress_bar {
            pb.set_message(file.sop_instance_uid.clone());
        }
        let cmd = store_req_command(&file.sop_class_uid, &file.sop_instance_uid, message_id);

        let mut cmd_data = Vec::with_capacity(128);
        cmd.write_dataset_with_ts(
            &mut cmd_data,
            &dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
        )
        .map_err(Box::from)
        .context(CreateCommandSnafu)?;

        let mut object_data = Vec::with_capacity(2048);
        let dicom_file =
            open_file(&file.file).whatever_context("Could not open listed DICOM file")?;
        let ts_selected = TransferSyntaxRegistry
            .get(&ts_uid_selected)
            .with_context(|| UnsupportedFileTransferSyntaxSnafu {
                uid: ts_uid_selected.to_string(),
            })?;

        // transcode file if necessary
        let dicom_file = into_ts(dicom_file, ts_selected, verbose)?;

        dicom_file
            .write_dataset_with_ts(&mut object_data, ts_selected)
            .whatever_context("Could not write object dataset")?;

        let nbytes = cmd_data.len() + object_data.len();

        if verbose {
            info!(
                "Sending file {} (~ {} kB), uid={}, sop={}, ts={}",
                file.file.display(),
                nbytes / 1_000,
                &file.sop_instance_uid,
                &file.sop_class_uid,
                ts_uid_selected,
            );
        }

        if nbytes < scu.acceptor_max_pdu_length().saturating_sub(100) as usize {
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

            scu.send(&pdu)
                .whatever_context("Failed to send C-STORE-RQ")?;
        } else {
            let pdu = Pdu::PData {
                data: vec![PDataValue {
                    presentation_context_id: pc_selected.id,
                    value_type: PDataValueType::Command,
                    is_last: true,
                    data: cmd_data,
                }],
            };

            scu.send(&pdu)
                .whatever_context("Failed to send C-STORE-RQ command")?;

            {
                let mut pdata = scu.send_pdata(pc_selected.id);
                pdata
                    .write_all(&object_data)
                    .whatever_context("Failed to send C-STORE-RQ P-Data")?;
            }
        }

        if verbose {
            debug!("Awaiting response...");
        }

        let rsp_pdu = scu
            .receive()
            .whatever_context("Failed to receive C-STORE-RSP")?;

        match rsp_pdu {
            Pdu::PData { data } => {
                let data_value = &data[0];

                let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                    &data_value.data[..],
                    &dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
                )
                .whatever_context("Could not read response from SCP")?;
                if verbose {
                    debug!("Full response: {:?}", cmd_obj);
                }
                let status = cmd_obj
                    .element(tags::STATUS)
                    .whatever_context("Could not find status code in response")?
                    .to_int::<u16>()
                    .whatever_context("Status code in response is not a valid integer")?;
                let storage_sop_instance_uid = file
                    .sop_instance_uid
                    .trim_end_matches(|c: char| c.is_whitespace() || c == '\0');

                match status {
                    // Success
                    0 => {
                        if verbose {
                            info!("Successfully stored instance {}", storage_sop_instance_uid);
                        }
                    }
                    // Warning
                    1 | 0x0107 | 0x0116 | 0xB000..=0xBFFF => {
                        warn!(
                            "Possible issue storing instance `{}` (status code {:04X}H)",
                            storage_sop_instance_uid, status
                        );
                    }
                    0xFF00 | 0xFF01 => {
                        warn!(
                            "Possible issue storing instance `{}`: status is pending (status code {:04X}H)",
                            storage_sop_instance_uid, status
                        );
                    }
                    0xFE00 => {
                        error!(
                            "Could not store instance `{}`: operation cancelled",
                            storage_sop_instance_uid
                        );
                        if fail_first {
                            let _ = scu.abort();
                            std::process::exit(-2);
                        }
                    }
                    _ => {
                        error!(
                            "Failed to store instance `{}` (status code {:04X}H)",
                            storage_sop_instance_uid, status
                        );
                        if fail_first {
                            let _ = scu.abort();
                            std::process::exit(-2);
                        }
                    }
                }
            }

            pdu @ Pdu::Unknown { .. }
            | pdu @ Pdu::AssociationRQ { .. }
            | pdu @ Pdu::AssociationAC { .. }
            | pdu @ Pdu::AssociationRJ { .. }
            | pdu @ Pdu::ReleaseRQ
            | pdu @ Pdu::ReleaseRP
            | pdu @ Pdu::AbortRQ { .. } => {
                error!("Unexpected SCP response: {:?}", pdu);
                let _ = scu.abort();
                std::process::exit(-2);
            }
        }
    }
    if let Some(pb) = progress_bar.as_ref() {
        pb.inc(1)
    };
    Ok(scu)
}
