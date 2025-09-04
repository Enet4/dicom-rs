// Auto-generated DICOM command structs

use dicom_core::{DataElement as DE, VR, dicom_value as value};
use dicom_object::{InMemDicomObject};
use bon::Builder;
use dicom_dictionary_std::tags;
use crate::pdu::commands::{CommandField, Priority};
use crate::pdu::commands::Command;

#[derive(Builder)]
pub struct CStoreRq<'a> {
    /// Implementation-specific value. It distinguishes this Message from other Messages.,
    pub message_id: u16,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-STORE-RQ Message.,
    pub message_id_being_responded_to: Option<u16>,
    /// SOP Class UID of the SOP Instance to be stored.,
    pub affected_sop_class_uid: &'a str,
    /// Contains the UID of the SOP Instance to be stored.,
    pub affected_sop_instance_uid: &'a str,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// Contains the DICOM AE Title of the DICOM AE that invoked the C-MOVE operation from which this C-STORE sub-operation is being performed.,
    pub move_originator_application_entity_title: Option<&'a str>,
    /// Contains the Message ID (0000,0110) of the C-MOVE-RQ Message from which this C-STORE sub-operations is being performed.,
    pub move_originator_message_id: Option<u16>,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: Option<u16>
}
impl<'a> Command for CStoreRq<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_STORE_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::AFFECTED_SOP_INSTANCE_UID, VR::UI, value!(self.affected_sop_instance_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::MOVE_ORIGINATOR_APPLICATION_ENTITY_TITLE, VR::AE, value!(self.move_originator_application_entity_title)),
            DE::new(tags::MOVE_ORIGINATOR_MESSAGE_ID, VR::US, value!(self.move_originator_message_id)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0001)),
            DE::new(tags::STATUS, VR::US, value!(self.status))
        ])
    }
}
#[derive(Builder)]
pub struct CStoreRsp<'a> {
    /// Implementation-specific value. It distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-STORE-RQ Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID of the SOP Instance to be stored.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Contains the UID of the SOP Instance to be stored.,
    pub affected_sop_instance_uid: Option<&'a str>,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// Contains the DICOM AE Title of the DICOM AE that invoked the C-MOVE operation from which this C-STORE sub-operation is being performed.,
    pub move_originator_application_entity_title: Option<&'a str>,
    /// Contains the Message ID (0000,0110) of the C-MOVE-RQ Message from which this C-STORE sub-operations is being performed.,
    pub move_originator_message_id: Option<u16>,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: u16
}
impl<'a> Command for CStoreRsp<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_STORE_RSP as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::AFFECTED_SOP_INSTANCE_UID, VR::UI, value!(self.affected_sop_instance_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::MOVE_ORIGINATOR_APPLICATION_ENTITY_TITLE, VR::AE, value!(self.move_originator_application_entity_title)),
            DE::new(tags::MOVE_ORIGINATOR_MESSAGE_ID, VR::US, value!(self.move_originator_message_id)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101)),
            DE::new(tags::STATUS, VR::US, value!(self.status))
        ])
    }
}
#[derive(Builder)]
pub struct CFindRq<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: u16,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-FIND-RQ Message.,
    pub message_id_being_responded_to: Option<u16>,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: &'a str,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: Option<u16>
}
impl<'a> Command for CFindRq<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_FIND_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0001)),
            DE::new(tags::STATUS, VR::US, value!(self.status))
        ])
    }
}
#[derive(Builder)]
pub struct CFindRsp<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-FIND-RQ Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: u16
}
impl<'a> Command for CFindRsp<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_FIND_RSP as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101)),
            DE::new(tags::STATUS, VR::US, value!(self.status))
        ])
    }
}
#[derive(Builder)]
pub struct CFindCncl<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-FIND-RQ Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: Option<u16>
}
impl<'a> Command for CFindCncl<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_CANCEL_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101)),
            DE::new(tags::STATUS, VR::US, value!(self.status))
        ])
    }
}
#[derive(Builder)]
pub struct CGetRq<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: u16,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-GET-RQ Message.,
    pub message_id_being_responded_to: Option<u16>,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: &'a str,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: Option<u16>,
    /// The number of remaining C-STORE sub-operations to be invoked for this C-GET operation.,
    pub number_of_remaining_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that have completed successfully.,
    pub number_of_completed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that have failed.,
    pub number_of_failed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that generated warning responses.,
    pub number_of_warning_suboperations: Option<u16>
}
impl<'a> Command for CGetRq<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_GET_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0001)),
            DE::new(tags::STATUS, VR::US, value!(self.status)),
            DE::new(tags::NUMBER_OF_REMAINING_SUBOPERATIONS, VR::US, value!(self.number_of_remaining_suboperations)),
            DE::new(tags::NUMBER_OF_COMPLETED_SUBOPERATIONS, VR::US, value!(self.number_of_completed_suboperations)),
            DE::new(tags::NUMBER_OF_FAILED_SUBOPERATIONS, VR::US, value!(self.number_of_failed_suboperations)),
            DE::new(tags::NUMBER_OF_WARNING_SUBOPERATIONS, VR::US, value!(self.number_of_warning_suboperations))
        ])
    }
}
#[derive(Builder)]
pub struct CGetRsp<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-GET-RQ Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: u16,
    /// The number of remaining C-STORE sub-operations to be invoked for this C-GET operation.,
    pub number_of_remaining_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that have completed successfully.,
    pub number_of_completed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that have failed.,
    pub number_of_failed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that generated warning responses.,
    pub number_of_warning_suboperations: Option<u16>
}
impl<'a> Command for CGetRsp<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_GET_RSP as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101)),
            DE::new(tags::STATUS, VR::US, value!(self.status)),
            DE::new(tags::NUMBER_OF_REMAINING_SUBOPERATIONS, VR::US, value!(self.number_of_remaining_suboperations)),
            DE::new(tags::NUMBER_OF_COMPLETED_SUBOPERATIONS, VR::US, value!(self.number_of_completed_suboperations)),
            DE::new(tags::NUMBER_OF_FAILED_SUBOPERATIONS, VR::US, value!(self.number_of_failed_suboperations)),
            DE::new(tags::NUMBER_OF_WARNING_SUBOPERATIONS, VR::US, value!(self.number_of_warning_suboperations))
        ])
    }
}
#[derive(Builder)]
pub struct CGetCncl<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-GET-RQ Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: Option<u16>,
    /// The number of remaining C-STORE sub-operations to be invoked for this C-GET operation.,
    pub number_of_remaining_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that have completed successfully.,
    pub number_of_completed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that have failed.,
    pub number_of_failed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-GET operation that generated warning responses.,
    pub number_of_warning_suboperations: Option<u16>
}
impl<'a> Command for CGetCncl<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_CANCEL_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101)),
            DE::new(tags::STATUS, VR::US, value!(self.status)),
            DE::new(tags::NUMBER_OF_REMAINING_SUBOPERATIONS, VR::US, value!(self.number_of_remaining_suboperations)),
            DE::new(tags::NUMBER_OF_COMPLETED_SUBOPERATIONS, VR::US, value!(self.number_of_completed_suboperations)),
            DE::new(tags::NUMBER_OF_FAILED_SUBOPERATIONS, VR::US, value!(self.number_of_failed_suboperations)),
            DE::new(tags::NUMBER_OF_WARNING_SUBOPERATIONS, VR::US, value!(self.number_of_warning_suboperations))
        ])
    }
}
#[derive(Builder)]
pub struct CMoveRq<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: u16,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-MOVE Message.,
    pub message_id_being_responded_to: Option<u16>,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: &'a str,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// Shall be set to the DICOM AE Title of the destination DICOM AE to which the C-STORE sub-operations are being performed.,
    pub move_destination: &'a str,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: Option<u16>,
    /// The number of remaining C-STORE sub-operations to be invoked for this C-MOVE operation.,
    pub number_of_remaining_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that have completed successfully.,
    pub number_of_completed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that have failed.,
    pub number_of_failed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that generated warning responses.,
    pub number_of_warning_suboperations: Option<u16>
}
impl<'a> Command for CMoveRq<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_MOVE_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::MOVE_DESTINATION, VR::AE, value!(self.move_destination)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0001)),
            DE::new(tags::STATUS, VR::US, value!(self.status)),
            DE::new(tags::NUMBER_OF_REMAINING_SUBOPERATIONS, VR::US, value!(self.number_of_remaining_suboperations)),
            DE::new(tags::NUMBER_OF_COMPLETED_SUBOPERATIONS, VR::US, value!(self.number_of_completed_suboperations)),
            DE::new(tags::NUMBER_OF_FAILED_SUBOPERATIONS, VR::US, value!(self.number_of_failed_suboperations)),
            DE::new(tags::NUMBER_OF_WARNING_SUBOPERATIONS, VR::US, value!(self.number_of_warning_suboperations))
        ])
    }
}
#[derive(Builder)]
pub struct CMoveRsp<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-MOVE Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// Shall be set to the DICOM AE Title of the destination DICOM AE to which the C-STORE sub-operations are being performed.,
    pub move_destination: Option<&'a str>,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: u16,
    /// The number of remaining C-STORE sub-operations to be invoked for this C-MOVE operation.,
    pub number_of_remaining_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that have completed successfully.,
    pub number_of_completed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that have failed.,
    pub number_of_failed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that generated warning responses.,
    pub number_of_warning_suboperations: Option<u16>
}
impl<'a> Command for CMoveRsp<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_MOVE_RSP as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::MOVE_DESTINATION, VR::AE, value!(self.move_destination)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101)),
            DE::new(tags::STATUS, VR::US, value!(self.status)),
            DE::new(tags::NUMBER_OF_REMAINING_SUBOPERATIONS, VR::US, value!(self.number_of_remaining_suboperations)),
            DE::new(tags::NUMBER_OF_COMPLETED_SUBOPERATIONS, VR::US, value!(self.number_of_completed_suboperations)),
            DE::new(tags::NUMBER_OF_FAILED_SUBOPERATIONS, VR::US, value!(self.number_of_failed_suboperations)),
            DE::new(tags::NUMBER_OF_WARNING_SUBOPERATIONS, VR::US, value!(self.number_of_warning_suboperations))
        ])
    }
}
#[derive(Builder)]
pub struct CMoveCncl<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-MOVE Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Priority for the request
    #[builder(default = Priority::Medium)]
    priority: Priority,
    /// Shall be set to the DICOM AE Title of the destination DICOM AE to which the C-STORE sub-operations are being performed.,
    pub move_destination: Option<&'a str>,
    /// The value of this field depends upon the status type. Annex C defines the encoding of the status types defined in the service definition.,
    pub status: Option<u16>,
    /// The number of remaining C-STORE sub-operations to be invoked for this C-MOVE operation.,
    pub number_of_remaining_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that have completed successfully.,
    pub number_of_completed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that have failed.,
    pub number_of_failed_suboperations: Option<u16>,
    /// The number of C-STORE sub-operations invoked by this C-MOVE operation that generated warning responses.,
    pub number_of_warning_suboperations: Option<u16>
}
impl<'a> Command for CMoveCncl<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_CANCEL_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::PRIORITY, VR::US, value!(self.priority as u16)),
            DE::new(tags::MOVE_DESTINATION, VR::AE, value!(self.move_destination)),
            DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101)),
            DE::new(tags::STATUS, VR::US, value!(self.status)),
            DE::new(tags::NUMBER_OF_REMAINING_SUBOPERATIONS, VR::US, value!(self.number_of_remaining_suboperations)),
            DE::new(tags::NUMBER_OF_COMPLETED_SUBOPERATIONS, VR::US, value!(self.number_of_completed_suboperations)),
            DE::new(tags::NUMBER_OF_FAILED_SUBOPERATIONS, VR::US, value!(self.number_of_failed_suboperations)),
            DE::new(tags::NUMBER_OF_WARNING_SUBOPERATIONS, VR::US, value!(self.number_of_warning_suboperations))
        ])
    }
}
#[derive(Builder)]
pub struct CEchoRq<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: u16,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-ECHO-RQ Message.,
    pub message_id_being_responded_to: Option<u16>,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: &'a str,
    /// Indicates the status of the response. It shall have a value of Success.,
    pub status: Option<u16>
}
impl<'a> Command for CEchoRq<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_ECHO_RQ as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::STATUS, VR::US, value!(self.status))
        ])
    }
}
#[derive(Builder)]
pub struct CEchoRsp<'a> {
    /// Implementation-specific value that distinguishes this Message from other Messages.,
    pub message_id: Option<u16>,
    /// Shall be set to the value of the Message ID (0000,0110) field used in associated C-ECHO-RQ Message.,
    pub message_id_being_responded_to: u16,
    /// SOP Class UID associated with this operation.,
    pub affected_sop_class_uid: Option<&'a str>,
    /// Indicates the status of the response. It shall have a value of Success.,
    pub status: u16
}
impl<'a> Command for CEchoRsp<'a> {
    fn command_field(&self) -> u16 {
        CommandField::C_ECHO_RSP as u16
    }

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::MESSAGE_ID, VR::US, value!(self.message_id)),
            DE::new(tags::MESSAGE_ID_BEING_RESPONDED_TO, VR::US, value!(self.message_id_being_responded_to)),
            DE::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, value!(self.affected_sop_class_uid)),
            DE::new(tags::STATUS, VR::US, value!(self.status))
        ])
    }
}
