//! Prelude module.
//!
//! You may import all symbols within for convenient usage of this library.
//!
//! # Example
//!
//! ```ignore
//! use dicom_core::prelude::*;
//! ```

pub use crate::value::{AsRange as _, DicomDate, DicomDateTime, DicomTime};
pub use crate::{DataDictionary as _, header::HasLength as _};
pub use crate::{DataElement, DicomValue, Tag, VR, dicom_value};
