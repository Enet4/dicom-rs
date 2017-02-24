//! This module contains a stub dictionary.

use super::{AttributeDictionary, DictionaryEntry};
use data::Tag;

/// An empty attribute dictionary.
#[derive(Debug, Clone, Copy)]
pub struct StubAttributeDictionary;

impl<'a> AttributeDictionary<'a> for StubAttributeDictionary {
    fn get_by_name(&self, _: &str) -> Option<&'a DictionaryEntry<'a>> {
        None
    }

    fn get_by_tag(&self, _: Tag) -> Option<&'a DictionaryEntry<'a>> {
        None
    }
}
