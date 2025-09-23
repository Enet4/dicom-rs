use clap::Parser;
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_dump::DumpOptions;
use dicom_encoding::transfer_syntax;
use dicom_object::{mem::InMemDicomObject, open_file};
use dicom_transfer_syntax_registry::{entries, TransferSyntaxRegistry};
use dicom_ul::pdu::commands::DatasetRequiredCommand;
use dicom_ul::pdu::{CFindRq, Pdu};
use dicom_ul::{
    association::ClientAssociationOptions,
};
use query::parse_queries;
use snafu::prelude::*;
use std::io::{stderr, BufRead as _, Read};
use std::path::PathBuf;
use tracing::{debug, error, info, warn, Level};
use transfer_syntax::TransferSyntaxIndex;

mod query;

/// DICOM C-FIND SCU
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// socket address to FIND SCP (example: "127.0.0.1:1045")
    addr: String,
    /// a DICOM file representing the query object
    file: Option<PathBuf>,
    /// a file containing lines of queries
    #[arg(long)]
    query_file: Option<PathBuf>,
    /// a sequence of queries
    #[arg(short('q'))]
    query: Vec<String>,

    /// verbose mode
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
    /// the calling AE title
    #[arg(long = "calling-ae-title", default_value = "FIND-SCU")]
    calling_ae_title: String,
    /// the called AE title
    #[arg(long = "called-ae-title")]
    called_ae_title: Option<String>,
    /// the maximum PDU length
    #[arg(
        long = "max-pdu-length",
        default_value = "16384",
        value_parser(clap::value_parser!(u32).range(4096..=131_072))
    )]
    max_pdu_length: u32,

    /// use patient root information model
    #[arg(short = 'P', long, conflicts_with = "study", conflicts_with = "mwl")]
    patient: bool,
    /// use study root information model (default)
    #[arg(short = 'S', long, conflicts_with = "patient", conflicts_with = "mwl")]
    study: bool,
    /// use modality worklist information model
    #[arg(
        short = 'W',
        long,
        conflicts_with = "study",
        conflicts_with = "patient"
    )]
    mwl: bool,
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
    CreateCommand { source: dicom_object::ReadError },

    /// Could not read DICOM command
    ReadCommand { source: dicom_object::ReadError },

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
    query_file: Option<PathBuf>,
    q: Vec<String>,
    patient: bool,
    study: bool,
    mwl: bool,
    verbose: bool,
) -> Result<InMemDicomObject, Error> {
    // read query file if provided
    let (base_query_obj, mut has_base) = if let Some(file) = file {
        if verbose {
            info!("Opening file '{}'...", file.display());
        }

        (
            open_file(file).context(CreateCommandSnafu)?.into_inner(),
            true,
        )
    } else {
        (InMemDicomObject::new_empty(), false)
    };

    // read queries from query text file
    let mut obj = base_query_obj;
    if let Some(query_file) = query_file {
        // read text file line by line
        let mut queries = Vec::new();
        let file = std::fs::File::open(query_file).whatever_context("Could not open query file")?;
        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            let line = line.whatever_context("Could not read line from query file")?;
            {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
            }
            queries.push(line);
        }

        obj = parse_queries(obj, &queries)
            .whatever_context("Could not build query object from query file")?;
        has_base = true;
    }

    // read query options from command line

    if q.is_empty() && !has_base {
        whatever!("Query not specified");
    }

    let mut obj =
        parse_queries(obj, &q).whatever_context("Could not build query object from terms")?;

    // try to infer query retrieve level if not defined by the user
    // but only if not using worklist
    if !mwl && obj.get(tags::QUERY_RETRIEVE_LEVEL).is_none() {
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
    }

    Ok(obj)
}

fn run() -> Result<(), Error> {
    let App {
        addr,
        file,
        query_file,
        query,
        verbose,
        calling_ae_title,
        called_ae_title,
        max_pdu_length,
        patient,
        study,
        mwl,
    } = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
            .finish(),
    )
    .unwrap_or_else(|e| {
        error!("{}", snafu::Report::from_error(e));
    });

    let dcm_query = build_query(file, query_file, query, patient, study, mwl, verbose)?;

    let abstract_syntax = match (patient, study, mwl) {
        // Patient Root Query/Retrieve Information Model - FIND
        (true, false, false) => uids::PATIENT_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_FIND,
        // Modality Worklist Information Model – FIND
        (false, false, true) => uids::MODALITY_WORKLIST_INFORMATION_MODEL_FIND,
        // Study Root Query/Retrieve Information Model – FIND (default)
        (false, false, false) | (false, true, false) => {
            uids::STUDY_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_FIND
        }
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

    let cmd = CFindRq::builder()
        .affected_sop_class_uid(abstract_syntax)
        .message_id(1)
        .build();
    println!("{:?}", &cmd);
    let pdu = cmd.pdu_with_dataset(pc_selected_id, dcm_query, ts)
        .whatever_context("Failed to write PDU")?;
    scu.send(&pdu).whatever_context("Could not send command")?;

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
                if data.is_empty() {
                    error!("Empty PData response");
                    break;
                } else if ![1, 2].contains(&data.len()) {
                    warn!(
                        "Unexpected number of PDataValue parts: {} (allowed 1 or 2)",
                        data.len()
                    );
                    break;
                }

                let data_value = &data[0];

                let cmd_obj = InMemDicomObject::read_dataset_with_ts(
                    &data_value.data[..],
                    &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
                )
                .context(ReadCommandSnafu)?;
                if verbose {
                    eprintln!("Match #{i} Response command:");
                    DumpOptions::new()
                        .dump_object_to(stderr(), &cmd_obj)
                        .context(DumpOutputSnafu)?;
                }
                let status = cmd_obj
                    .get(tags::STATUS)
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
                    // Some worklist servers sends both command and data in the same PData
                    // So there is no need to download another PData
                    let dcm = if let Some(second_pdata) = data.get(1) {
                        InMemDicomObject::read_dataset_with_ts(second_pdata.data.as_slice(), ts)
                            .whatever_context("Could not read response data set")?
                    } else {
                        let mut rsp = scu.receive_pdata();
                        let mut response_data = Vec::new();
                        rsp.read_to_end(&mut response_data)
                            .whatever_context("Failed to read response data")?;

                        InMemDicomObject::read_dataset_with_ts(&response_data[..], ts)
                            .whatever_context("Could not read response data set")?
                    };

                    println!(
                        "------------------------ Match #{i} ------------------------"
                    );
                    DumpOptions::new()
                        .dump_object(&dcm)
                        .context(DumpOutputSnafu)?;

                    // check DICOM status in response data,
                    // as some implementations might report status code 0
                    // upon sending the response data
                    if let Some(status) = dcm.get(tags::STATUS) {
                        let status = status.to_int::<u16>().ok();
                        if status == Some(0) {
                            if verbose {
                                debug!("Matching is complete");
                            }
                            break;
                        }
                    }

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

#[cfg(test)]
mod tests {
    use crate::App;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        App::command().debug_assert();
    }
}
