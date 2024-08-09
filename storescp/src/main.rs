use std::{
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};

use clap::Parser;
use dicom_core::{dicom_value, DataElement, VR};
use dicom_dictionary_std::tags;
use dicom_object::{InMemDicomObject, StandardDataDictionary};
use tracing::{error, info, Level};


mod transfer;
#[cfg(feature = "async")]
mod store_async;
#[cfg(feature = "async")]
use store_async::run;
#[cfg(not(feature = "async"))]
mod store_sync;
#[cfg(not(feature = "async"))]
use store_sync::run;

/// DICOM C-STORE SCP
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// Verbose mode
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
    /// Calling Application Entity title
    #[arg(long = "calling-ae-title", default_value = "STORE-SCP")]
    calling_ae_title: String,
    /// Enforce max pdu length
    #[arg(short = 's', long = "strict")]
    strict: bool,
    /// Only accept native/uncompressed transfer syntaxes
    #[arg(long)]
    uncompressed_only: bool,
    /// Accept unknown SOP classes
    #[arg(long)]
    promiscuous: bool,
    /// Maximum PDU length
    #[arg(short = 'm', long = "max-pdu-length", default_value = "16384")]
    max_pdu_length: u32,
    /// Output directory for incoming objects
    #[arg(short = 'o', default_value = ".")]
    out_dir: PathBuf,
    /// Which port to listen on
    #[arg(short, default_value = "11111")]
    port: u16,
}



fn create_cstore_response(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            dicom_value!(Str, sop_class_uid),
        ),
        DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x8001])),
        DataElement::new(
            tags::MESSAGE_ID_BEING_RESPONDED_TO,
            VR::US,
            dicom_value!(U16, [message_id]),
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0101]),
        ),
        DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [0x0000])),
        DataElement::new(
            tags::AFFECTED_SOP_INSTANCE_UID,
            VR::UI,
            dicom_value!(Str, sop_instance_uid),
        ),
    ])
}

fn create_cecho_response(message_id: u16) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x8030])),
        DataElement::new(
            tags::MESSAGE_ID_BEING_RESPONDED_TO,
            VR::US,
            dicom_value!(U16, [message_id]),
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0101]),
        ),
        DataElement::new(tags::STATUS, VR::US, dicom_value!(U16, [0x0000])),
    ])
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    let args = Arc::new(App::parse());

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if args.verbose {
                Level::DEBUG
            } else {
                Level::INFO
            })
            .finish(),
    )
    .unwrap_or_else(|e| {
        eprintln!(
            "Could not set up global logger: {}",
            snafu::Report::from_error(e)
        );
    });

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
            if let Err(e) = run(socket, &args).await {
                error!("{}", snafu::Report::from_error(e));
            }
        });
    }
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;

    let args = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if args.verbose {
                Level::DEBUG
            } else {
                Level::INFO
            })
            .finish(),
    )
    .unwrap_or_else(|e| {
        eprintln!(
            "Could not set up global logger: {}",
            snafu::Report::from_error(e)
        );
    });

    std::fs::create_dir_all(&args.out_dir).unwrap_or_else(|e| {
        error!("Could not create output directory: {}", e);
        std::process::exit(-2);
    });

    let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), args.port);
    let listener = std::net::TcpListener::bind(listen_addr)?;
    info!(
        "{} listening on: tcp://{}",
        &args.calling_ae_title, listen_addr
    );

    for stream in listener.incoming() {
        match stream {
            Ok(scu_stream) => {
                if let Err(e) = run(scu_stream, &args) {
                    error!("{}", snafu::Report::from_error(e));
                }
            }
            Err(e) => {
                error!("{}", snafu::Report::from_error(e));
            }
        }
    }

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
