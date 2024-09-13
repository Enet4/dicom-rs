use dicom_core::{Tag, VR, header::DataElementHeader};
use std::io::Read;

/// Abstraction for any attribute in a DICOM object.
pub trait Attribute<'a> {
    type Error;
    type Reader: 'a + Read;
    type Item: 'a;
    type ItemIter: IntoIterator<Item = Self::Item>;

    /// Retrieve the header information of this attribute.
    fn header(&self) -> DataElementHeader;

    /// Retrieve the value representation.
    fn vr(&self) -> VR {
        self.header().vr
    }

    /// Retrieve the tag.
    fn tag(&self) -> Tag {
        self.header().tag
    }

    /// Read the entire value as a single string.
    fn str(&self) -> Result<&'a str, Self::Error>;

    /// Read the entire value as raw bytes.
    fn raw_bytes(&self) -> Result<&'a [u8], Self::Error>;

    /// Create a new byte reader for the value of this attribute.
    fn stream(&self) -> Result<Self::Reader, Self::Error>;
}
