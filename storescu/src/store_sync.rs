use std::{
    io::{stderr, Write},
};

use dicom_dictionary_std::tags;
use dicom_encoding::TransferSyntaxIndex;
use dicom_object::{open_file, InMemDicomObject};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::{
    ClientAssociation, Pdu, association::{Association, CloseSocket, SyncAssociation}, pdu::{PDataValue, PDataValueType}
};
use indicatif::ProgressBar;
use snafu::{OptionExt, Report, ResultExt};
use tracing::{debug, error, info, warn};

use crate::{
    ConvertFieldSnafu, CreateCommandSnafu, DicomFile, Error, MissingAttributeSnafu, ReadDatasetSnafu, ReadFilePathSnafu, ScuSnafu, UnsupportedFileTransferSyntaxSnafu, WriteDatasetSnafu, WriteIOSnafu, check_presentation_contexts, into_ts, store_req_command
};

pub fn send_file<T>(
    mut scu: ClientAssociation<T>,
    file: DicomFile,
    message_id: u16,
    progress_bar: Option<&ProgressBar>,
    verbose: bool,
    fail_first: bool,
) -> Result<ClientAssociation<T>, Error> 
where T: std::io::Read + std::io::Write + CloseSocket{
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
        let dicom_file = open_file(&file.file)
            .map_err(Box::from)
            .context(ReadFilePathSnafu {
                path: file.file.display().to_string(),
            })?;
        let ts_selected = TransferSyntaxRegistry
            .get(&ts_uid_selected)
            .with_context(|| UnsupportedFileTransferSyntaxSnafu {
                uid: ts_uid_selected.to_string(),
            })?;

        // transcode file if necessary
        let dicom_file = into_ts(dicom_file, ts_selected, verbose)?;

        dicom_file
            .write_dataset_with_ts(&mut object_data, ts_selected)
            .map_err(Box::from)
            .context(WriteDatasetSnafu)?;

        let nbytes = cmd_data.len() + object_data.len();

        if verbose {
            info!(
                "Sending file {} (~ {} kB), uid={}, sop={}, ts={}, pc={}",
                file.file.display(),
                nbytes / 1_000,
                &file.sop_instance_uid,
                &file.sop_class_uid,
                ts_uid_selected,
                pc_selected.id,
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

            scu.send(&pdu).map_err(Box::from).context(ScuSnafu)?;
        } else {
            let pdu = Pdu::PData {
                data: vec![PDataValue {
                    presentation_context_id: pc_selected.id,
                    value_type: PDataValueType::Command,
                    is_last: true,
                    data: cmd_data,
                }],
            };

            scu.send(&pdu).map_err(Box::from).context(ScuSnafu)?;

            {
                let mut pdata = scu.send_pdata(pc_selected.id);
                pdata.write_all(&object_data).context(WriteIOSnafu)?;
            }
        }

        if verbose {
            debug!("Awaiting response...");
        }

        let rsp_pdu = scu.receive().map_err(Box::from).context(ScuSnafu)?;

        match rsp_pdu {
            Pdu::PData { data } => {
                let data_value = &data[0];

                let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                    &data_value.data[..],
                    &dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
                )
                .context(ReadDatasetSnafu)?;
                if verbose {
                    debug!("Full response:");
                    let _ = dicom_dump::dump_object_to(stderr(), &cmd_obj);
                }
                let status = cmd_obj
                    .element(tags::STATUS)
                    .context(MissingAttributeSnafu { tag: tags::STATUS })?
                    .to_int::<u16>()
                    .context(ConvertFieldSnafu { tag: tags::STATUS })?;
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


pub fn inner<T>(
    mut scu: ClientAssociation<T>,
    d_files: Vec<DicomFile>,
    pbx: &Option<ProgressBar>,
    fail_first: bool,
    verbose: bool,
    never_transcode: bool,
    ignore_sop_class: bool,
) -> Result<(), Error>
where T: std::io::Read + std::io::Write + CloseSocket{
    let mut message_id = 1;
    for mut file in d_files {
        // identify the right transfer syntax to use
        let r: Result<_, Error> =
            check_presentation_contexts(&file, scu.presentation_contexts(), ignore_sop_class, never_transcode);
        match r {
            Ok((pc, ts)) => {
                if verbose {
                    debug!(
                        "{}: Selected presentation context: {:?}",
                        file.file.display(),
                        pc
                    );
                }
                file.pc_selected = Some(pc);
                file.ts_selected = Some(ts);
            }
            Err(e) => {
                error!("{}", Report::from_error(e));
                if fail_first {
                    let _ = scu.abort();
                    std::process::exit(-2);
                }
            }
        }
        scu = send_file(
            scu,
            file,
            message_id,
            pbx.as_ref(),
            verbose,
            fail_first,
        )?;
        message_id += 1;
    }
    scu.release().map_err(Box::from).context(ScuSnafu)?;
    if let Some(pb) = pbx {
        pb.finish_with_message("done")
    };
    Ok(())
}