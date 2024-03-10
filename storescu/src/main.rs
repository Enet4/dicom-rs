use clap::Parser;
use dicom_core::{dicom_value, header::Tag, DataElement, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_encoding::transfer_syntax;
use dicom_encoding::TransferSyntax;
use dicom_object::{mem::InMemDicomObject, open_file, DefaultDicomObject, StandardDataDictionary};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::{
    association::ClientAssociationOptions,
    pdu::{PDataValue, PDataValueType, Pdu},
};
use indicatif::{ProgressBar, ProgressStyle};
use snafu::prelude::*;
use snafu::{Report, Whatever};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, error, info, warn, Level};
use transfer_syntax::TransferSyntaxIndex;
use walkdir::WalkDir;

/// DICOM C-STORE SCU
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// socket address to Store SCP,
    /// optionally with AE title
    /// (example: "STORE-SCP@127.0.0.1:104")
    addr: String,
    /// the DICOM file(s) to store
    #[arg(required = true)]
    files: Vec<PathBuf>,
    /// verbose mode
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
    /// the C-STORE message ID
    #[arg(short = 'm', long = "message-id", default_value = "1")]
    message_id: u16,
    /// the calling Application Entity title
    #[arg(long = "calling-ae-title", default_value = "STORE-SCU")]
    calling_ae_title: String,
    /// the called Application Entity title,
    /// overrides AE title in address if present [default: ANY-SCP]
    #[arg(long = "called-ae-title")]
    called_ae_title: Option<String>,
    /// the maximum PDU length accepted by the SCU
    #[arg(long = "max-pdu-length", default_value = "16384")]
    max_pdu_length: u32,
    /// fail if not all DICOM files can be transferred
    #[arg(long = "fail-first")]
    fail_first: bool,
    /// fail file transfer if it cannot be done without transcoding
    #[arg(long("never-transcode"))]
    // hide option if transcoding is disabled
    #[cfg_attr(not(feature = "transcode"), arg(hide(true)))]
    never_transcode: bool,
}

struct DicomFile {
    /// File path
    file: PathBuf,
    /// Storage SOP Class UID
    sop_class_uid: String,
    /// Storage SOP Instance UID
    sop_instance_uid: String,
    /// File Transfer Syntax
    file_transfer_syntax: String,
    /// Transfer Syntax selected
    ts_selected: Option<String>,
    /// Presentation Context selected
    pc_selected: Option<dicom_ul::pdu::PresentationContextResult>,
}

#[derive(Debug, Snafu)]
enum Error {
    /// Could not initialize SCU
    InitScu {
        source: dicom_ul::association::client::Error,
    },

    /// Could not construct DICOM command
    CreateCommand { source: Box<dicom_object::WriteError> },

    /// Unsupported file transfer syntax {uid}
    UnsupportedFileTransferSyntax { uid: std::borrow::Cow<'static, str> },

    #[snafu(whatever, display("{}", message))]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error + 'static>, Some)))]
        source: Option<Box<dyn std::error::Error + 'static>>,
    },
}

fn main() {
    run().unwrap_or_else(|e| {
        error!("{}", Report::from_error(e));
        std::process::exit(-2);
    });
}

fn run() -> Result<(), Error> {
    let App {
        addr,
        files,
        verbose,
        message_id,
        calling_ae_title,
        called_ae_title,
        max_pdu_length,
        fail_first,
        mut never_transcode,
    } = App::parse();

    // never transcode if the feature is disabled
    if cfg!(not(feature = "transcode")) {
        never_transcode = true;
    }

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
            .finish(),
    )
    .whatever_context("Could not set up global logging subscriber")
    .unwrap_or_else(|e: Whatever| {
        eprintln!("[ERROR] {}", Report::from_error(e));
    });

    let mut checked_files: Vec<PathBuf> = vec![];
    let mut dicom_files: Vec<DicomFile> = vec![];
    let mut presentation_contexts = HashSet::new();

    for file in files {
        if file.is_dir() {
            for file in WalkDir::new(file.as_path())
                .into_iter()
                .filter_map(Result::ok)
                .filter(|f| !f.file_type().is_dir())
            {
                checked_files.push(file.into_path());
            }
        } else {
            checked_files.push(file);
        }
    }

    for file in checked_files {
        if verbose {
            info!("Opening file '{}'...", file.display());
        }

        match check_file(&file) {
            Ok(dicom_file) => {
                presentation_contexts.insert((
                    dicom_file.sop_class_uid.to_string(),
                    dicom_file.file_transfer_syntax.clone(),
                ));

                // also accept uncompressed transfer syntaxes
                // as mandated by the standard
                // (though it might not always be able to fulfill this)
                if !never_transcode {
                    presentation_contexts.insert((
                        dicom_file.sop_class_uid.to_string(),
                        uids::EXPLICIT_VR_LITTLE_ENDIAN.to_string(),
                    ));
                    presentation_contexts.insert((
                        dicom_file.sop_class_uid.to_string(),
                        uids::IMPLICIT_VR_LITTLE_ENDIAN.to_string(),
                    ));
                }

                dicom_files.push(dicom_file);
            }
            Err(_) => {
                warn!("Could not open file {} as DICOM", file.display());
            }
        }
    }

    if dicom_files.is_empty() {
        eprintln!("No supported files to transfer");
        std::process::exit(-1);
    }

    if verbose {
        info!("Establishing association with '{}'...", &addr);
    }

    let mut scu_init = ClientAssociationOptions::new()
        .calling_ae_title(calling_ae_title)
        .max_pdu_length(max_pdu_length);

    for (storage_sop_class_uid, transfer_syntax) in &presentation_contexts {
        scu_init = scu_init.with_presentation_context(storage_sop_class_uid, vec![transfer_syntax]);
    }

    if let Some(called_ae_title) = called_ae_title {
        scu_init = scu_init.called_ae_title(called_ae_title);
    }

    let mut scu = scu_init.establish_with(&addr).context(InitScuSnafu)?;

    if verbose {
        info!("Association established");
    }

    for file in &mut dicom_files {
        // identify the right transfer syntax to use
        let r: Result<_, Error> =
            check_presentation_contexts(file, scu.presentation_contexts(), never_transcode)
                .whatever_context::<_, _>("Could not choose a transfer syntax");
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
    }

    let progress_bar;
    if !verbose {
        progress_bar = Some(ProgressBar::new(dicom_files.len() as u64));
        if let Some(pb) = progress_bar.as_ref() {
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40} {pos}/{len} {wide_msg}")
                    .expect("Invalid progress bar template"),
            );
            pb.enable_steady_tick(Duration::new(0, 480_000_000));
        };
    } else {
        progress_bar = None;
    }

    for file in dicom_files {
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
                .with_context(|| UnsupportedFileTransferSyntaxSnafu { uid: ts_uid_selected.to_string() })?;

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
                        &dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                            .erased(),
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
    }

    if let Some(pb) = progress_bar {
        pb.finish_with_message("done")
    };

    scu.release()
        .whatever_context("Failed to release SCU association")?;
    Ok(())
}

fn store_req_command(
    storage_sop_class_uid: &str,
    storage_sop_instance_uid: &str,
    message_id: u16,
) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        // SOP Class UID
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, storage_sop_class_uid),
        ),
        // command field
        DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x0001])),
        // message ID
        DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [message_id])),
        //priority
        DataElement::new(tags::PRIORITY, VR::US, dicom_value!(U16, [0x0000])),
        // data set type
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0000]),
        ),
        // affected SOP Instance UID
        DataElement::new(
            tags::AFFECTED_SOP_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, storage_sop_instance_uid),
        ),
    ])
}

fn check_file(file: &Path) -> Result<DicomFile, Error> {
    // Ignore DICOMDIR files until better support is added
    let _ = (file.file_name() != Some(OsStr::new("DICOMDIR")))
        .then_some(false)
        .whatever_context("DICOMDIR file not supported")?;
    let dicom_file = dicom_object::OpenFileOptions::new()
        .read_until(Tag(0x0001, 0x000))
        .open_file(file)
        .with_whatever_context(|_| format!("Could not open DICOM file {}", file.display()))?;

    let meta = dicom_file.meta();

    let storage_sop_class_uid = &meta.media_storage_sop_class_uid;
    let storage_sop_instance_uid = &meta.media_storage_sop_instance_uid;
    let transfer_syntax_uid = &meta.transfer_syntax.trim_end_matches('\0');
    let ts = TransferSyntaxRegistry
        .get(transfer_syntax_uid)
        .with_context(|| UnsupportedFileTransferSyntaxSnafu { uid: transfer_syntax_uid.to_string() })?;
    Ok(DicomFile {
        file: file.to_path_buf(),
        sop_class_uid: storage_sop_class_uid.to_string(),
        sop_instance_uid: storage_sop_instance_uid.to_string(),
        file_transfer_syntax: String::from(ts.uid()),
        ts_selected: None,
        pc_selected: None,
    })
}

fn check_presentation_contexts(
    file: &DicomFile,
    pcs: &[dicom_ul::pdu::PresentationContextResult],
    never_transcode: bool,
) -> Result<(dicom_ul::pdu::PresentationContextResult, String), Error> {
    let file_ts = TransferSyntaxRegistry
        .get(&file.file_transfer_syntax)
        .with_context(|| UnsupportedFileTransferSyntaxSnafu { uid: file.file_transfer_syntax.to_string() })?;
    // if destination does not support original file TS,
    // check whether we can transcode to explicit VR LE

    let pc = pcs.iter().find(|pc| {
        // Check support for this transfer syntax.
        // If it is the same as the file, we're good.
        // Otherwise, uncompressed data set encoding
        // and native pixel data is required on both ends.
        let ts = &pc.transfer_syntax;
        ts == file_ts.uid()
            || TransferSyntaxRegistry
                .get(&pc.transfer_syntax)
                .filter(|ts| file_ts.is_codec_free() && ts.is_codec_free())
                .map(|_| true)
                .unwrap_or(false)
    });

    let pc = match pc {
        Some(pc) => pc,
        None => {
            if never_transcode || !file_ts.can_decode_all() {
                whatever!("No presentation context acceptable");
            }

            // Else, if transcoding is possible, we go for it.
            pcs.iter()
                // accept explicit VR little endian
                .find(|pc| pc.transfer_syntax == uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .or_else(||
                    // accept implicit VR little endian
                    pcs.iter()
                        .find(|pc| pc.transfer_syntax == uids::IMPLICIT_VR_LITTLE_ENDIAN))
                // welp
                .whatever_context("No presentation context acceptable")?
        }
    };
    let ts = TransferSyntaxRegistry
        .get(&pc.transfer_syntax)
        .whatever_context("Poorly negotiated transfer syntax")?;

    Ok((pc.clone(), String::from(ts.uid())))
}


// transcoding functions

#[cfg(feature = "transcode")]
fn into_ts(
    dicom_file: DefaultDicomObject,
    ts_selected: &TransferSyntax,
    verbose: bool,
) -> Result<DefaultDicomObject, Error> {
    if ts_selected.uid() != dicom_file.meta().transfer_syntax() {
        use dicom_pixeldata::Transcode;
        let mut file = dicom_file;
        if verbose {
            info!(
                "Transcoding file from {} to {}",
                file.meta().transfer_syntax(),
                ts_selected.uid()
            );
        }
        file.transcode(ts_selected)
            .whatever_context("Failed to transcode file")?;
        Ok(file)
    } else {
        Ok(dicom_file)
    }
}

#[cfg(not(feature = "transcode"))]
fn into_ts(
    dicom_file: DefaultDicomObject,
    ts_selected: &TransferSyntax,
    _verbose: bool,
) -> Result<DefaultDicomObject, Error> {
    if ts_selected.uid() != dicom_file.meta().transfer_syntax() {
        panic!("Transcoding feature is disabled, should not have tried to transcode")
    } else {
        Ok(dicom_file)
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        App::command().debug_assert();
    }
}
