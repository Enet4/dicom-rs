//! This modules contains everything needed to access and manipulate DICOM data elements.
use attribute::VR;

pub mod decode;
pub mod encode;
pub mod text;
pub mod codec;

use error::{Result, Error};
use attribute::value::DicomValue;
use attribute::tag::Tag;

/// A trait for a data type containing a DICOM header.
pub trait Header {
    /// Retrieve the element's tag as a `(group, element)` tuple.
    fn tag(&self) -> Tag;

    /// Retrieve the value data's length as specified by the data element.
    /// According to the standard, this can be 0xFFFFFFFFu32 if the length is undefined,
    /// which can be the case for sequence elements.
    fn len(&self) -> u32;

    /// Check whether this is the header of an item.
    fn is_item(&self) -> bool {
        self.tag() == Tag(0xFFFE, 0xE000)
    }

    /// Check whether this is the header of an item delimiter.
    fn is_item_delimiter(&self) -> bool {
        self.tag() == Tag(0xFFFE,0x0E0D)
    }

    /// Check whether this is the header of a sequence delimiter.
    fn is_sequence_delimiter(&self) -> bool {
        self.tag() == Tag(0xFFFE, 0xE0DD)
    }
}

/// A possible data element type.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DataElementType {
    /// Simple data.
    Data,
    /// The start of a sequence.
    Sequence,
    /// An item.
    Item,
    /// An item delimiter.
    ItemDelimiter,
    /// A sequence delimiter.
    SequenceDelimiter,
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
    fn tag(&self) -> Tag {
        self.header.tag()
    }

    #[inline]
    fn len(&self) -> u32 {
        self.header.len()
    }
}

impl DataElement {
    /// Retrieve the element's value representation, which can be unknown.
    pub fn vr(&self) -> VR {
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
    tag: Tag,
    vr: VR,
    len: u32,
}

impl Header for DataElementHeader {
    fn tag(&self) -> Tag {
        self.tag
    }

    fn len(&self) -> u32 {
        self.len
    }
}

impl DataElementHeader {
    /// Create a new data element header with the given properties.
    /// This is just a trivial constructor.
    pub fn new<T: Into<Tag>>(tag: T, vr: VR, len: u32) -> DataElementHeader {
        DataElementHeader {
            tag: tag.into(),
            vr: vr,
            len: len,
        }
    }

    /// Retrieve the element's value representation, which can be unknown.
    pub fn vr(&self) -> VR {
        self.vr
    }
}

impl From<SequenceItemHeader> for DataElementHeader {
    fn from(value: SequenceItemHeader) -> DataElementHeader {
        DataElementHeader {
            tag: value.tag(),
            vr: VR::UN,
            len: value.len(),
        }
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
    pub fn new<T: Into<Tag>>(tag: T, len: u32) -> Result<SequenceItemHeader> {
        match tag.into() {
            Tag(0xFFFE, 0xE000) => {
                // item
                Ok(SequenceItemHeader::Item { len: len })
            }
            Tag(0xFFFE, 0xE00D) => {
                // item delimiter
                // delimiters should not have a positive length
                if len > 0 {
                    Err(Error::UnexpectedDataValueLength)
                } else {
                    Ok(SequenceItemHeader::ItemDelimiter)
                }
            }
            Tag(0xFFFE, 0xE0DD) => {
                // sequence delimiter
                Ok(SequenceItemHeader::SequenceDelimiter)
            }
            _ => Err(Error::UnexpectedElement),
        }
    }

    /// Retrieve the sequence item's attribute tag.
    pub fn tag(&self) -> Tag {
        match *self {
            SequenceItemHeader::Item { .. } => Tag(0xFFFE, 0xE000),
            SequenceItemHeader::ItemDelimiter => Tag(0xFFFE, 0xE00D),
            SequenceItemHeader::SequenceDelimiter => Tag(0xFFFE, 0xE0DD),
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
