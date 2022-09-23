use std::{
    io::Write,
    net::{TcpListener, TcpStream},
};

use dicom::{
    core::{DataElement, PrimitiveValue, VR},
    dicom_value,
    dictionary_std::tags,
    encoding::transfer_syntax::TransferSyntaxIndex,
    object::{FileMetaTableBuilder, InMemDicomObject, StandardDataDictionary},
};
use dicom::{
    transfer_syntax::TransferSyntaxRegistry,
    ul::{
        pdu::reader::read_pdu,
        pdu::{
            writer::write_pdu, PDataValueType, PresentationContextProposed,
            PresentationContextResult,
        },
    },
};
use structopt::StructOpt;

/// DICOM C-STORE SCP
#[derive(Debug, StructOpt)]
struct App {
    /// verbose mode
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
    /// enforce max pdu length
    #[structopt(short = "s", long = "strict")]
    strict: bool,
    /// max pdu length
    #[structopt(short = "m", long = "max-pdu-length", default_value = "16384")]
    max_pdu_length: u32,

    /// Which port to listen on
    #[structopt(short, default_value = "11111")]
    port: u16,
}

fn run(
    scu_stream: &mut TcpStream,
    strict: bool,
    verbose: bool,
    max_pdu_length: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer: Vec<u8> = Vec::with_capacity(max_pdu_length as usize);
    let mut instance_buffer: Vec<u8> = Vec::with_capacity(1024 * 1024);
    let mut msgid = 1;
    let mut sop_class_uid = "".to_string();
    let mut sop_instance_uid = "".to_string();

    let mut presentation_context_results: Vec<PresentationContextResult> = vec![];

    loop {
        match read_pdu(scu_stream, max_pdu_length, strict) {
            Ok(mut pdu) => {
                if verbose {
                    println!("scu ----> scp: {}", pdu.short_description());
                }
                match pdu {
                    dicom::ul::Pdu::AssociationRQ {
                        protocol_version,
                        calling_ae_title,
                        called_ae_title,
                        application_context_name,
                        presentation_contexts,
                        user_variables,
                    } => {
                        buffer.clear();
                        presentation_context_results = presentation_contexts
                            .iter()
                            .map(|pc| {
                                let PresentationContextProposed {
                                    id,
                                    abstract_syntax: _,
                                    transfer_syntaxes,
                                } = &pc;
                                PresentationContextResult {
                                    id: *id,
                                    reason:
                                        dicom::ul::pdu::PresentationContextResultReason::Acceptance,
                                    // accept the first proposed transfer syntax
                                    transfer_syntax: transfer_syntaxes[0].clone(),
                                }
                            })
                            .collect();

                        // copying most variables for now, should be set to application specific values
                        let response = dicom::ul::Pdu::AssociationAC {
                            protocol_version,
                            calling_ae_title,
                            called_ae_title,
                            application_context_name,
                            user_variables,
                            presentation_contexts: presentation_context_results.clone(),
                        };
                        write_pdu(&mut buffer, &response).unwrap();
                        if verbose {
                            println!("scu <---- scp: {}", response.short_description());
                        }
                        scu_stream.write_all(&buffer).unwrap();
                    }
                    dicom::ul::Pdu::PData { ref mut data } => {
                        if data[0].value_type == PDataValueType::Data && !data[0].is_last {
                            instance_buffer.append(&mut data[0].data);
                        } else if data[0].value_type == PDataValueType::Command && data[0].is_last {
                            // commands are always in implict VR LE
                            let ts =
                                dicom::transfer_syntax::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();
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

                            let presentation_context = presentation_context_results
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
                            file_obj.write_to_file(sop_instance_uid.trim_end_matches('\0'))?;

                            // send C-STORE-RSP object
                            // commands are always in implict VR LE
                            let ts =
                                dicom::transfer_syntax::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

                            let obj =
                                create_cstore_response(msgid, &sop_class_uid, &sop_instance_uid);

                            let mut obj_data = Vec::new();

                            obj.write_dataset_with_ts(&mut obj_data, &ts)?;

                            let pdu_response = dicom::ul::Pdu::PData {
                                data: vec![dicom::ul::pdu::PDataValue {
                                    presentation_context_id: data[0].presentation_context_id,
                                    value_type: PDataValueType::Command,
                                    is_last: true,
                                    data: obj_data,
                                }],
                            };
                            buffer.clear();
                            write_pdu(&mut buffer, &pdu_response).unwrap();
                            scu_stream.write_all(&buffer).unwrap();
                        }
                    }
                    dicom::ul::Pdu::ReleaseRQ => {
                        buffer.clear();
                        write_pdu(&mut buffer, &dicom::ul::Pdu::ReleaseRP).unwrap();
                        scu_stream.write_all(&buffer).unwrap();
                    }
                    _ => {}
                }
            }
            Err(dicom::ul::pdu::reader::Error::NoPduAvailable { .. }) => {
                break;
            }
            Err(_err) => {
                break;
            }
        }
    }
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
    let App {
        verbose,
        strict,
        port,
        max_pdu_length,
    } = App::from_args();

    let listen_addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&listen_addr).unwrap();
    if verbose {
        println!("listening on: {}", listen_addr);
    }

    for mut stream in listener.incoming() {
        match stream {
            Ok(ref mut scu_stream) => {
                if let Err(e) = run(scu_stream, strict, verbose, max_pdu_length) {
                    eprintln!("[ERROR] {}", e);
                }
            }
            Err(e) => {
                eprintln!("[ERROR] {}", e);
            }
        }
    }

    Ok(())
}
