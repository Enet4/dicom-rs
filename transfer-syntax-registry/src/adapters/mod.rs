//! Root module for extended pixel data adapters.
//!
//! Additional support for certain transfer syntaxes
//! can be added via Cargo features.
//!
//! - [`jpeg`](jpeg) provides native JPEG decoding
//!   (baseline and lossless)
//!   and encoding (baseline).
//!   Requires the `jpeg` feature,
//!   enabled by default.
//! - [`jpeg2k`](jpeg2k) contains JPEG 2000 support,
//!   which is currently available through [OpenJPEG].
//!   The `openjp2` feature provides native JPEG 2000 decoding
//!   via the [Rust port of OpenJPEG][OpenJPEG-rs],
//!   which works on Linux and Mac OS, but not on Windows.
//!   Alternatively, enable the `openjpeg-sys` feature
//!   to statically link to the OpenJPEG reference implementation.
//!   `openjp2` is enabled by the feature `native`.
//!   To build on Windows, enable `native_windows` instead.
//! - [`jpegxl`](jpegxl) provides JPEG XL decoding and encoding,
//!   through `jxl-oxide` and `zune-jpegxl`, respectively.
//! - [`rle_lossless`](rle_lossless) provides native RLE lossless decoding.
//!   Requires the `rle` feature,
//!   enabled by default.
//!
//! [OpenJPEG]: https://github.com/uclouvain/openjpeg
//! [OpenJPEG-rs]: https://crates.io/crates/openjp2
#[cfg(feature = "jpeg")]
pub mod jpeg;
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub mod jpeg2k;
#[cfg(feature = "charls")]
pub mod jpegls;
#[cfg(feature = "jpegxl")]
pub mod jpegxl;
#[cfg(feature = "rle")]
pub mod rle_lossless;

pub mod uncompressed;

/// **Note:** This module is a stub.
/// Enable the `jpeg` feature to use this module.
#[cfg(not(feature = "jpeg"))]
pub mod jpeg {}

/// **Note:** This module is a stub.
/// Enable either `openjp2` or `openjpeg-sys` to use this module.
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub mod jpeg2k {}

/// **Note:** This module is a stub.
/// Enable the `rle` feature to use this module.
#[cfg(not(feature = "rle"))]
pub mod rle {}

/// **Note:** This module is a stub.
/// Enable the `charls` feature to use this module.
#[cfg(not(feature = "charls"))]
pub mod jpegls {}

/// **Note:** This module is a stub.
/// Enable the `jpegxl` feature to use this module.
#[cfg(not(feature = "jpegxl"))]
pub mod jpegxl {}
