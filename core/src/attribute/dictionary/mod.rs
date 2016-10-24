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

/** The dictionary entry data type, representing a DICOM attribute. */
#[derive(Debug, PartialEq, Eq)]
pub struct DictionaryEntry<'a> {
    /// The attribute tag
    pub tag: (u16, u16),
    /// The alias of the attribute, with no spaces, usually InCapitalizedCamelCase
    pub alias: &'a str,
    /// The _typical_  value representation of the attribute
    pub vr: ValueRepresentation,
}
