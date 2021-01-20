use dicom::{core::{DataElement, PrimitiveValue, VR}};
use dicom::core::dicom_value;
use dicom::object::{StandardDataDictionary, Tag, mem::InMemDicomObject};
use dicom_ul::pdu;
use dicom_ul::{
    association::scu::ScuAssociationOptions,
    pdu::{PDataValueType, Pdu},
};
use pdu::PDataValue;
use structopt::StructOpt;

/// DICOM C-ECHO SCU
#[derive(Debug, StructOpt)]
struct App {
    /// socket address to SCP (example: "127.0.0.1:104")
    addr: String,
    /// verbose mode
    #[structopt(short = "v")]
    verbose: bool,
    /// the C-ECHO message ID
    #[structopt(short = "m", long = "message-id", default_value = "1")]
    message_id: u16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let App {
        addr,
        verbose,
        message_id,
    } = App::from_args();

    let mut association = ScuAssociationOptions::new()
        .with_abstract_syntax("1.2.840.10008.1.1")
        .establish(&addr)?;

    if verbose {
        println!("Association with {} successful", addr);
    }

    // commands are always in implict VR LE
    let ts = dicom::transfer_syntax::entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();

    let obj = create_echo_command(message_id);

    let mut data = Vec::new();

    obj.write_dataset_with_ts(&mut data, &ts)?;

    association.send(&Pdu::PData {
        data: vec![PDataValue {
            presentation_context_id: association.presentation_context_id(),
            value_type: PDataValueType::Command,
            is_last: true,
            data,
        }],
    })?;

    if verbose {
        println!(
            "Echo message sent (msg id {}), awaiting reply...",
            message_id
        );
    }

    let pdu = association.receive()?;


    match pdu {
        Pdu::PData { data } => {
            let data_value = &data[0];
            let v = &data_value.data;

            let obj = InMemDicomObject::read_dataset_with_ts(v.as_slice(), &ts)?;
            if verbose {
                println!("{:?}", obj);
            }

            // check status
            let status_elem = obj.element(Tag(0x0000, 0x0900))?;
            if verbose {
                println!("Status: {}", status_elem.to_int::<u16>()?);
            }

            // msg ID response, should be equal to sent msg ID
            let msg_id_elem = obj.element(Tag(0x0000, 0x0120))?;

            assert_eq!(message_id, msg_id_elem.to_int()?);
            if verbose {
                println!("C-ECHO successful.");
            }
        },
        pdu => panic!("Unexpected pdu {:?}", pdu),
    }

    Ok(())
}

fn create_echo_command(message_id: u16) -> InMemDicomObject<StandardDataDictionary> {
    let mut obj = InMemDicomObject::create_empty();

    // group length
    obj.put(DataElement::new(
        Tag(0x0000, 0x0000),
        VR::UI,
        PrimitiveValue::from(8 + 18 + 8 + 2 + 8 + 2 + 8 + 2),
    ));

    // service
    obj.put(DataElement::new(
        Tag(0x0000, 0x0002),
        VR::UI,
        dicom_value!(Str, "1.2.840.10008.1.1\0"),
    ));
    // command
    obj.put(DataElement::new(
        Tag(0x0000, 0x0100),
        VR::US,
        dicom_value!(U16, [0x0030]),
    ));
    // message ID
    obj.put(DataElement::new(
        Tag(0x0000, 0x0110),
        VR::US,
        dicom_value!(U16, [message_id]),
    ));
    // data set type
    obj.put(DataElement::new(
        Tag(0x0000, 0x0800),
        VR::US,
        dicom_value!(U16, [0x0101]),
    ));

    obj
}
