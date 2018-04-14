//! This module contains a stub dictionary.

use super::{DataDictionary, DictionaryEntryRef};
use data::Tag;

/// An empty attribute dictionary.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StubDataDictionary;

impl DataDictionary for StubDataDictionary {
    type Entry = DictionaryEntryRef<'static>;
    fn by_name(&self, _: &str) -> Option<&DictionaryEntryRef<'static>> {
        None
    }

    fn by_tag(&self, _: Tag) -> Option<&DictionaryEntryRef<'static>> {
        None
    }
}

impl<'a> DataDictionary for &'a StubDataDictionary {
    type Entry = DictionaryEntryRef<'static>;
    fn by_name(&self, _: &str) -> Option<&DictionaryEntryRef<'static>> {
        None
    }

    fn by_tag(&self, _: Tag) -> Option<&DictionaryEntryRef<'static>> {
        None
    }
}

impl DataDictionary for Box<StubDataDictionary> {
    type Entry = DictionaryEntryRef<'static>;
    fn by_name(&self, _: &str) -> Option<&DictionaryEntryRef<'static>> {
        None
    }

    fn by_tag(&self, _: Tag) -> Option<&DictionaryEntryRef<'static>> {
        None
    }
}
