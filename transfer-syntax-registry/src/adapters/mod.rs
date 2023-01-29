//! Root module for extended pixel data adapters.
//! 
//! Additional support for certain transfer syntaxes
//! can be added via Cargo features.
//! 
//! - [`jpeg`](jpeg) provides native JPEG decoding
//!   (baseline and lossless).
//!   Requires the `jpeg` feature,
//!   enabled by default.
//! - [`rle_lossless`](rle_lossless) provides RLE lossless decoding.
//!   Requires the `rle` feature,
//!   enabled by default.

#[cfg(feature = "jpeg")]
pub mod jpeg;
#[cfg(feature = "rle")]
pub mod rle_lossless;

/// **Note:** This module is a stub.
/// Enable the `jpeg` feature to use this module.
#[cfg(not(feature = "jpeg"))]
pub mod jpeg {}

/// **Note:** This module is a stub.
/// Enable the `rle` feature to use this module.
#[cfg(not(feature = "rle"))]
pub mod rle {}
