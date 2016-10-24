//! This module contains all DICOM data element encoding logic.
use error::Result;
use std::io::Write;
use data_element::DataElementHeader;

/// Type trait for a data element encoder.
pub trait Encode {
    /// The encoding destination's data type.
    type Writer: Write + ?Sized;

    /// Encode and write a data element header to the given destination.
    fn encode_element_header(&self, de: DataElementHeader, to: &mut Self::Writer) -> Result<()>;

    /// Encode and write a DICOM sequence item header to the given destination.
    fn encode_item_header(&self, len: u32, to: &mut Self::Writer) -> Result<()>;

    /// Encode and write a DICOM sequence item delimiter to the given destination.
    fn encode_item_delimiter(&self, to: &mut Self::Writer) -> Result<()>;

    /// Encode and write a DICOM sequence delimiter to the given destination.
    fn encode_sequence_delimiter(&self, to: &mut Self::Writer) -> Result<()>;
}
