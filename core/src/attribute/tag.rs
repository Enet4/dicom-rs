//! This module contains the concept of DICOM element tag. Tags in this library
//! are typically defined as a tuple of two unsigned 16-bit integers, but the
//! `Tag` trait provides facility methods for dealing with tags.
//! This trait is also implemented for `[u16; 2]`.

/// Idiomatic alias for a tag's group number (always an unsigned 16-bit integer)
pub type GroupNumber = u16;
/// Idiomatic alias for a tag's element number (always an unsigned 16-bit integer)
pub type ElementNumber = u16;

#[cfg(test)]
mod tests {
    use super::Tag;

    #[test]
    fn tag_from_u16_pair() {
        let t = (0x0010u16, 0x0020u16);
        assert_eq!(0x0010u16, t.group());
        assert_eq!(0x0020u16, t.element());
    }

    #[test]
    fn tag_from_u16_array() {
        let t: [u16; 2] = [0x0010u16, 0x0020u16];
        assert_eq!(0x0010u16, t.group());
        assert_eq!(0x0020u16, t.element());
    }
}

/// Generic trait for anything that can be interpreted as a DICOM data element tag.
/// Rather than sticking to a struct, importing this trait will automatically allow
/// the use of `(u16, u16)` and `[u16; 2]` bindings as tags, where the first and second
/// elements refer to the group and element values, respectively.
///
/// Certain data types will not have a monomorphized tag, and so will only support
/// a (group, element) pair. For this purpose, `Tag` also provides a method
/// for converting it to a tuple.
pub trait Tag {
    /// Getter for the tag's group value.
    fn group(&self) -> GroupNumber;
    /// Getter for the tag's element value.
    fn element(&self) -> ElementNumber;

    /// Transform this tag into a tag tuple. This is useful for passin
    #[inline]
    fn into(self) -> (GroupNumber, ElementNumber)
        where Self: Sized
    {
        (self.group(), self.element())
    }
}

impl Tag for (GroupNumber, ElementNumber) {
    #[inline]
    fn group(&self) -> GroupNumber {
        self.0
    }

    #[inline]
    fn element(&self) -> ElementNumber {
        self.1
    }

    #[inline]
    fn into(self) -> (GroupNumber, ElementNumber) {
        self
    }
}

impl Tag for [u16; 2] {
    #[inline]
    fn group(&self) -> GroupNumber {
        self[0]
    }

    #[inline]
    fn element(&self) -> ElementNumber {
        self[1]
    }
}
