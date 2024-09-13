//! Core data element dictionary types

use std::str::FromStr;

use snafu::{ensure, Backtrace, OptionExt, ResultExt, Snafu};

use crate::{
    ops::{AttributeSelector, AttributeSelectorStep},
    Tag, VR,
};

/// Specification of a range of tags pertaining to an attribute.
/// Very often, the dictionary of attributes indicates a unique
/// group part and element part `(group,elem)`,
/// but occasionally an attribute may cover
/// a range of groups or elements instead.
/// For example,
/// _Overlay Data_ (60xx,3000) has more than one possible tag,
/// since it is part of a repeating group.
/// Moreover, a unique variant is defined for group length tags
/// and another one for private creator tags.
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
    /// Generic group length tag,
    /// refers to any attribute of the form `(GGGG,0000)`,
    /// _save for the following exceptions_
    /// which have their own single tag record:
    ///
    /// - _Command Group Length_ (0000,0000)
    /// - _File Meta Information Group Length_ (0002,0000)
    GroupLength,
    /// Generic private creator tag,
    /// refers to any tag from (GGGG,0010) to (GGGG,00FF),
    /// where `GGGG` is an odd number.
    PrivateCreator,
}

impl TagRange {
    /// Retrieve the inner tag representation of this range.
    ///
    /// Open components are zeroed out.
    /// Returns a zeroed out tag
    /// (equivalent to _Command Group Length_)
    /// if it is a group length tag.
    /// If it is a private creator tag,
    /// this method returns `Tag(0x0009, 0x0010)`.
    pub fn inner(self) -> Tag {
        match self {
            TagRange::Single(tag) => tag,
            TagRange::Group100(tag) => tag,
            TagRange::Element100(tag) => tag,
            TagRange::GroupLength => Tag(0x0000, 0x0000),
            TagRange::PrivateCreator => Tag(0x0009, 0x0010),
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

/// A "virtual" value representation (VR) descriptor
/// which extends the standard enumeration with context-dependent VRs.
///
/// It is used by element dictionary entries to describe circumstances
/// in which the real VR may depend on context.
/// As an example, the _Pixel Data_ attribute
/// can have a value representation of either [`OB`](VR::OB) or [`OW`](VR::OW).
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum VirtualVr {
    /// The value representation is exactly known
    /// and does not depend on context.
    Exact(VR),
    /// Represents a pixel data sample value
    /// with a short magnitude.
    ///
    /// The value representation depends on
    /// the pixel data value sample representation.
    /// If pixel data values are signed
    /// (represented by a _Pixel Representation_ value of `1`),
    /// then values with this virtual VR
    /// should be interpreted as signed 16 bit integers
    /// ([`SS`](VR::SS)),
    /// otherwise they should be interpreted as unsigned 16 bit integers
    /// ([`US`](VR::US)).
    Xs,
    /// Represents overlay data sample values.
    ///
    /// It can be either [`OB`](VR::OB) or [`OW`](VR::OW).
    Ox,
    /// Represents pixel data sample value.
    ///
    /// It can be either [`OB`](VR::OB) or [`OW`](VR::OW).
    Px,
    /// Represents LUT data, which can be [`US`](VR::US) or [`OW`](VR::OW)
    Lt,
}

impl From<VR> for VirtualVr {
    fn from(value: VR) -> Self {
        VirtualVr::Exact(value)
    }
}

impl VirtualVr {
    /// Return the underlying value representation
    /// in the case that it can be unambiguously defined without context.
    pub fn exact(self) -> Option<VR> {
        match self {
            VirtualVr::Exact(vr) => Some(vr),
            _ => None,
        }
    }

    /// Return the underlying value representation,
    /// making a relaxed conversion if it cannot be
    /// accurately resolved without context.
    ///
    /// - [`Xs`](VirtualVr::Xs) is relaxed to [`US`](VR::US)
    /// - [`Ox`](VirtualVr::Ox) is relaxed to [`OW`](VR::OW)
    /// - [`Px`](VirtualVr::Px) is relaxed to [`OW`](VR::OW)
    /// - [`Lt`](VirtualVr::Lt) is relaxed to [`OW`](VR::OW)
    ///
    /// This method is ill-advised for uses where
    /// the corresponding attribute is important.
    pub fn relaxed(self) -> VR {
        match self {
            VirtualVr::Exact(vr) => vr,
            VirtualVr::Xs => VR::US,
            VirtualVr::Ox => VR::OW,
            VirtualVr::Px => VR::OW,
            VirtualVr::Lt => VR::OW,
        }
    }
}

/// An error during attribute selector parsing
#[derive(Debug, Snafu)]
pub struct ParseSelectorError(ParseSelectorErrorInner);

#[derive(Debug, Snafu)]
enum ParseSelectorErrorInner {
    /// missing item index delimiter `[`
    MissingItemDelimiter,
    /// invalid tag or unrecognized keyword
    ParseKey,
    /// invalid item index, should be an unsigned integer
    ParseItemIndex,
    /// last selector step should select a plain tag
    ParseLeaf,
}

/// Type trait for a dictionary of DICOM attributes.
///
/// The main purpose of an attribute dictionary is
/// to retrieve a record containing additional information about a data element,
/// in one of the following ways:
///
/// - By DICOM tag, via [`by_tag`][1];
/// - By its keyword (also known as alias) via [`by_name`][2];
/// - By an expression which may either be a keyword
///   or a tag printed in one of its standard forms,
///   using [`by_expr`][3].
///
/// These methods will return `None`
/// when the tag or name is not recognized by the dictionary.
///
/// In addition,
/// the data element dictionary provides
/// built-in DICOM tag and selector (path) parsers for convenience.
/// [`parse_tag`][4] converts an arbitrary expression to a tag,
/// whereas [`parse_selector`][5] produces an [attribute selector][6].
///
/// [1]: DataDictionary::by_tag
/// [2]: DataDictionary::by_name
/// [3]: DataDictionary::by_expr
/// [4]: DataDictionary::parse_tag
/// [5]: DataDictionary::parse_selector
/// [6]: crate::ops::AttributeSelector
pub trait DataDictionary {
    /// The type of the dictionary entry.
    type Entry: DataDictionaryEntry;

    /// Fetch a data element entry by its tag.
    fn by_tag(&self, tag: Tag) -> Option<&Self::Entry>;

    /// Fetch an entry by its usual alias
    /// (e.g. "PatientName" or "SOPInstanceUID").
    /// Aliases (or keyword)
    /// are usually in UpperCamelCase,
    /// not separated by spaces,
    /// and are case sensitive.
    ///
    /// Querying the dictionary by name is usually
    /// slightly more expensive than by DICOM tag.
    /// If the parameter provided is a string literal
    /// (e.g. `"StudyInstanceUID"`),
    /// then it may be better to use [`by_tag`][1]
    /// with a known tag constant
    /// (such as [`tags::STUDY_INSTANCE_UID`][2]
    /// from the [`dicom-dictionary-std`][3] crate).
    ///
    /// [1]: DataDictionary::by_tag
    /// [2]: https://docs.rs/dicom-dictionary-std/0.5.0/dicom_dictionary_std/tags/constant.STUDY_INSTANCE_UID.html
    /// [3]: https://docs.rs/dicom-dictionary-std/0.5.0
    fn by_name(&self, name: &str) -> Option<&Self::Entry>;

    /// Fetch an entry by its alias or by DICOM tag expression.
    ///
    /// This method accepts a tag descriptor in any of the following formats:
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
    fn by_expr(&self, tag: &str) -> Option<&Self::Entry> {
        match tag.parse() {
            Ok(tag) => self.by_tag(tag),
            Err(_) => self.by_name(tag),
        }
    }

    /// Use this data element dictionary to interpret a DICOM tag.
    ///
    /// This method accepts a tag descriptor in any of the following formats:
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

    /// Parse a string as an [attribute selector][1].
    ///
    /// Attribute selectors are defined by the syntax
    /// `( «key»([«item»])? . )* «key» `
    /// where_`«key»`_ is either a DICOM tag or keyword
    /// as accepted by this dictionary
    /// when calling the method [`parse_tag`](DataDictionary::parse_tag).
    /// More details about the syntax can be found
    /// in the documentation of [`AttributeSelector`][1].
    ///
    /// Returns an error if the string does not follow the given syntax,
    /// or one of the key components could not be resolved.
    ///
    /// [1]: crate::ops::AttributeSelector
    ///
    /// ### Examples of valid input:
    ///
    /// - `(0002,00010)`:
    ///   _Transfer Syntax UID_
    /// - `00101010`:
    ///   _Patient Age_
    /// - `0040A168[0].CodeValue`:
    ///   _Code Value_ in first item of _Concept Code Sequence_
    /// - `SequenceOfUltrasoundRegions.RegionSpatialFormat`:
    ///   _Region Spatial Format_ in first item of _Sequence of Ultrasound Regions_
    fn parse_selector(&self, selector_text: &str) -> Result<AttributeSelector, ParseSelectorError> {
        let mut steps = crate::value::C::new();
        for part in selector_text.split('.') {
            // detect if intermediate
            if part.ends_with(']') {
                let split_i = part.find('[').context(MissingItemDelimiterSnafu)?;
                let tag_part = &part[0..split_i];
                let item_index_part = &part[split_i + 1..part.len() - 1];

                let tag: Tag = self.parse_tag(tag_part).context(ParseKeySnafu)?;
                let item: u32 = item_index_part.parse().ok().context(ParseItemIndexSnafu)?;
                steps.push(AttributeSelectorStep::Nested { tag, item });
            } else {
                // treat it as a tag step
                let tag: Tag = self.parse_tag(part).context(ParseKeySnafu)?;
                steps.push(AttributeSelectorStep::Tag(tag));
            }
        }

        Ok(AttributeSelector::new(steps).context(ParseLeafSnafu)?)
    }
}

/// The data element dictionary entry type,
/// representing a DICOM attribute.
pub trait DataDictionaryEntry {
    /// The full possible tag range of the attribute,
    /// which this dictionary entry can represent.
    fn tag_range(&self) -> TagRange;

    /// Fetch a single tag applicable to this attribute.
    ///
    /// Note that this is not necessarily
    /// the original tag used as key for this entry.
    fn tag(&self) -> Tag {
        self.tag_range().inner()
    }
    /// The alias of the attribute, with no spaces, usually in UpperCamelCase.
    fn alias(&self) -> &str;

    /// The extended value representation descriptor of the attribute.
    /// The use of [`VirtualVr`] is to attend to edge cases
    /// in which the representation of a value
    /// depends on surrounding context.
    fn vr(&self) -> VirtualVr;
}

/// A data type for a dictionary entry with full ownership.
#[derive(Debug, PartialEq, Clone)]
pub struct DataDictionaryEntryBuf {
    /// The attribute tag range
    pub tag: TagRange,
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: String,
    /// The _typical_  value representation of the attribute
    pub vr: VirtualVr,
}

impl DataDictionaryEntry for DataDictionaryEntryBuf {
    fn tag_range(&self) -> TagRange {
        self.tag
    }
    fn alias(&self) -> &str {
        self.alias.as_str()
    }
    fn vr(&self) -> VirtualVr {
        self.vr
    }
}

/// A data type for a dictionary entry with a string slice for its alias.
#[derive(Debug, PartialEq, Clone)]
pub struct DataDictionaryEntryRef<'a> {
    /// The attribute tag or tag range
    pub tag: TagRange,
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: &'a str,
    /// The extended value representation descriptor of the attribute
    pub vr: VirtualVr,
}

impl<'a> DataDictionaryEntry for DataDictionaryEntryRef<'a> {
    fn tag_range(&self) -> TagRange {
        self.tag
    }
    fn alias(&self) -> &str {
        self.alias
    }
    fn vr(&self) -> VirtualVr {
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
