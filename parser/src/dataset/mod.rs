//! Interpretation of DICOM data sets as streams of tokens.
use dicom_core::header::{DataElementHeader, Length};
use dicom_core::value::{DicomValueType, PrimitiveValue};
use dicom_core::Tag;
use std::fmt;

pub mod read;
pub mod write;

pub use self::read::DataSetReader;
pub use self::write::DataSetWriter;

/// A token of a DICOM data set stream. This is part of the interpretation of a
/// data set as a stream of symbols, which may either represent data headers or
/// actual value data.
#[derive(Debug, Clone)]
pub enum DataToken {
    /// A data header of a primitive value.
    ElementHeader(DataElementHeader),
    /// The beginning of a sequence element.
    SequenceStart { tag: Tag, len: Length },
    /// The ending delimiter of a sequence.
    SequenceEnd,
    /// The beginning of a new item in the sequence.
    ItemStart { len: Length },
    /// The ending delimiter of an item.
    ItemEnd,
    /// A primitive data element value.
    PrimitiveValue(PrimitiveValue),
}

impl fmt::Display for DataToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DataToken::PrimitiveValue(ref v) => write!(f, "PrimitiveValue({:?})", v.value_type()),
            other => write!(f, "{:?}", other),
        }
    }
}

/// This implementation treats undefined lengths as equal.
impl PartialEq<Self> for DataToken {
    fn eq(&self, other: &Self) -> bool {
        use DataToken::*;
        match (self, other) {
            (ElementHeader(DataElementHeader { tag: tag1, vr: vr1, len: len1 }), ElementHeader(DataElementHeader { tag: tag2, vr: vr2, len: len2 })) => {
                tag1 == tag2 && vr1 == vr2 && len1.inner_eq(*len2)
            }
            (SequenceStart { tag: tag1, len: len1 }, SequenceStart { tag: tag2, len: len2 }) => {
                tag1 == tag2 && len1.inner_eq(*len2)
            },
            (ItemStart {len: len1}, ItemStart {len: len2}) => {
                len1.inner_eq(*len2)
            },
            (PrimitiveValue(v1), PrimitiveValue(v2)) => { v1 == v2 }
            (ItemEnd, ItemEnd) | (SequenceEnd, SequenceEnd) => true,
            _ => false,
        }
    }
}

/// The type of delimiter: sequence or item.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SeqTokenType {
    Sequence,
    Item,
}
