//! Module containing the DICOM Transfer Syntax data structure and related methods.
use data_element::decode::Decode;
use data_element::decode::get_decoder;
use std::io::Read;

/// Enum type for a transfer syntax identifier.
/// This enum will only contain the transfer syntaxes specified in the standard
/// (version 2016a). For custom transfer syntaxes, consider rolling your own
/// enumerate and element decoder factory.
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

/// Retrieve the transfer syntax identified by its respective UID.
/// This function will only provide the transfer syntaxes specified in the standard
/// (version 2016a). For custom transfer syntaxes, consider rolling your own
/// enumerate and element decoder factory.
pub fn from_uid(uid: &str) -> Option<TransferSyntax> {
    match uid {
        "1.2.840.10008.1.2" => Some(TransferSyntax::ImplicitVRLittleEndian),
        "1.2.840.10008.1.2.1" => Some(TransferSyntax::ExplicitVRLittleEndian),
        "1.2.840.10008.1.2.​1.​99" => Some(TransferSyntax::DeflatedExplicitVRLittleEndian),
        "1.2.840.10008.1.​2.​2" => Some(TransferSyntax::ExplicitVRBigEndian),
        "1.2.840.10008.1.2.​4.​50" => Some(TransferSyntax::JPEGBaseline),
        // TODO put the rest of them here
        _ => None,
    }
}

impl TransferSyntax {

    /// Retrieve the appropriate data element decoder for this transfer syntax.
    /// Can yield none if the core library does not support it at the moment.
    pub fn get_decoder<'s, S: Read + ?Sized + 's>(&self) -> Option<Box<Decode<Source = S> + 's>> {
        get_decoder(*self)
    }
}

impl Default for TransferSyntax {
    fn default() -> TransferSyntax {
        TransferSyntax::ImplicitVRLittleEndian
    }
}


