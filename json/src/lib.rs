#![warn(missing_docs)]
//! DICOM JSON module
//!
//!
//! This library provides serialization of DICOM data to JSON
//! and deserialization of JSON to DICOM data,
//! as per the [DICOM standard part 18 chapter F][1].
//!
//! [1]: https://dicom.nema.org/medical/dicom/current/output/chtml/part18/chapter_F.html
//!
//! The easiest path to serialization is in
//! using the functions readily available [`to_string`] and [`to_value`].
//! Alternatively, DICOM data can be enclosed by a [`DicomJson`] value,
//! which implements serialization and deserialization via [Serde](serde).
//!
//! # Example
//!
//! To serialize an object to standard DICOM JSON:
//!
//! ```rust
//! # use dicom_core::{PrimitiveValue, VR};
//! # use dicom_object::mem::{InMemDicomObject, InMemElement};
//! # use dicom_dictionary_std::tags;
//! let obj = InMemDicomObject::from_element_iter([
//!     InMemElement::new(tags::SERIES_DATE, VR::DA, PrimitiveValue::from("20230610")),
//!     InMemElement::new(tags::INSTANCE_NUMBER, VR::IS, PrimitiveValue::from("5")),
//! ]);
//!
//! let json = dicom_json::to_string(&obj)?;
//!
//! assert_eq!(
//!     json,
//!     r#"{"00080021":{"vr":"DA","Value":["20230610"]},"00200013":{"vr":"IS","Value":["5"]}}"#
//! );
//!
//! Ok::<(), serde_json::Error>(())
//! ```
//!
//! Use [`DicomJson`] for greater control on how to serialize it:
//!
//! ```rust
//! # use dicom_core::{PrimitiveValue, VR};
//! # use dicom_object::mem::{InMemDicomObject, InMemElement};
//! # use dicom_dictionary_std::tags;
//! # let obj = InMemDicomObject::from_element_iter([
//! #     InMemElement::new(tags::SERIES_DATE, VR::DA, PrimitiveValue::from("20230610")),
//! #     InMemElement::new(tags::INSTANCE_NUMBER, VR::IS, PrimitiveValue::from("5")),
//! # ]);
//! let dicom_obj = dicom_json::DicomJson::from(&obj);
//! let serialized = serde_json::to_value(dicom_obj)?;
//!
//! assert_eq!(
//!     serialized,
//!     serde_json::json!({
//!         "00080021": {
//!             "vr": "DA",
//!             "Value": [ "20230610" ]
//!         },
//!         "00200013": {
//!             "vr": "IS",
//!             "Value": [ "5" ]
//!         }
//!     }),
//! );
//!
//! Ok::<(), serde_json::Error>(())
//! ```

mod de;
mod ser;

pub use crate::de::{from_reader, from_slice, from_str, from_value};
pub use crate::ser::{to_string, to_string_pretty, to_value, to_vec, to_writer};

/// A wrapper type for DICOM JSON serialization using [Serde](serde).
///
/// Serializing this type will yield JSON data according to the standard.
/// Deserialization from this type
/// will interpret the input data as standard DICOM JSON deserialization.
///
/// # Serialization
///
/// Convert a DICOM data type such as a file, object, or data element
/// using [`From`] or [`Into`],
/// then use a JSON serializer such as the one in [`serde_json`]
/// to serialize it to the intended type.
/// A reference may be used as well,
/// so as to not consume the DICOM data.
///
/// `DicomJson` can serialize:
///
/// - [`InMemDicomObject`][1] as a standard DICOM JSON data set;
/// - [`InMemElement`][2] by writing the VR and value in a single object
///   (note that the tag will not be serialized);
/// - `&[InMemDicomObject]` and `Vec<InMemDicomObject>`
///   will be serialized as an array of DICOM JSON data sets;
/// - [`DefaultDicomObject`][3] will include the attributes from the file meta group.
///   Note however, that this is not conforming to the standard.
///   Obtain the inner data set through [`Deref`][4] (`&*obj`)
///   if you do not wish to include file meta group data.
/// - [`Tag`][5] values are written as a single string
///   in the expected DICOM JSON format `"GGGGEEEE"`
///   where `GGGG` and `EEEE` are the group/element parts
///   in uppercase hexadecimal.
///
/// [1]: dicom_object::InMemDicomObject
/// [2]: dicom_object::mem::InMemElement
/// [3]: dicom_object::DefaultDicomObject
/// [4]: std::ops::Deref
/// [5]: dicom_core::Tag
///
/// ## Example
///
/// ```
/// # use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
/// # use dicom_object::InMemDicomObject;
/// use dicom_json::DicomJson;
///
/// // creating a DICOM object with a single attribute
/// let obj = InMemDicomObject::from_element_iter([
///     DataElement::new(
///         Tag(0x0010, 0x0020),
///         VR::LO,
///         PrimitiveValue::from("ID0001"),
///     )
/// ]);
/// // wrap it with DicomJson
/// let json_obj = DicomJson::from(&obj);
/// // serialize it to a JSON Value
/// let serialized = serde_json::to_value(&json_obj)?;
/// assert_eq!(
///   serialized,
///   serde_json::json!({
///       "00100020": {
///           "vr": "LO",
///           "Value": [ "ID0001" ]
///       }
///   })
/// );
/// # Result::<_, serde_json::Error>::Ok(())
/// ```
///
/// # Deserialization
///
/// Specify the concrete DICOM data type to deserialize to,
/// place it as the type parameter `T` of `DicomJson<T>`,
/// then request to deserialize it.
/// 
/// `DicomJson` can deserialize:
/// 
/// - [`Tag`][5], a string formatted as a DICOM tag;
/// - [`VR`][6], a 2-character string with one of the supported
///   value representation identifiers;
/// - [`InMemDicomObject`][1], expecting a JSON object indexed by tags.
/// 
/// [6]: dicom_core::VR
/// 
/// ## Example
///
/// ```
/// # use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
/// # use dicom_object::InMemDicomObject;
/// use dicom_json::DicomJson;
///
/// // given this JSON data
/// let json_data = r#"{
///     "00100020": {
///         "vr": "LO",
///         "Value": [ "ID0001" ]
///     }
/// }"#;
/// // deserialize to DicomJson, then unwrap it
/// let deserialized: DicomJson<InMemDicomObject> = serde_json::from_str(json_data)?;
/// let obj = deserialized.into_inner();
/// assert_eq!(
///   obj,
///   InMemDicomObject::from_element_iter([
///       DataElement::new(Tag(0x0010, 0x0020), VR::LO, "ID0001"),
///   ]),
/// );
/// # Result::<_, serde_json::Error>::Ok(())
/// ```
///
/// TODO
#[derive(Debug, Clone, PartialEq)]
pub struct DicomJson<T>(T);

impl<T> DicomJson<T> {
    /// Unwrap the DICOM JSON wrapper,
    /// returning the underlying value.
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Obtain a reference to the underlying value.
    pub fn inner(&self) -> &T {
        &self.0
    }
}
