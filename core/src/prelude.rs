//! Prelude module.
//!
//! You may import all symbols within for convenient usage of this library.
//!
//! # Example
//!
//! ```ignore
//! use dicom_core::prelude::*;
//! ```

pub use crate::{dicom_value, DataElement, DicomValue, Tag, VR};
pub use crate::{header::HasLength as _, DataDictionary as _};
pub use crate::value::{AsRange as _, DicomDate, DicomTime, DicomDateTime};
