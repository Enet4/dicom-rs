//! This module contains the concept of a DICOM data dictionary.
//!
//! The standard data dictionary is available in the `dicom-std-dict` crate.

pub mod stub;

use crate::header::{Tag, VR};
use snafu::{ensure, Backtrace, OptionExt, ResultExt, Snafu};
use std::fmt::Debug;
use std::str::FromStr;

/// Specification of a range of tags pertaining to an attribute.
/// Very often, the dictionary of attributes indicates a unique `(group,elem)`
/// for a specific attribute, but occasionally a range of groups or elements
/// is indicated instead (e.g. _Pixel Data_ is associated with ).
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TagRange {
    /// Only a specific tag
    Single(Tag),
    /// The two rightmost digits of the _group_ portion are open:
    /// `(GGxx,EEEE)`
    Group100(Tag),
    /// The two rightmost digits of the _element_ portion are open:
    /// `(GGGG,EExx)`
    Element100(Tag),
}

impl TagRange {
    /// Retrieve the inner tag representation of this range.
    pub fn inner(self) -> Tag {
        match self {
            TagRange::Single(tag) => tag,
            TagRange::Group100(tag) => tag,
            TagRange::Element100(tag) => tag,
        }
    }
}

/// An error returned when parsing an invalid tag range.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum TagRangeParseError {
    #[snafu(display("Not enough tag components, expected tag (group, element)"))]
    MissingTag { backtrace: Backtrace },
    #[snafu(display("Not enough tag components, expected tag element"))]
    MissingTagElement { backtrace: Backtrace },
    #[snafu(display(
        "tag component `group` has an invalid length: got {} but must be 4",
        got
    ))]
    InvalidGroupLength { got: usize, backtrace: Backtrace },
    #[snafu(display(
        "tag component `element` has an invalid length: got {} but must be 4",
        got
    ))]
    InvalidElementLength { got: usize, backtrace: Backtrace },
    #[snafu(display("unsupported tag range"))]
    UnsupportedTagRange { backtrace: Backtrace },
    #[snafu(display("invalid tag component `group`"))]
    InvalidTagGroup {
        backtrace: Backtrace,
        source: std::num::ParseIntError,
    },
    #[snafu(display("invalid tag component `element`"))]
    InvalidTagElement {
        backtrace: Backtrace,
        source: std::num::ParseIntError,
    },
}

impl FromStr for TagRange {
    type Err = TagRangeParseError;

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('(') && s.ends_with(')') {
            s = &s[1..s.len() - 1];
        }
        let mut parts = s.split(',');
        let group = parts.next().context(MissingTagSnafu)?;
        let elem = parts.next().context(MissingTagElementSnafu)?;
        ensure!(
            group.len() == 4,
            InvalidGroupLengthSnafu { got: group.len() }
        );
        ensure!(
            elem.len() == 4,
            InvalidElementLengthSnafu { got: elem.len() }
        );

        match (&group.as_bytes()[2..], &elem.as_bytes()[2..]) {
            (b"xx", b"xx") => UnsupportedTagRangeSnafu.fail(),
            (b"xx", _) => {
                // Group100
                let group =
                    u16::from_str_radix(&group[..2], 16).context(InvalidTagGroupSnafu)? << 8;
                let elem = u16::from_str_radix(elem, 16).context(InvalidTagElementSnafu)?;
                Ok(TagRange::Group100(Tag(group, elem)))
            }
            (_, b"xx") => {
                // Element100
                let group = u16::from_str_radix(group, 16).context(InvalidTagGroupSnafu)?;
                let elem =
                    u16::from_str_radix(&elem[..2], 16).context(InvalidTagElementSnafu)? << 8;
                Ok(TagRange::Element100(Tag(group, elem)))
            }
            (_, _) => {
                // single element
                let group = u16::from_str_radix(group, 16).context(InvalidTagGroupSnafu)?;
                let elem = u16::from_str_radix(elem, 16).context(InvalidTagElementSnafu)?;
                Ok(TagRange::Single(Tag(group, elem)))
            }
        }
    }
}

/** Type trait for a dictionary of DICOM attributes. Attribute dictionaries provide the
 * means to convert a tag to an alias and vice versa, as well as a form of retrieving
 * additional information about the attribute.
 *
 * The methods herein have no generic parameters, so as to enable being
 * used as a trait object.
 */
pub trait DataDictionary: Debug {
    /// The type of the dictionary entry.
    type Entry: DictionaryEntry;

    /// Fetch an entry by its usual alias (e.g. "PatientName" or "SOPInstanceUID").
    /// Aliases are usually case sensitive and not separated by spaces.
    fn by_name(&self, name: &str) -> Option<&Self::Entry>;

    /// Fetch an entry by its tag.
    fn by_tag(&self, tag: Tag) -> Option<&Self::Entry>;

    /// Use this data element dictionary to interpret a DICOM tag.
    ///
    /// This method accepts tags in any of the following formats:
    ///
    /// - `(gggg,eeee)`:
    ///   a 4-digit hexadecimal group part
    ///   and a 4-digit hexadecimal element part
    ///   surrounded by parentheses
    /// - `gggg,eeee`:
    ///   a 4-digit hexadecimal group part
    ///   and a 4-digit hexadecimal element part
    ///   not surrounded by parentheses
    /// - _`KeywordName`_:
    ///   an exact match (case sensitive) by DICOM tag keyword
    /// 
    /// When failing to identify the intended syntax or the tag keyword,
    /// `None` is returned.
    fn parse_tag(&self, tag: &str) -> Option<Tag> {
        tag.parse().ok().or_else(|| {
            // look for tag in standard data dictionary
            self.by_name(tag).map(|e| e.tag())
        })
    }
}

/// The dictionary entry data type, representing a DICOM attribute.
pub trait DictionaryEntry {
    /// The full possible tag range of this attribute.
    fn tag_range(&self) -> TagRange;
    /// The attribute single tag.
    fn tag(&self) -> Tag {
        match self.tag_range() {
            TagRange::Single(tag) => tag,
            TagRange::Group100(tag) => tag,
            TagRange::Element100(tag) => tag,
        }
    }
    /// The alias of the attribute, with no spaces, usually in UpperCamelCase.
    fn alias(&self) -> &str;
    /// The _typical_ value representation of the attribute.
    /// In some edge cases, an element might not have this VR.
    fn vr(&self) -> VR;
}

/// A data type for a dictionary entry with full ownership.
#[derive(Debug, PartialEq, Clone)]
pub struct DictionaryEntryBuf {
    /// The attribute tag range
    pub tag: TagRange,
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: String,
    /// The _typical_  value representation of the attribute
    pub vr: VR,
}

impl DictionaryEntry for DictionaryEntryBuf {
    fn tag_range(&self) -> TagRange {
        self.tag
    }
    fn alias(&self) -> &str {
        self.alias.as_str()
    }
    fn vr(&self) -> VR {
        self.vr
    }
}

/// A data type for a dictionary entry with a string slice for its alias.
#[derive(Debug, PartialEq, Clone)]
pub struct DictionaryEntryRef<'a> {
    /// The attribute tag or tag range
    pub tag: TagRange,
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: &'a str,
    /// The _typical_  value representation of the attribute
    pub vr: VR,
}

impl<'a> DictionaryEntry for DictionaryEntryRef<'a> {
    fn tag_range(&self) -> TagRange {
        self.tag
    }
    fn alias(&self) -> &str {
        self.alias
    }
    fn vr(&self) -> VR {
        self.vr
    }
}

/// Utility data structure that resolves to a DICOM attribute tag
/// at a later time.
#[derive(Debug, Clone)]
pub struct TagByName<N, D> {
    dict: D,
    name: N,
}

impl<N, D> TagByName<N, D>
where
    N: AsRef<str>,
    D: DataDictionary,
{
    /// Create a tag resolver by name using the given dictionary.
    pub fn new(dictionary: D, name: N) -> TagByName<N, D> {
        TagByName {
            dict: dictionary,
            name,
        }
    }
}

impl<N, D> From<TagByName<N, D>> for Option<Tag>
where
    N: AsRef<str>,
    D: DataDictionary,
{
    fn from(tag: TagByName<N, D>) -> Option<Tag> {
        tag.dict.by_name(tag.name.as_ref()).map(|e| e.tag())
    }
}

#[cfg(test)]
mod tests {
    use super::TagRange;
    use crate::header::Tag;

    #[test]
    fn test_parse_tag_range() {
        let tag: TagRange = "(1234,5678)".parse().unwrap();
        assert_eq!(tag, TagRange::Single(Tag(0x1234, 0x5678)));

        let tag: TagRange = "1234,5678".parse().unwrap();
        assert_eq!(tag, TagRange::Single(Tag(0x1234, 0x5678)));

        let tag: TagRange = "12xx,5678".parse().unwrap();
        assert_eq!(tag, TagRange::Group100(Tag(0x1200, 0x5678)));

        let tag: TagRange = "1234,56xx".parse().unwrap();
        assert_eq!(tag, TagRange::Element100(Tag(0x1234, 0x5600)));
    }
}
