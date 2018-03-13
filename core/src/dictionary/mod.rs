//! This module contains the concept of a DICOM data dictionary, and aggregates
//! all built-in data dictionaries.
//!
//! For most purposes, the standard data dictionary is sufficient.

pub mod standard;
pub mod stub;

pub use self::standard::StandardDataDictionary;

use data::Tag;
use data::VR;
use std::fmt::Debug;

/// Retrieve the global standard DICOM dictionary.
pub fn standard_dictionary() -> &'static StandardDataDictionary {
    standard::instance()
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
    fn get_by_name(&self, name: &str) -> Option<&Self::Entry>;

    /// Fetch an entry by its tag.
    fn get_by_tag(&self, tag: Tag) -> Option<&Self::Entry>;
}

/// The dictionary entry data type, representing a DICOM attribute.
pub trait DictionaryEntry {
    /// The attribute tag.
    fn tag(&self) -> Tag;
    /// The alias of the attribute, with no spaces, usually in UpperCamelCase.
    fn alias(&self) -> &str;
    /// The _typical_  value representation of the attribute.
    fn vr(&self) -> VR;
}

/// A data type for a dictionary entry with full ownership.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DictionaryEntryBuf {
    /// The attribute tag
    pub tag: Tag,
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: String,
    /// The _typical_  value representation of the attribute
    pub vr: VR,
}

impl DictionaryEntry for DictionaryEntryBuf {
    fn tag(&self) -> Tag {
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
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DictionaryEntryRef<'a> {
    /// The attribute tag
    pub tag: Tag,
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: &'a str,
    /// The _typical_  value representation of the attribute
    pub vr: VR,
}

impl<'a> DictionaryEntry for DictionaryEntryRef<'a> {
    fn tag(&self) -> Tag {
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
#[derive(Debug)]
pub struct TagByName<'d, N: AsRef<str>, AD: 'd + DataDictionary> {
    dict: &'d AD,
    name: N,
}

impl<'d, N: AsRef<str>, AD: 'd + DataDictionary> TagByName<'d, N, AD> {
    /// Create a tag resolver by name using the given dictionary.
    pub fn new(dictionary: &'d AD, name: N) -> TagByName<'d, N, AD> {
        TagByName {
            dict: dictionary,
            name: name,
        }
    }
}

impl<N: AsRef<str>> TagByName<'static, N, StandardDataDictionary> {
    /// Create a tag resolver by name using the standard dictionary.
    pub fn with_std_dict(name: N) -> TagByName<'static, N, StandardDataDictionary> {
        TagByName {
            dict: standard_dictionary(),
            name: name,
        }
    }
}

impl<'d, N: AsRef<str>, AD: 'd + DataDictionary> From<TagByName<'d, N, AD>> for Option<Tag> {
    fn from(tag: TagByName<'d, N, AD>) -> Option<Tag> {
        tag.dict.get_by_name(tag.name.as_ref()).map(|e| e.tag())
    }
}
