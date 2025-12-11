//! Root module for extended pixel data adapters.
//!
//! Additional support for certain transfer syntaxes
//! can be added via Cargo features.
//!
//! - [`jpeg`] provides native JPEG decoding
//!   (baseline and lossless)
//!   and encoding (baseline).
//!   Requires the `jpeg` feature,
//!   enabled by default.
//! - [`jpeg2k`] contains JPEG 2000 support,
//!   which is currently available through [OpenJPEG].
//!   Use feature `openjpeg-sys`
//!   to statically link to the OpenJPEG reference implementation,
//!   thus providing JPEG 2000 decoding.
//!   Alternatively, feature `openjp2` provides native JPEG 2000 decoding
//!   via the [Rust port of OpenJPEG][OpenJPEG-rs],
//!   which is maintained separately.
//! - [`jpegxl`] contains JPEG XL support.
//!   `jxl-oxide` enables decoding via [jxl-oxide],
//!   and `zune-jpegxl` adds lossless encoding via [zune-jpegxl].
//!   Currently, the `jpegxl` feature enables both.
//! - [`rle_lossless`] provides native RLE lossless decoding.
//!   Requires the `rle` feature,
//!   enabled by default.
//!
//! [OpenJPEG]: https://github.com/uclouvain/openjpeg
//! [OpenJPEG-rs]: https://crates.io/crates/openjp2
//! [jxl-oxide]: https://crates.io/crates/jxl-oxide
//! [zune-jpegxl]: https://crates.io/crates/zune-jpegxl
#[cfg(feature = "jpeg")]
pub mod jpeg;
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub mod jpeg2k;
#[cfg(feature = "charls")]
pub mod jpegls;
#[cfg(any(feature = "jxl-oxide", feature = "zune-jpegxl"))]
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
/// Enable the features `jxl-oxide` or `zune-jpegxl` to use this module.
#[cfg(not(any(feature = "jxl-oxide", feature = "zune-jpegxl")))]
pub mod jpegxl {}
