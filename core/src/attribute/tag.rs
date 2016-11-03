//! This module contains the concept of DICOM attribute tag. A proper Tag
//! data structure is provided, containing a tuple of two unsigned 16-bit
//! integers, but both `(u16, u16)` and `[u16; 2]` can be efficiently
//! converted to this type.

use std::fmt;
use std::cmp::Ordering;

/// Idiomatic alias for a tag's group number (always an unsigned 16-bit integer)
pub type GroupNumber = u16;
/// Idiomatic alias for a tag's element number (always an unsigned 16-bit integer)
pub type ElementNumber = u16;

#[cfg(test)]
mod tests {
    use super::Tag;

    #[test]
    fn tag_from_u16_pair() {
        let t = Tag::from((0x0010u16, 0x0020u16));
        assert_eq!(0x0010u16, t.group());
        assert_eq!(0x0020u16, t.element());
    }

    #[test]
    fn tag_from_u16_array() {
        let t = Tag::from([0x0010u16, 0x0020u16]);
        assert_eq!(0x0010u16, t.group());
        assert_eq!(0x0020u16, t.element());
    }
}

/// The data type for DICOM data element tags.
///
/// Since  types will not have a monomorphized tag, and so will only support
/// a (group, element) pair. For this purpose, `Tag` also provides a method
/// for converting it to a tuple.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Clone, Copy)]
pub struct Tag (pub GroupNumber,  pub ElementNumber);

impl Tag {
    /// Getter for the tag's group value.
    #[inline]
    pub fn group(&self) -> GroupNumber { self.0 }

    /// Getter for the tag's element value.
    #[inline]
    pub fn element(&self) -> ElementNumber { self.1 }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "({:04X},{:04X})", self.0, self.1)
    }
}

impl PartialEq<(u16, u16)> for Tag {
    fn eq(&self, other: &(u16, u16)) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl PartialEq<[u16; 2]> for Tag {
    fn eq(&self, other: &[u16; 2]) -> bool {
        self.0 == other[0] && self.1 == other[1]
    }
}

/// This implementation tests for group element equality.
impl PartialEq<u16> for Tag {
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}

/// This implementation tests for this group
/// element's order relative to the given group element number.
impl PartialOrd<u16> for Tag {
    fn partial_cmp(&self, other: &u16) -> Option<Ordering> {
        Some(self.0.cmp(other))
    }
}

impl From<(u16, u16)> for Tag {
    #[inline]
    fn from(value: (u16, u16)) -> Tag {
        Tag(value.0, value.1)
    }
}

impl From<[u16; 2]> for Tag {
    #[inline]
    fn from(value: [u16; 2]) -> Tag {
        Tag(value[0], value[1])
    }
}
