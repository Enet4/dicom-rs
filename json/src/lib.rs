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
//! which implements serialization via [Serde](serde).
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

mod ser;

pub use crate::ser::{
    DicomJson, to_string, to_value,
};
