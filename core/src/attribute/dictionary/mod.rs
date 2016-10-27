//! This module contains the concept of a DICOM data dictionary, and aggregates
//! all built-in data dictionaries.
//!
//! For most purposes, the standard data dictionary is sufficient.

pub mod standard;
pub mod stub;

use attribute::ValueRepresentation;
use std::fmt::Debug;

/// Retrieve the standard DICOM dictionary.
pub fn get_standard_dictionary() -> &'static standard::StandardAttributeDictionary {
    standard::get_instance()
}

/** Type trait for a dictionary of DICOM attributes. Attribute dictionaries provide the
 * means to convert a tag to an alias and vice versa, as well as a form of retrieving
 * additional information about the attribute.
 * 
 * The methods herein have no generic parameters, so as to be used as a trait object.
 */
pub trait AttributeDictionary<'a>: Debug {
    /// Fetch an entry by its usual alias (e.g. "PatientName" or "SOPInstanceUID").
    /// Aliases are usually case sensitive and not separated by spaces.
    fn get_by_name(&self, name: &str) -> Option<&'a DictionaryEntry<'a>>;

    /// Fetch an entry by its tag.
    fn get_by_tag(&self, tag: (u16, u16)) -> Option<&'a DictionaryEntry<'a>>;
}

/// The dictionary entry data type, representing a DICOM attribute.
#[derive(Debug, PartialEq, Eq)]
pub struct DictionaryEntry<'a> {
    /// The attribute tag
    pub tag: (u16, u16),
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: &'a str,
    /// The _typical_  value representation of the attribute
    pub vr: ValueRepresentation,
}

/// Utility data structure that resolves to a DICOM attribute tag
/// at a later time. 
#[derive(Debug)]
pub struct TagByName<'d, N: AsRef<str>, AD: AttributeDictionary<'d> + 'd> {
    dict: &'d AD,
    name: N,
}

impl<'d, N: AsRef<str>, AD: AttributeDictionary<'d> + 'd> TagByName<'d, N, AD> {
    /// Create a tag resolver by name using the given dictionary.
    pub fn new(dictionary: &'d AD, name: N) -> TagByName<'d, N, AD> {
        TagByName {
            dict: dictionary,
            name: name,
        }
    }
}

impl<N: AsRef<str>> TagByName<'static, N, standard::StandardAttributeDictionary> {
    /// Create a tag resolver by name using the standard dictionary.
    pub fn with_std_dict(name: N) -> TagByName<'static, N, standard::StandardAttributeDictionary> {
        TagByName {
            dict: get_standard_dictionary(),
            name: name
        }
    }
}

impl<'d, N: AsRef<str>, AD: AttributeDictionary<'d>> From<TagByName<'d, N, AD>> for Option<(u16, u16)> {
    fn from(tag: TagByName<'d, N, AD>) -> Option<(u16, u16)> {
        tag.dict.get_by_name(tag.name.as_ref()).map(|e| e.tag)
    }
}

