//! Baseline attribute operation implementations.
//!
//! See the [`dicom_encoding::ops`] module
//! for more information.

use dicom_core::value::{ModifyValueError, ValueType};
use dicom_encoding::ops::{ApplyOp, AttributeOp};
use snafu::Snafu;

use crate::FileDicomObject;

/// An error which may occur when applying an attribute operation to an object.
#[derive(Debug, Snafu)]
#[non_exhaustive]
#[snafu(visibility(pub(crate)))]
pub enum ApplyError {
    /// Incompatible source element type {kind:?} for extension
    IncompatibleTypes {
        /// the source element value type
        kind: ValueType,
    },
    /// Illegal removal of mandatory attribute
    Mandatory,
    /// Could not modify source element type through extension
    Modify { source: ModifyValueError },
    /// Illegal extension of fixed cardinality attribute
    IllegalExtend,
    /// Unsupported action
    UnsupportedAction,
    /// Unsupported attribute insertion
    UnsupportedAttribute,
}

/// Result type for when applying attribute operations to an object.
pub type ApplyResult<T = (), E = ApplyError> = std::result::Result<T, E>;

impl<T> ApplyOp for FileDicomObject<T>
where
    T: ApplyOp<Err = ApplyError>,
{
    type Err = ApplyError;

    /// Apply the given attribute operation on this object.
    ///
    /// The operation is delegated to the file meta table
    /// if the selector tag is in group `0002`,
    /// and to the underlying object otherwise.
    ///
    /// See the [`dicom_encoding::ops`] module
    /// for more information.
    fn apply(&mut self, op: AttributeOp) -> ApplyResult {
        if op.tag.0 == 0x0002 {
            self.meta.apply(op)
        } else {
            self.obj.apply(op)
        }
    }
}

#[cfg(test)]
mod tests {
    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_encoding::ops::{AttributeAction, AttributeOp, ApplyOp};

    use crate::{FileMetaTableBuilder, InMemDicomObject};

    /// Attribute operations can be applied on a `FileDicomObject<InMemDicomObject>`
    #[test]
    fn file_dicom_object_can_apply_op() {
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            dicom_dictionary_std::tags::PATIENT_NAME,
            VR::PN,
            PrimitiveValue::from("John Doe"),
        ));

        let mut obj = obj
            .with_meta(
                FileMetaTableBuilder::new()
                    .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                    .media_storage_sop_instance_uid("1.2.23456789")
                    .transfer_syntax("1.2.840.10008.1.2.1"),
            )
            .unwrap();

        // apply operation on main data set
        obj.apply(AttributeOp {
            tag: dicom_dictionary_std::tags::PATIENT_NAME,
            action: AttributeAction::ReplaceStr("Patient^Anonymous".into()),
        })
        .unwrap();

        // contains new patient name
        assert_eq!(
            obj.element(dicom_dictionary_std::tags::PATIENT_NAME)
                .unwrap()
                .value()
                .to_str()
                .unwrap(),
            "Patient^Anonymous",
        );

        // apply operation on file meta information
        obj.apply(AttributeOp {
            tag: dicom_dictionary_std::tags::MEDIA_STORAGE_SOP_INSTANCE_UID,
            action: AttributeAction::ReplaceStr(
                "2.25.153241429675951194530939969687300037165".into(),
            ),
        })
        .unwrap();

        // file meta table contains new SOP instance UID
        assert_eq!(
            obj.meta().media_storage_sop_instance_uid(),
            "2.25.153241429675951194530939969687300037165",
        );
    }
}
