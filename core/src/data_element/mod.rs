//! This modules contains everything needed to access and manipulate DICOM data elements.
use attribute::ValueRepresentation;

pub mod decode;
pub mod encode;
pub mod text;
mod explicit_le;
mod implicit_le;
mod explicit_be;
use error::{Result, Error};
use attribute::value::DicomValue;

/// A trait for a data type containing a DICOM header.
pub trait Header {
    /// Retrieve the element's tag as a `(group, element)` tuple.
    fn tag(&self) -> (u16, u16);

    /// Retrieve the value data's length as specified by the data element.
    /// According to the standard, this can be 0xFFFFFFFFu32 if the length is undefined,
    /// which can be the case for sequence elements.
    fn len(&self) -> u32;
}

/// A generic trait for any data type that can represent
/// a DICOM data element.
#[derive(Debug, PartialEq, Clone)]
pub struct DataElement {
    header: DataElementHeader,
    value: DicomValue,
}

impl Header for DataElement {
    #[inline]
    fn tag(&self) -> (u16, u16) {
        self.header.tag()
    }

    #[inline]
    fn len(&self) -> u32 {
        self.header.len()
    }
}

impl DataElement {
    /// Retrieve the element's value representation, which can be unknown.
    pub fn vr(&self) -> ValueRepresentation {
        self.header.vr()
    }


    /// Retrieve the data value.
    pub fn value(&self) -> &DicomValue {
        &self.value
    }
}

/// A data structure for a data element header, containing
/// a tag, value representation and specified length.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct DataElementHeader {
    tag: (u16, u16),
    vr: ValueRepresentation,
    len: u32,
}

impl Header for DataElementHeader {
    fn tag(&self) -> (u16, u16) {
        self.tag
    }

    fn len(&self) -> u32 {
        self.len
    }
}

impl DataElementHeader {
    /// Create a new data element header with the given properties.
    /// This is just a trivial constructor.
    pub fn new(tag: (u16, u16), vr: ValueRepresentation, len: u32) -> DataElementHeader {
        DataElementHeader {
            tag: tag,
            vr: vr,
            len: len,
        }
    }

    /// Retrieve the element's value representation, which can be unknown.
    pub fn vr(&self) -> ValueRepresentation {
        self.vr
    }
}


/// Data type for describing a sequence item data element.
/// If the element represents an item, it will also contain
/// the specified length.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SequenceItemHeader {
    /// The cursor contains an item.
    Item {
        /// the length of the item in bytes (can be 0xFFFFFFFF if undefined)
        len: u32,
    },
    /// The cursor read an item delimiter.
    /// The element ends here and should not be read any further.
    ItemDelimiter,
    /// The cursor read a sequence delimiter.
    /// The element ends here and should not be read any further.
    SequenceDelimiter,
}

impl SequenceItemHeader {
    /// Create a sequence item header using the element's raw properties.
    /// An error can be raised if the given properties do not relate to a
    /// sequence item, a sequence item delimiter or a sequence delimiter.
    pub fn new(tag: (u16, u16), len: u32) -> Result<SequenceItemHeader> {
        match tag {
            (0xFFFE, 0xE000) => {
                // item
                Ok(SequenceItemHeader::Item { len: len })
            }
            (0xFFFE, 0xE00D) => {
                // item delimiter
                // delimiters should not have a positive length
                if len > 0 {
                    Err(Error::UnexpectedDataValueLength)
                } else {
                    Ok(SequenceItemHeader::ItemDelimiter)
                }
            }
            (0xFFFE, 0xE0DD) => {
                // sequence delimiter
                Ok(SequenceItemHeader::SequenceDelimiter)
            }
            _ => Err(Error::UnexpectedElement),
        }
    }

    /// Retrieve the sequence item's attribute tag.
    pub fn tag(&self) -> (u16, u16) {
        match *self {
            SequenceItemHeader::Item { .. } => (0xFFFE, 0xE000),
            SequenceItemHeader::ItemDelimiter => (0xFFFE, 0xE00D),
            SequenceItemHeader::SequenceDelimiter => (0xFFFE, 0xE0DD),
        }
    }

    /// Retrieve the sequence item's length (0 in case of a delimiter).
    pub fn len(&self) -> u32 {
        match *self {
            SequenceItemHeader::Item { len } => len,
            SequenceItemHeader::ItemDelimiter |
            SequenceItemHeader::SequenceDelimiter => 0,
        }
    }
}
