//! This module contains a stub dictionary.

use super::{DataDictionary, DictionaryEntry};
use data::Tag;

/// An empty attribute dictionary.
#[derive(Debug, Clone, Copy)]
pub struct StubDataDictionary;

impl<'a> DataDictionary<'a> for StubDataDictionary {
    fn get_by_name(&self, _: &str) -> Option<&'a DictionaryEntry<'a>> {
        None
    }

    fn get_by_tag(&self, _: Tag) -> Option<&'a DictionaryEntry<'a>> {
        None
    }
}
