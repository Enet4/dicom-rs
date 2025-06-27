use clap::Parser;
use dicom_core::dicom_value;
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::{tags, uids};
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
use snafu::prelude::*;
use snafu::Report;
use std::io::{stderr, BufRead as _, Read};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;
use std::{thread, time};
use tracing::{debug, error, info, warn, Level};
use transfer_syntax::TransferSyntaxIndex;

mod query;
mod store_async;
use store_async::run_store_async;

/// DICOM C-MOVE SCU
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// socket address to MOVE SCP (example: "127.0.0.1:1045")
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
    #[arg(long = "calling-ae-title", default_value = "STORE-SCP")]
    calling_ae_title: String,
    /// the called AE title
    #[arg(long = "called-ae-title")]
    called_ae_title: Option<String>,
    /// the C-MOVE destination AE title
    #[arg(long = "move-destination", default_value = "STORE-SCP")]
    move_destination: String,

    /// the maximum PDU length
    #[arg(
        long = "max-pdu-length",
        default_value = "16384",
        value_parser(clap::value_parser!(u32).range(4096..=131_072))
    )]
    max_pdu_length: u32,
    /// Output directory for incoming objects
    #[arg(short = 'o', default_value = ".")]
    out_dir: PathBuf,
    /// Which port to listen on
    #[arg(short, default_value = "11111")]
    port: u16,

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

    /// Enforce max pdu length
    #[arg(short = 's', long = "strict")]
    strict: bool,
    /// Only accept native/uncompressed transfer syntaxes
    #[arg(long)]
    uncompressed_only: bool,
    /// Accept unknown SOP classes
    #[arg(long)]
    promiscuous: bool,

    /// Don't use built-in scp
    #[arg(long = "no-scp", default_value = "false")]
    no_scp: bool,
}

fn main() {
    let app = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if app.verbose {
                Level::DEBUG
            } else {
                Level::INFO
            })
            .finish(),
    )
    .unwrap_or_else(|e| {
        error!("{}", snafu::Report::from_error(e));
    });

    if !app.no_scp {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .spawn(async move {
                run_async(app).await.unwrap_or_else(|e| {
                    error!("{:?}", e);
                    std::process::exit(-2);
                });
            });

            // make sure the async runtime is running
            thread::sleep(time::Duration::from_millis(3_000));
    }


    run_move_scu(App::parse()).unwrap_or_else(|err| {
        error!("{}", snafu::Report::from_error(err));
        std::process::exit(-2);
    });
}

async fn run_async(args: App) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    let args = Arc::new(args);

    std::fs::create_dir_all(&args.out_dir).unwrap_or_else(|e| {
        error!("Could not create output directory: {}", e);
        std::process::exit(-2);
    });

    let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), args.port);
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    info!(
        "{} listening on: tcp://{}",
        &args.calling_ae_title, listen_addr
    );

    loop {
        let (socket, _addr) = listener.accept().await?;
        let args = args.clone();
        tokio::task::spawn(async move {
            if let Err(e) = run_store_async(socket, &args).await {
                error!("{}", Report::from_error(e));
            }
        });
    }
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

fn run_move_scu(app: App) -> Result<(), Error> {
    let App {
        addr,
        file,
        query_file,
        query,
        verbose,
        calling_ae_title,
        called_ae_title,
        max_pdu_length,
        move_destination,
        patient,
        study,
        mwl,
        out_dir: _,
        port: _,
        strict: _,
        uncompressed_only: _,
        promiscuous: _,
        no_scp: _,
    } = app;

    info!("verbose mode: {}", verbose);

    info!("sending c_move request to: {}", addr);

    let dcm_query = build_query(file, query_file, query, patient, study, mwl, verbose)?;

    let abstract_syntax = uids::STUDY_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_MOVE;

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

    let cmd = move_req_command(abstract_syntax, move_destination.as_str(), 1);

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
        .whatever_context("Could not send C-MOVE request")?;

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
                    eprintln!("Match #{} Response command:", i);
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
                        "------------------------ Match #{} ------------------------",
                        i
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

fn move_req_command(
    sop_class_uid: &str,
    move_destination: &str,
    message_id: u16,
) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        // SOP Class UID
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            PrimitiveValue::from(sop_class_uid),
        ),
        // command field
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            // 0021H: C-MOVE-RQ message  --> suggestion to create constants for these
            dicom_value!(U16, [0x0021]),
        ),
        // message ID
        DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [message_id])),
        //priority
        DataElement::new(
            tags::PRIORITY,
            VR::US,
            // medium
            dicom_value!(U16, [0x0000]),
        ),
        // data set type
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0001]),
        ),
        // data set type
        DataElement::new(
            tags::MOVE_DESTINATION,
            VR::AE,
            PrimitiveValue::from(move_destination),
        ),
    ])
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
