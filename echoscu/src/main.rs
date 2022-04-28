use dicom_core::dicom_value;
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::{mem::InMemDicomObject, StandardDataDictionary};
use dicom_ul::pdu;
use dicom_ul::{
    association::client::ClientAssociationOptions,
    pdu::{PDataValueType, Pdu},
};
use pdu::PDataValue;
use snafu::{prelude::*, ErrorCompat, Whatever};
use structopt::StructOpt;

/// DICOM C-ECHO SCU
#[derive(Debug, StructOpt)]
struct App {
    /// socket address to SCP (example: "127.0.0.1:104")
    addr: String,
    /// verbose mode
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
    /// the C-ECHO message ID
    #[structopt(short = "m", long = "message-id", default_value = "1")]
    message_id: u16,
    /// the calling AE title
    #[structopt(long = "calling-ae-title", default_value = "ECHOSCU")]
    calling_ae_title: String,
    /// the called AE title
    #[structopt(long = "called-ae-title", default_value = "ANY-SCP")]
    called_ae_title: String,
}

fn report<E: 'static>(err: &E)
where
    E: std::error::Error,
    E: ErrorCompat,
{
    eprintln!("[ERROR] {}", err);
    if let Some(source) = err.source() {
        eprintln!();
        eprintln!("Caused by:");
        for (i, e) in std::iter::successors(Some(source), |e| e.source()).enumerate() {
            eprintln!("   {}: {}", i, e);
        }
    }

    let env_backtrace = std::env::var("RUST_BACKTRACE").unwrap_or_default();
    let env_lib_backtrace = std::env::var("RUST_LIB_BACKTRACE").unwrap_or_default();
    if env_lib_backtrace == "1" || (env_backtrace == "1" && env_lib_backtrace != "0") {
        if let Some(backtrace) = ErrorCompat::backtrace(err) {
            eprintln!();
            eprintln!("Backtrace:");
            eprintln!("{}", backtrace);
        }
    }
}

fn main() {
    run().unwrap_or_else(|e| {
        report(&e);
        std::process::exit(-2);
    })
}

fn run() -> Result<(), Whatever> {
    tracing::subscriber::set_global_default(tracing_subscriber::FmtSubscriber::new())
        .whatever_context("Could not set up global logging subscriber")
        .unwrap_or_else(|e: Whatever| {
            report(&e);
        });

    let App {
        addr,
        verbose,
        message_id,
        called_ae_title,
        calling_ae_title,
    } = App::from_args();

    let mut association = ClientAssociationOptions::new()
        .with_abstract_syntax("1.2.840.10008.1.1")
        .calling_ae_title(calling_ae_title)
        .called_ae_title(called_ae_title)
        .establish(&addr)
        .whatever_context("Could not establish association with SCP")?;

    let pc = association
        .presentation_contexts()
        .first()
        .whatever_context("No presentation context accepted")?
        .clone();

    if verbose {
        println!("Association with {} successful", addr);
    }

    // commands are always in implict VR LE
    let ts = dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    let obj = create_echo_command(message_id);

    let mut data = Vec::new();

    obj.write_dataset_with_ts(&mut data, &ts)
        .whatever_context("Failed to construct C-ECHO request")?;

    association
        .send(&Pdu::PData {
            data: vec![PDataValue {
                presentation_context_id: pc.id,
                value_type: PDataValueType::Command,
                is_last: true,
                data,
            }],
        })
        .whatever_context("Failed to send C-ECHO request")?;

    if verbose {
        println!(
            "Echo message sent (msg id {}), awaiting reply...",
            message_id
        );
    }

    let pdu = association
        .receive()
        .whatever_context("Could not receive response from SCP")?;

    match pdu {
        Pdu::PData { data } => {
            let data_value = &data[0];
            let v = &data_value.data;

            let obj = InMemDicomObject::read_dataset_with_ts(v.as_slice(), &ts)
                .whatever_context("Failed to read response dataset from SCP")?;
            if verbose {
                println!("{:?}", obj);
            }

            // check status
            let status_elem = obj
                .element(tags::STATUS)
                .whatever_context("Missing Status code in response")?;
            if verbose {
                println!(
                    "Status: {}",
                    status_elem
                        .to_int::<u16>()
                        .whatever_context("Status code in response is not a valid integer")?
                );
            }

            // msg ID response, should be equal to sent msg ID
            let msg_id_elem = obj
                .element(tags::MESSAGE_ID_BEING_RESPONDED_TO)
                .whatever_context("Could not retrieve Message ID from response")?;

            if message_id
                == msg_id_elem
                    .to_int()
                    .whatever_context("Message ID is not a valid integer")?
            {
                whatever!("Message ID mismatch");
            }
            if verbose {
                println!("C-ECHO successful.");
            }
        }
        pdu => whatever!("Unexpected PDU {:?}", pdu),
    }

    Ok(())
}

fn create_echo_command(message_id: u16) -> InMemDicomObject<StandardDataDictionary> {
    let mut obj = InMemDicomObject::new_empty();

    // group length
    obj.put(DataElement::new(
        tags::COMMAND_GROUP_LENGTH,
        VR::UI,
        PrimitiveValue::from(8 + 18 + 8 + 2 + 8 + 2 + 8 + 2),
    ));

    // service
    obj.put(DataElement::new(
        tags::AFFECTED_SOP_CLASS_UID,
        VR::UI,
        dicom_value!(Str, "1.2.840.10008.1.1\0"),
    ));
    // command
    obj.put(DataElement::new(
        tags::COMMAND_FIELD,
        VR::US,
        dicom_value!(U16, [0x0030]),
    ));
    // message ID
    obj.put(DataElement::new(
        tags::MESSAGE_ID,
        VR::US,
        dicom_value!(U16, [message_id]),
    ));
    // data set type
    obj.put(DataElement::new(
        tags::COMMAND_DATA_SET_TYPE,
        VR::US,
        dicom_value!(U16, [0x0101]),
    ));

    obj
}
