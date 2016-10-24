//! This module contains all DICOM data element encoding logic.
use error::Result;
use std::io::Write;
use data_element::DataElement;

/// Type trait for a data element encoder.
pub trait Encode {
    /// Encode and write a data element to the given destination.
    fn encode_element<W: Write, DE: DataElement>(&self, de: DE, to: &mut W) -> Result<()>;

    /// Encode and write a DICOM sequence item header to the given destination.
    fn encode_item<W: Write>(&self, len: u32, to: &mut W) -> Result<()>;

    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter<W: Write>(&self, to: &mut W) -> Result<()>;

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter<W: Write>(&self, to: &mut W) -> Result<()>;
}
