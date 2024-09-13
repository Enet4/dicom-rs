use clap::Parser;
use dicom_core::{dicom_value, DataElement, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{mem::InMemDicomObject, StandardDataDictionary};
use dicom_ul::{
    association::client::ClientAssociationOptions,
    pdu::{self, PDataValueType, Pdu},
};
use pdu::PDataValue;
use snafu::{prelude::*, Whatever};
use tracing::{debug, error, info, warn, Level};

/// DICOM C-ECHO SCU
#[derive(Debug, Parser)]
#[command(version)]
struct App {
    /// socket address to SCP,
    /// optionally with AE title
    /// (example: "QUERY-SCP@127.0.0.1:1045")
    addr: String,
    /// verbose mode
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
    /// the C-ECHO message ID
    #[arg(short = 'm', long = "message-id", default_value = "1")]
    message_id: u16,
    /// the calling AE title
    #[arg(long = "calling-ae-title", default_value = "ECHOSCU")]
    calling_ae_title: String,
    /// the called Application Entity title,
    /// overrides AE title in address if present [default: ANY-SCP]
    #[arg(long = "called-ae-title")]
    called_ae_title: Option<String>,
}

fn main() {
    run().unwrap_or_else(|e| {
        error!("{}", snafu::Report::from_error(e));
        std::process::exit(-2);
    })
}

fn run() -> Result<(), Whatever> {
    let App {
        addr,
        verbose,
        message_id,
        called_ae_title,
        calling_ae_title,
    } = App::parse();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
            .finish(),
    )
    .whatever_context("Could not set up global logging subscriber")
    .unwrap_or_else(|e: Whatever| {
        eprintln!("[ERROR] {}", snafu::Report::from_error(e));
    });

    let mut association_opt = ClientAssociationOptions::new()
        .with_abstract_syntax("1.2.840.10008.1.1")
        .calling_ae_title(calling_ae_title);
    if let Some(called_ae_title) = called_ae_title {
        association_opt = association_opt.called_ae_title(called_ae_title);
    }
    let mut association = association_opt
        .establish_with(&addr)
        .whatever_context("Could not establish association with SCP")?;

    let pc = association
        .presentation_contexts()
        .first()
        .whatever_context("No presentation context accepted")?
        .clone();

    if verbose {
        debug!("Association with {} successful", addr);
    }

    // commands are always in implicit VR LE
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
        debug!(
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
                dicom_dump::dump_object(&obj)
                    .whatever_context("Failed to output DICOM response")?;
            }

            // check status
            let status = obj
                .element(tags::STATUS)
                .whatever_context("Missing Status code in response")?
                .to_int::<u16>()
                .whatever_context("Status code in response is not a valid integer")?;
            if verbose {
                debug!("Status: {:04X}H", status);
            }
            match status {
                // Success
                0 => {
                    if verbose {
                        info!("✓ C-ECHO successful");
                    }
                }
                // Warning
                1 | 0x0107 | 0x0116 | 0xB000..=0xBFFF => {
                    warn!("Possible issue in C-ECHO (status code {:04X}H)", status);
                }
                0xFF00 | 0xFF01 => {
                    warn!(
                        "Possible issue in C-ECHO: status is pending (status code {:04X}H)",
                        status
                    );
                }
                0xFE00 => {
                    warn!("Operation cancelled");
                }
                _ => {
                    error!("C-ECHO failed (status code {:04X}H)", status);
                }
            }

            // msg ID response, should be equal to sent msg ID
            let got_msg_id: u16 = obj
                .element(tags::MESSAGE_ID_BEING_RESPONDED_TO)
                .whatever_context("Could not retrieve Message ID from response")?
                .to_int()
                .whatever_context("Message ID is not a valid integer")?;

            if message_id != got_msg_id {
                whatever!("Message ID mismatch");
            }
        }
        pdu => whatever!("Unexpected PDU {:?}", pdu),
    }

    Ok(())
}

fn create_echo_command(message_id: u16) -> InMemDicomObject<StandardDataDictionary> {
    InMemDicomObject::command_from_element_iter([
        // service
        DataElement::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, uids::VERIFICATION),
        // command
        DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x0030])),
        // message ID
        DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [message_id])),
        // data set type
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0101]),
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
