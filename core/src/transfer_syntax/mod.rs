//! Module containing the DICOM Transfer Syntax data structure and related methods.
//! Similar to the DcmCodec in DCMTK, the `TransferSyntax` contains all of the necessary
//! algorithms for decoding and encoding DICOM data in a certain transfer syntax.

pub mod codec;
pub mod explicit_le;
pub mod explicit_be;
pub mod implicit_le;

use std::io::{Read, Write};
use data::decode::basic::BasicDecoder;
use data::decode::Decode;
use data::encode::Encode;
use util::Endianness;

/// A decoder with its type erased.
pub type DynamicDecoder = Box<Decode<Source = Read>>;

/// An encoder with its type erased.
pub type DynamicEncoder = Box<Encode<Writer = Write>>;

/// Trait for a DICOM transfer syntax. Trait implementers make an entry
/// point for obtaining the decoder and/or encoder that can handle DICOM objects
/// under a particular transfer syntax.
pub trait TransferSyntax {
    /// Retrieve the UID of this transfer syntax.
    fn uid(&self) -> &'static str;

    /// Obtain this transfer syntax' expected endianness.
    fn endianness(&self) -> Endianness;

    /// Retrieve the appropriate data element decoder for this transfer syntax.
    /// Can yield none if decoding is not supported.
    fn get_decoder(&self) -> Option<DynamicDecoder> {
        None
    }

    /// Retrieve the appropriate data element encoder for this transfer syntax.
    /// Can yield none if encoding is not supported.
    fn get_encoder(&self) -> Option<DynamicEncoder> {
        None
    }

    /// Obtain a dynamic basic decoder, based on this transfer syntax' expected endianness.
    fn get_basic_decoder(&self) -> BasicDecoder {
        BasicDecoder::from(self.endianness())
    }
}

/// Retrieve the default transfer syntax.
pub fn default() -> ImplicitVRLittleEndian {
    ImplicitVRLittleEndian
}

/// A concrete encoder for the transfer syntax ExplicitVRLittleEndian
#[derive(Debug, Default, Clone, Copy)]
pub struct ImplicitVRLittleEndian;
impl TransferSyntax for ImplicitVRLittleEndian {
    fn uid(&self) -> &'static str {
        "1.2.840.10008.1.2"
    }

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn get_decoder<'s>(&self) -> Option<DynamicDecoder> {
        Some(Box::new(
            implicit_le::ImplicitVRLittleEndianDecoder::default(),
        ))
    }

    fn get_encoder<'w>(&self) -> Option<DynamicEncoder> {
        Some(Box::new(
            implicit_le::ImplicitVRLittleEndianEncoder::default(),
        ))
    }
}

/// Transfer syntax: ExplicitVRLittleEndian
#[derive(Debug, Default, Clone, Copy)]
pub struct ExplicitVRLittleEndian;
impl TransferSyntax for ExplicitVRLittleEndian {
    fn uid(&self) -> &'static str {
        "1.2.840.10008.1.2.1"
    }

    fn endianness(&self) -> Endianness {
        Endianness::LE
    }

    fn get_decoder(&self) -> Option<DynamicDecoder> {
        Some(Box::new(
            explicit_le::ExplicitVRLittleEndianDecoder::default(),
        ))
    }

    fn get_encoder(&self) -> Option<DynamicEncoder> {
        Some(Box::new(
            explicit_le::ExplicitVRLittleEndianEncoder::default(),
        ))
    }
}

/// Transfer syntax: ExplicitVRBigEndian
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExplicitVRBigEndian;
impl TransferSyntax for ExplicitVRBigEndian {
    fn uid(&self) -> &'static str {
        "1.2.840.10008.1.2.2"
    }

    fn endianness(&self) -> Endianness {
        Endianness::BE
    }

    fn get_decoder<'s>(&self) -> Option<DynamicDecoder> {
        Some(Box::new(explicit_be::ExplicitVRBigEndianDecoder::default()))
    }

    fn get_encoder<'w>(&self) -> Option<DynamicEncoder> {
        Some(Box::new(explicit_be::ExplicitVRBigEndianEncoder::default()))
    }
}

macro_rules! declare_stub_ts {
    ($name: ident, $uid: expr) => (
        /// Transfer syntax: $name
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        pub struct $name;
        impl TransferSyntax for $name {
            fn uid(&self) -> &'static str { $uid }

            fn endianness(&self) -> Endianness { Endianness::LE }
        }
    )
}

declare_stub_ts!(DeflatedExplicitVRLittleEndian, "1.2.840.10008.1.2.1.99");
declare_stub_ts!(JPEGBaseline, "1.2.840.10008.1.2.4.50");
