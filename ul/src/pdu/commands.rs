use dicom_core::{DataElement, VR, dicom_value};
use dicom_dictionary_std::tags;
use dicom_encoding::TransferSyntax;
use dicom_object::{InMemDicomObject, WriteError};
use dicom_transfer_syntax_registry::entries;

use crate::{Pdu, pdu::{PDataValue, PDataValueType}};

#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Priority {
    Low = 0x0002,
    Medium = 0x0000,
    High = 0x0001,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum CommandDatasetType {
    Present = 0x0001,
    Absent = 0x0101
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

/// Trait that marks a message struct to only allow PDUs with an associated dataset
/// 
/// Only exposes the `pdu_with_dataset` command requiring the user to pass an associated
/// dataset
pub trait DatasetRequiredCommand: Command {
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
                data: self.encode(true)?,
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


/// Trait that marks a message struct as conditionally allowing a dataset
/// 
/// This could either be truly conditional, e.g. For the C-FIND response primitive
/// the standard says:
/// 
/// > In the response/confirmation, this is the same list of Attributes with 
/// > values of these Attributes in a particular composite SOP Instance that 
/// > matched. It shall be sent only when that Status (0000,0900) is equal to
/// > Pending (not permitted for other statuses).
/// 
/// A service user option, e.g. for both C-MOVE and C-GET, the response primitive
/// is not required by the DIMSE service to contain a dataset, but it _is_ required
/// by the DIMSE C-GET and C-MOVE service user
/// 
/// 
/// > **NOTE** Structs implementing this trait will have access to both the
/// > `pdu` and `pdu_with_dataset`. Users of these structs should take care
/// > to use the appropriate method based on the standard.
pub trait DatasetConditionalCommand: DatasetRequiredCommand {
    /// Create a PDU for the command using the selected presentation context
    fn pdu(&self, pc_selected: u8) -> Result<Pdu, Box<WriteError>> {
        Ok(Pdu::PData {
            data: vec![PDataValue {
                presentation_context_id: pc_selected,
                value_type: PDataValueType::Command,
                is_last: true,
                data: self.encode(false)?,
            }],
        })
    }
}

/// Trait that marks a message as not allowing a dataset.
pub trait DatasetForbiddenCommand: Command {
    /// Create a PDU for the command using the selected presentation context
    fn pdu(&self, pc_selected: u8) -> Result<Pdu, Box<WriteError>> {
        Ok(Pdu::PData {
            data: vec![PDataValue {
                presentation_context_id: pc_selected,
                value_type: PDataValueType::Command,
                is_last: true,
                data: self.encode(false)?,
            }],
        })
    }

}

pub trait Command {
    /// Get the command field code for this Command
    fn command_field(&self) -> u16;
    /// Get the dicom dataset represenation of this command
    fn dataset(&self) -> InMemDicomObject;
    /// Encode the command into bytes
    fn encode(&self, ds_included: bool) -> Result<Vec<u8>, Box<WriteError>> {
        let mut ds = self.dataset();
        ds.put(
            DataElement::new(
                tags::COMMAND_DATA_SET_TYPE,
                VR::US,
                if ds_included {
                    dicom_value!(CommandDatasetType::Present as u16)
                } else {
                    dicom_value!(CommandDatasetType::Absent as u16)
                }
            )
        );
        let mut buffer = Vec::new();
        ds.write_dataset_with_ts(&mut buffer, &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased())
            .map_err(Box::from)?;
        Ok(buffer)
    }
}