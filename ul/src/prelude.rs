//! Prelude module for easy inclusion of important types
//!
//! This module re-exports all association traits as underscore imports. 
//! 
//! # Usage
//!
//! ```ignore
//! use dicom_ul::prelude::*;
//! ```
#[cfg(feature = "async")]
pub use crate::association::AsyncAssociation as _;
pub use crate::association::{Association as _, SyncAssociation as _};
