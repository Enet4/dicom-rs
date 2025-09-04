use dicom_encoding::TransferSyntax;
use dicom_object::{InMemDicomObject, WriteError};
use dicom_transfer_syntax_registry::entries;

use crate::{Pdu, pdu::{PDataValue, PDataValueType}};

#[allow(unused)]
#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Priority {
    Low = 0x0002,
    Medium = 0x0000,
    High = 0x0001,
}

#[allow(non_camel_case_types)]
pub enum CommandField {
    C_STORE_RQ         = 0x0001,
    C_STORE_RSP        = 0x8001,
    C_GET_RQ           = 0x0010,
    C_GET_RSP          = 0x8010,
    C_FIND_RQ          = 0x0020,
    C_FIND_RSP         = 0x8020,
    C_MOVE_RQ          = 0x0021,
    C_MOVE_RSP         = 0x8021,
    C_ECHO_RQ          = 0x0030,
    C_ECHO_RSP         = 0x8030,
    N_EVENT_REPORT_RQ  = 0x0100,
    N_EVENT_REPORT_RSP = 0x8100,
    N_GET_RQ           = 0x0110,
    N_GET_RSP          = 0x8110,
    N_SET_RQ           = 0x0120,
    N_SET_RSP          = 0x8120,
    N_ACTION_RQ        = 0x0130,
    N_ACTION_RSP       = 0x8130,
    N_CREATE_RQ        = 0x0140,
    N_CREATE_RSP       = 0x8140,
    N_DELETE_RQ        = 0x0150,
    N_DELETE_RSP       = 0x8150,
    C_CANCEL_RQ        = 0x0FFF
}

pub trait Command {
    /// Get the command field code for this Command
    fn command_field(&self) -> u16;
    /// Get the dicom dataset represenation of this command
    fn dataset(&self) -> InMemDicomObject;
    /// Encode the command into bytes
    fn encode(&self) -> Result<Vec<u8>, Box<WriteError>> {
        let ds = self.dataset();
        let mut buffer = Vec::new();
        ds.write_dataset_with_ts(&mut buffer, &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased())
            .map_err(Box::from)?;
        Ok(buffer)
    }
    /// Create a PDU for the command using the selected presentation context
    fn pdu(&self, pc_selected: u8) -> Result<Pdu, Box<WriteError>> {
        Ok(Pdu::PData {
            data: vec![PDataValue {
                presentation_context_id: pc_selected,
                value_type: PDataValueType::Command,
                is_last: true,
                data: self.encode()?,
            }],
        })
    }

    /// Create a PDU for the command using the selected presentation context and associated dataset
    ///
    /// NOTE: Panics if the transfer syntax from the presentation context is not found in the registry.
    fn pdu_with_dataset(
        &self,
        pc_selected: u8,
        dataset: InMemDicomObject,
        ts: &TransferSyntax,
    ) -> Result<Pdu, Box<WriteError>> {
        let mut ds_data = Vec::new();
        dataset
            .write_dataset_with_ts(&mut ds_data, ts)
            .expect("Failed to write dataset to buffer");
        let data = vec![
            PDataValue {
                presentation_context_id: pc_selected,
                value_type: PDataValueType::Command,
                is_last: true,
                data: self.encode()?,
            },
            PDataValue {
                presentation_context_id: pc_selected,
                value_type: PDataValueType::Data,
                is_last: true,
                data: ds_data,
            },
        ];
        Ok(Pdu::PData { data })
    }
}