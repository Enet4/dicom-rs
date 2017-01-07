//! Module containing the DICOM Transfer Syntax data structure and related methods.
pub mod explicit_le;
pub mod explicit_be;
pub mod implicit_le;

use std::io::{Read,Write};
use data_element::decode::Decode;
use data_element::encode::Encode;

/*
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TransferSyntax {
    /// Implicit VR Little Endian, default
    ImplicitVRLittleEndian,
    /// Explitic VR Little Endian, always used in DICOM file meta info
    ExplicitVRLittleEndian,
    /// Deflated Explicit VR Little Endian
    DeflatedExplicitVRLittleEndian,
    /// (retired)
    ExplicitVRBigEndian,
    /// JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression
    JPEGBaseline,
}
*/

/// Trait for a DICOM transfer syntax. Trait implementers make an entry
/// point for obtaining the decoder and/or encoder that can handle DICOM objects
/// under a particular transfer syntax.
pub trait TransferSyntax {

    /// Retrieve the UID of this transfer syntax.
    fn uid(&self) -> &'static str;

    /// Retrieve the appropriate data element decoder for this transfer syntax.
    /// Can yield none if decoding is not supported.
    fn get_decoder<'s>(&self) -> Option<Box<Decode<Source = (Read + 's)>>> { None }

    /// Retrieve the appropriate data element encoder for this transfer syntax.
    /// Can yield none if encoding is not supported.
    fn get_encoder<'s>(&self) -> Option<Box<Encode<Writer = (Write + 's)>>> { None }
}

/// Retrieve the transfer syntax identified by its respective UID.
/// This function will only provide some of the transfer syntaxes specified in the standard
/// (version 2016a). For custom transfer syntaxes, consider rolling your own
/// enumerate and element decoder factory.
pub fn from_uid(uid: &str) -> Option<Box<TransferSyntax>> {
    match uid {
        "1.2.840.10008.1.2" => Some(Box::new(ImplicitVRLittleEndian)),
        "1.2.840.10008.1.2.1" => Some(Box::new(ExplicitVRLittleEndian)),
        "1.2.840.10008.1.2.​1.​99" => Some(Box::new(DeflatedExplicitVRLittleEndian)),
        "1.2.840.10008.1.​2.​2" => Some(Box::new(ExplicitVRBigEndian)),
        "1.2.840.10008.1.2.​4.​50" => Some(Box::new(JPEGBaseline)),
        // TODO put the rest of them here
        _ => None,
    }
}

/// Retrieve the default transfer syntax.
pub fn default() -> ImplicitVRLittleEndian {
   ImplicitVRLittleEndian
}

/// A concrete encoder for the transfer syntax ExplicitVRLittleEndian
#[derive(Debug, Clone, Copy)]
pub struct ImplicitVRLittleEndian;
impl TransferSyntax for ImplicitVRLittleEndian {
    fn uid(&self) -> &'static str { "1.2.840.10008.1.2" }

    fn get_decoder<'s>(&self) -> Option<Box<Decode<Source = (Read + 's)>>> {
        Some(Box::new(implicit_le::ImplicitVRLittleEndianDecoder::with_default_dict()))
    }
}

/// Transfer syntax: ExplicitVRLittleEndian
#[derive(Debug, Clone, Copy)]
pub struct ExplicitVRLittleEndian;
impl TransferSyntax for ExplicitVRLittleEndian {
    fn uid(&self) -> &'static str { "1.2.840.10008.1.2.1" }

    fn get_decoder<'s>(&self) -> Option<Box<Decode<Source = (Read + 's)>>> {
        Some(Box::new(explicit_le::ExplicitVRLittleEndianDecoder::default()))
    }

    fn get_encoder<'s>(&self) -> Option<Box<Encode<Writer = (Write + 's)>>> {
        Some(Box::new(explicit_le::ExplicitVRLittleEndianEncoder::default()))
    }
}

/// Transfer syntax: ExplicitVRBigEndian
#[derive(Debug, Clone, Copy)]
pub struct ExplicitVRBigEndian;
impl TransferSyntax for ExplicitVRBigEndian {
    fn uid(&self) -> &'static str { "1.2.840.10008.1.2.2" }

    fn get_decoder<'s>(&self) -> Option<Box<Decode<Source = (Read + 's)>>> {
        Some(Box::new(explicit_be::ExplicitVRBigEndianDecoder::default()))
    }

    fn get_encoder<'s>(&self) -> Option<Box<Encode<Writer = (Write + 's)>>> {
        Some(Box::new(explicit_be::ExplicitVRBigEndianEncoder::default()))
    }
}

macro_rules! declare_stub_ts {
    ($name: ident, $uid: expr) => (
        /// Transfer syntax: $name
        #[derive(Debug, Clone, Copy)]
        pub struct $name;
        impl TransferSyntax for $name {
            fn uid(&self) -> &'static str { $uid }
        }
    )
}

declare_stub_ts!(DeflatedExplicitVRLittleEndian, "1.2.840.10008.1.2.1.99");
declare_stub_ts!(JPEGBaseline, "1.2.840.10008.1.2.4.50");
