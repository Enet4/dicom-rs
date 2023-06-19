//! This module contains a stub dictionary.

use super::{DataDictionary, DataDictionaryEntryRef};
use crate::header::Tag;

/// An empty attribute dictionary.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StubDataDictionary;

impl DataDictionary for StubDataDictionary {
    type Entry = DataDictionaryEntryRef<'static>;
    fn by_name(&self, _: &str) -> Option<&DataDictionaryEntryRef<'static>> {
        None
    }

    fn by_tag(&self, _: Tag) -> Option<&DataDictionaryEntryRef<'static>> {
        None
    }
}

impl<'a> DataDictionary for &'a StubDataDictionary {
    type Entry = DataDictionaryEntryRef<'static>;
    fn by_name(&self, _: &str) -> Option<&DataDictionaryEntryRef<'static>> {
        None
    }

    fn by_tag(&self, _: Tag) -> Option<&DataDictionaryEntryRef<'static>> {
        None
    }
}

impl DataDictionary for Box<StubDataDictionary> {
    type Entry = DataDictionaryEntryRef<'static>;
    fn by_name(&self, _: &str) -> Option<&DataDictionaryEntryRef<'static>> {
        None
    }

    fn by_tag(&self, _: Tag) -> Option<&DataDictionaryEntryRef<'static>> {
        None
    }
}
