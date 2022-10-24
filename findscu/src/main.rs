use clap::Parser;
use dicom_core::{dicom_value, smallvec};
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_dump::DumpOptions;
use dicom_encoding::transfer_syntax;
use dicom_object::{mem::InMemDicomObject, open_file, StandardDataDictionary};
use dicom_transfer_syntax_registry::{entries, TransferSyntaxRegistry};
use dicom_ul::pdu::Pdu;
use dicom_ul::{
    association::ClientAssociationOptions,
    pdu::{PDataValue, PDataValueType},
};
use query::parse_queries;
use smallvec::smallvec;
use snafu::prelude::*;
use std::io::{stderr, Read};
use std::path::PathBuf;
use tracing::{debug, error, info, warn, Level};
use transfer_syntax::TransferSyntaxIndex;

mod query;

/// DICOM C-FIND SCU
#[derive(Debug, Parser)]
struct App {
    /// socket address to FIND SCP (example: "127.0.0.1:1045")
    addr: String,
    /// a DICOM file representing the query object
    file: Option<PathBuf>,
    /// sequence of queries
    #[clap(short('q'))]
    query: Vec<String>,

    /// verbose mode
    #[clap(short = 'v', long = "verbose")]
    verbose: bool,
    /// the calling AE title
    #[clap(long = "calling-ae-title", default_value = "FIND-SCU")]
    calling_ae_title: String,
    /// the called AE title
    #[clap(long = "called-ae-title")]
    called_ae_title: Option<String>,
    /// the maximum PDU length
    #[clap(long = "max-pdu-length", default_value = "16384")]
    max_pdu_length: u32,

    /// use patient root information model
    #[clap(short = 'P', long, conflicts_with = "study")]
    patient: bool,
    /// use study root information model (default)
    #[clap(short = 'S', long, conflicts_with = "patient")]
    study: bool,
}

fn main() {
    run().unwrap_or_else(|err| {
        error!("{}", snafu::Report::from_error(err));
        std::process::exit(-2);
    });
}

#[derive(Debug, Snafu)]
enum Error {
    /// Could not initialize SCU
    InitScu {
        source: dicom_ul::association::client::Error,
    },

    /// Could not construct DICOM command
    CreateCommand { source: dicom_object::Error },

    /// Could not read DICOM command
    ReadCommand { source: dicom_object::Error },

    /// Could not dump DICOM output
    DumpOutput { source: std::io::Error },

    #[snafu(whatever, display("{}", message))]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error + 'static>, Some)))]
        source: Option<Box<dyn std::error::Error + 'static>>,
    },
}

fn build_query(
    file: Option<PathBuf>,
    q: Vec<String>,
    patient: bool,
    study: bool,
    verbose: bool,
) -> Result<InMemDicomObject, Error> {
    match (file, q) {
        (Some(file), q) => {
            if !q.is_empty() {
                whatever!("Conflicted file with query terms");
            }

            if verbose {
                info!("Opening file '{}'...", file.display());
            }

            open_file(file)
                .context(CreateCommandSnafu)
                .map(|file| file.into_inner())
        }
        (None, q) => {
            if q.is_empty() {
                whatever!("Query not specified");
            }

            let mut obj =
                parse_queries(&q).whatever_context("Could not build query object from terms")?;

            // (0008,0052) CS QueryRetrieveLevel
            let level = match (patient, study) {
                (true, false) => "PATIENT",
                (false, true) | (false, false) => "STUDY",
                _ => unreachable!(),
            };
            obj.put(DataElement::new(
                tags::QUERY_RETRIEVE_LEVEL,
                VR::CS,
                PrimitiveValue::from(level),
            ));

            Ok(obj)
        }
    }
}

fn run() -> Result<(), Error> {
    let App {
        addr,
        file,
        verbose,
        calling_ae_title,
        called_ae_title,
        max_pdu_length,
        patient,
        study,
        query,
    } = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
            .finish(),
    )
    .unwrap_or_else(|e| {
        error!("{}", snafu::Report::from_error(e));
    });

    let dcm_query = build_query(file, query, patient, study, verbose)?;

    let abstract_syntax = match (patient, study) {
        // Patient Root Query/Retrieve Information Model - FIND
        (true, false) => "1.2.840.10008.5.1.4.1.2.1.1",
        // Study Root Query/Retrieve Information Model – FIND (default)
        (false, false) | (false, true) => "1.2.840.10008.5.1.4.1.2.2.1",
        // Series
        _ => unreachable!("Unexpected flag combination"),
    };

    if verbose {
        info!("Establishing association with '{}'...", &addr);
    }

    let mut scu_opt = ClientAssociationOptions::new()
        .with_abstract_syntax(abstract_syntax)
        .calling_ae_title(calling_ae_title)
        .max_pdu_length(max_pdu_length);

    if let Some(called_ae_title) = called_ae_title {
        scu_opt = scu_opt.called_ae_title(called_ae_title);
    }

    let mut scu = scu_opt.establish_with(&addr).context(InitScuSnafu)?;

    if verbose {
        info!("Association established");
    }

    let pc_selected = if let Some(pc_selected) = scu.presentation_contexts().first() {
        pc_selected
    } else {
        error!("Could not choose a presentation context");
        let _ = scu.abort();
        std::process::exit(-2);
    };
    let pc_selected_id = pc_selected.id;

    let ts = if let Some(ts) = TransferSyntaxRegistry.get(&pc_selected.transfer_syntax) {
        ts
    } else {
        error!("Poorly negotiated transfer syntax");
        let _ = scu.abort();
        std::process::exit(-2);
    };

    if verbose {
        debug!("Transfer Syntax: {}", ts.name());
    }

    let cmd = find_req_command(abstract_syntax, 1);

    let mut cmd_data = Vec::with_capacity(128);
    cmd.write_dataset_with_ts(&mut cmd_data, &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased())
        .whatever_context("Failed to write command")?;

    let mut iod_data = Vec::with_capacity(128);
    dcm_query
        .write_dataset_with_ts(&mut iod_data, ts)
        .whatever_context("failed to write identifier dataset")?;

    let nbytes = cmd_data.len() + iod_data.len();

    if verbose {
        debug!("Sending query ({} B)...", nbytes);
    }

    let pdu = Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: pc_selected_id,
            value_type: PDataValueType::Command,
            is_last: true,
            data: cmd_data,
        }],
    };
    scu.send(&pdu).whatever_context("Could not send command")?;

    let pdu = Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: pc_selected_id,
            value_type: PDataValueType::Data,
            is_last: true,
            data: iod_data,
        }],
    };
    scu.send(&pdu)
        .whatever_context("Could not send C-Find request")?;

    if verbose {
        debug!("Awaiting response...");
    }

    let mut i = 0;
    loop {
        let rsp_pdu = scu
            .receive()
            .whatever_context("Failed to receive response from remote node")?;

        match rsp_pdu {
            Pdu::PData { data } => {
                let data_value = &data[0];

                let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                    &data_value.data[..],
                    &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
                )
                .context(ReadCommandSnafu)?;
                if verbose {
                    eprintln!("Match #{} Response command:", i);
                    DumpOptions::new()
                        .dump_object_to(stderr(), &cmd_obj)
                        .context(DumpOutputSnafu)?;
                }
                let status = cmd_obj
                    .element(tags::STATUS)
                    .whatever_context("status code from response is missing")?
                    .to_int::<u16>()
                    .whatever_context("failed to read status code")?;
                if status == 0 {
                    if verbose {
                        debug!("Matching is complete");
                    }
                    if i == 0 {
                        info!("No results matching query");
                    }
                    break;
                } else if status == 0xFF00 || status == 0xFF01 {
                    if verbose {
                        debug!("Operation pending: {:x}", status);
                    }

                    // fetch DICOM data

                    let dcm = {
                        let mut rsp = scu.receive_pdata();
                        let mut response_data = Vec::new();
                        rsp.read_to_end(&mut response_data)
                            .whatever_context("Failed to read response data")?;

                        InMemDicomObject::read_dataset_with_ts(&response_data[..], ts)
                            .whatever_context("Could not read response data set")?
                    };

                    println!(
                        "------------------------ Match #{} ------------------------",
                        i
                    );
                    DumpOptions::new()
                        .dump_object(&dcm)
                        .context(DumpOutputSnafu)?;
                    i += 1;
                } else {
                    warn!("Operation failed (status code {})", status);
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
                error!("Unexpected SCP response: {:?}", pdu);
                let _ = scu.abort();
                std::process::exit(-2);
            }
        }
    }
    let _ = scu.release();

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
