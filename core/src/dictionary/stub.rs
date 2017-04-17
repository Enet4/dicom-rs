//! This module contains a stub dictionary.

use super::{DataDictionary, DictionaryEntryRef};
use data::Tag;

/// An empty attribute dictionary.
#[derive(Debug, Clone, Copy)]
pub struct StubDataDictionary;

impl DataDictionary for StubDataDictionary {
    type Entry = DictionaryEntryRef<'static>;
    fn get_by_name(&self, _: &str) -> Option<&DictionaryEntryRef<'static>> {
        None
    }

    fn get_by_tag(&self, _: Tag) -> Option<&DictionaryEntryRef<'static>> {
        None
    }
}

impl<'a> DataDictionary for &'a StubDataDictionary {
    type Entry = DictionaryEntryRef<'static>;
    fn get_by_name(&self, _: &str) -> Option<&DictionaryEntryRef<'static>> {
        None
    }

    fn get_by_tag(&self, _: Tag) -> Option<&DictionaryEntryRef<'static>> {
        None
    }
}

impl DataDictionary for Box<StubDataDictionary> {
    type Entry = DictionaryEntryRef<'static>;
    fn get_by_name(&self, _: &str) -> Option<&DictionaryEntryRef<'static>> {
        None
    }

    fn get_by_tag(&self, _: Tag) -> Option<&DictionaryEntryRef<'static>> {
        None
    }
}
