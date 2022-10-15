use std::{
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    path::PathBuf,
};

use clap::Parser;
use dicom_core::{dicom_value, DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_object::{FileMetaTableBuilder, InMemDicomObject, StandardDataDictionary};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_ul::{pdu::PDataValueType, Pdu};
use tracing::{debug, error, info, warn, Level};

use crate::transfer::{ABSTRACT_SYNTAXES, NATIVE_TRANSFER_SYNTAXES, TRANSFER_SYNTAXES};

mod transfer;

/// DICOM C-STORE SCP
#[derive(Debug, Parser)]
struct App {
    /// verbose mode
    #[clap(short = 'v', long = "verbose")]
    verbose: bool,
    /// the calling Application Entity title
    #[structopt(long = "calling-ae-title", default_value = "STORE-SCP")]
    calling_ae_title: String,
    /// enforce max pdu length
    #[clap(short = 's', long = "strict")]
    strict: bool,
    /// Only accept native/uncompressed transfer syntaxes
    #[clap(long)]
    uncompressed_only: bool,
    /// max pdu length
    #[clap(short = 'm', long = "max-pdu-length", default_value = "16384")]
    max_pdu_length: u32,
    /// output directory for incoming objects
    #[clap(short = 'o', default_value = ".")]
    out_dir: PathBuf,
    /// Which port to listen on
    #[clap(short, default_value = "11111")]
    port: u16,
}

fn run(scu_stream: TcpStream, args: &App) -> Result<(), Box<dyn std::error::Error>> {
    let App {
        verbose,
        calling_ae_title,
        strict,
        uncompressed_only,
        max_pdu_length,
        out_dir,
        port: _,
    } = args;
    let verbose = *verbose;

    let mut buffer: Vec<u8> = Vec::with_capacity(*max_pdu_length as usize);
    let mut instance_buffer: Vec<u8> = Vec::with_capacity(1024 * 1024);
    let mut msgid = 1;
    let mut sop_class_uid = "".to_string();
    let mut sop_instance_uid = "".to_string();

    let mut options = dicom_ul::association::ServerAssociationOptions::new()
        .accept_any()
        .ae_title(calling_ae_title)
        .strict(*strict);

    let accepted_tss = if *uncompressed_only {
        NATIVE_TRANSFER_SYNTAXES
    } else {
        TRANSFER_SYNTAXES
    };

    for uid in accepted_tss {
        options = options.with_transfer_syntax(*uid);
    }

    for uid in ABSTRACT_SYNTAXES {
        options = options.with_abstract_syntax(*uid);
    }

    let mut association = options.establish(scu_stream)?;

    info!("New association from {}", association.client_ae_title());
    debug!(
        "> Presentation contexts: {:?}",
        association.presentation_contexts()
    );

    loop {
        match association.receive() {
            Ok(mut pdu) => {
                if verbose {
                    debug!("scu ----> scp: {}", pdu.short_description());
                }
                match pdu {
                    Pdu::PData { ref mut data } => {
                        if data[0].value_type == PDataValueType::Data && !data[0].is_last {
                            instance_buffer.append(&mut data[0].data);
                        } else if data[0].value_type == PDataValueType::Command && data[0].is_last {
                            // commands are always in implict VR LE
                            let ts =
                                dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                                    .erased();
                            let data_value = &data[0];
                            let v = &data_value.data;

                            let obj = InMemDicomObject::read_dataset_with_ts(v.as_slice(), &ts)?;
                            msgid = obj.element(tags::MESSAGE_ID)?.to_int()?;
                            sop_class_uid = obj
                                .element(tags::AFFECTED_SOP_CLASS_UID)?
                                .to_str()?
                                .to_string();
                            sop_instance_uid = obj
                                .element(tags::AFFECTED_SOP_INSTANCE_UID)?
                                .to_str()?
                                .to_string();
                            instance_buffer.clear();
                        } else if data[0].value_type == PDataValueType::Data && data[0].is_last {
                            instance_buffer.append(&mut data[0].data);

                            let presentation_context = association
                                .presentation_contexts()
                                .iter()
                                .filter(|pc| pc.id == data[0].presentation_context_id)
                                .next()
                                .unwrap();
                            let ts = &presentation_context.transfer_syntax;

                            let obj = InMemDicomObject::read_dataset_with_ts(
                                instance_buffer.as_slice(),
                                TransferSyntaxRegistry.get(ts).unwrap(),
                            )?;
                            let file_meta = FileMetaTableBuilder::new()
                                .media_storage_sop_class_uid(
                                    obj.element_by_name("SOPClassUID")?.to_str()?,
                                )
                                .media_storage_sop_instance_uid(
                                    obj.element_by_name("SOPInstanceUID")?.to_str()?,
                                )
                                .transfer_syntax(ts)
                                .build()?;
                            let file_obj = obj.with_exact_meta(file_meta);

                            // write the files to the current directory with their SOPInstanceUID as filenames
                            let mut file_path = out_dir.clone();
                            file_path
                                .push(sop_instance_uid.trim_end_matches('\0').to_string() + ".dcm");
                            file_obj.write_to_file(&file_path)?;
                            info!("Stored {}", file_path.display());

                            // send C-STORE-RSP object
                            // commands are always in implict VR LE
                            let ts =
                                dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN
                                    .erased();

                            let obj =
                                create_cstore_response(msgid, &sop_class_uid, &sop_instance_uid);

                            let mut obj_data = Vec::new();

                            obj.write_dataset_with_ts(&mut obj_data, &ts)?;

                            let pdu_response = Pdu::PData {
                                data: vec![dicom_ul::pdu::PDataValue {
                                    presentation_context_id: data[0].presentation_context_id,
                                    value_type: PDataValueType::Command,
                                    is_last: true,
                                    data: obj_data,
                                }],
                            };
                            association.send(&pdu_response)?;
                        }
                    }
                    Pdu::ReleaseRQ => {
                        buffer.clear();
                        association.send(&Pdu::ReleaseRP)?;
                        info!(
                            "Released association with {}",
                            association.client_ae_title()
                        );
                    }
                    _ => {}
                }
            }
            Err(err @ dicom_ul::association::server::Error::Receive { .. }) => {
                debug!("{}", err);
                break;
            }
            Err(err) => {
                warn!("Unexpected error: {}", err);
                break;
            }
        }
    }
    info!("Association with {} dropped", association.client_ae_title());
    Ok(())
}

fn create_cstore_response(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
) -> InMemDicomObject<StandardDataDictionary> {
    let mut obj = InMemDicomObject::new_empty();

    // group length
    obj.put(DataElement::new(
        tags::COMMAND_GROUP_LENGTH,
        VR::UL,
        PrimitiveValue::from(
            8 + sop_class_uid.len() as i32
                + 8
                + 2
                + 8
                + 2
                + 8
                + 2
                + 8
                + 2
                + sop_instance_uid.len() as i32,
        ),
    ));

    // service
    obj.put(DataElement::new(
        tags::AFFECTED_SOP_CLASS_UID,
        VR::UI,
        dicom_value!(Str, sop_class_uid),
    ));
    // command
    obj.put(DataElement::new(
        tags::COMMAND_FIELD,
        VR::US,
        dicom_value!(U16, [0x8001]),
    ));
    // message ID being responded to
    obj.put(DataElement::new(
        tags::MESSAGE_ID_BEING_RESPONDED_TO,
        VR::US,
        dicom_value!(U16, [message_id]),
    ));
    // data set type
    obj.put(DataElement::new(
        tags::COMMAND_DATA_SET_TYPE,
        VR::US,
        dicom_value!(U16, [0x0101]),
    ));
    // status https://dicom.nema.org/dicom/2013/output/chtml/part07/chapter_C.html
    obj.put(DataElement::new(
        tags::STATUS,
        VR::US,
        dicom_value!(U16, [0x0000]),
    ));
    // SOPInstanceUID
    obj.put(DataElement::new(
        tags::AFFECTED_SOP_INSTANCE_UID,
        VR::UI,
        dicom_value!(Str, sop_instance_uid),
    ));

    obj
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = App::from_args();

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
        eprintln!("Could not set up global logger: {}", e);
    });

    std::fs::create_dir_all(&args.out_dir).unwrap_or_else(|e| {
        error!("Could not create output directory: {}", e);
        std::process::exit(-2);
    });

    let listen_addr = SocketAddrV4::new(Ipv4Addr::from(0), args.port);
    let listener = TcpListener::bind(&listen_addr)?;
    info!(
        "{} listening on: tcp://{}",
        &args.calling_ae_title, listen_addr
    );

    for stream in listener.incoming() {
        match stream {
            Ok(scu_stream) => {
                if let Err(e) = run(scu_stream, &args) {
                    error!("{}", e);
                }
            }
            Err(e) => {
                error!("{}", e);
            }
        }
    }

    Ok(())
}
