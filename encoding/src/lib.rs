#![deny(trivial_numeric_casts, unsafe_code, unstable_features)]
#![warn(
    missing_debug_implementations,
    unused_qualifications,
    unused_import_braces
)]
#![allow(clippy::derive_partial_eq_without_eq)]
//! DICOM encoding and decoding primitives.
//!
//! This crate provides interfaces and data structures for reading and writing
//! data in accordance to the DICOM standard. This crate also hosts the concept
//! of [transfer syntax specifier], which can be used to produce DICOM encoders
//! and decoders at run-time.
//!
//! For the time being, all APIs are based on synchronous I/O.
//!
//! [transfer syntax specifier]: ./transfer_syntax/index.html

pub mod adapters;
pub mod decode;
pub mod encode;
pub mod ops;
pub mod text;
pub mod transfer_syntax;

pub use adapters::NeverPixelAdapter;
pub use byteordered::Endianness;
pub use decode::Decode;
pub use encode::Encode;
pub use transfer_syntax::Codec;
pub use transfer_syntax::DataRWAdapter;
pub use transfer_syntax::NeverAdapter;
pub use transfer_syntax::TransferSyntax;
pub use transfer_syntax::TransferSyntaxIndex;
