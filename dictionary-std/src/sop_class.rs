//! SOP class dictionary implementation

use std::collections::HashMap;

use dicom_core::dictionary::UidDictionaryEntryRef;
use once_cell::sync::Lazy;

use crate::uids::SOP_CLASSES;

static DICT: Lazy<StandardUidRegistry> = Lazy::new(init_dictionary);

/// Retrieve a singleton instance of the standard SOP class registry.
///
/// Note that one does not generally have to call this
/// unless when retrieving the underlying registry is important.
/// The unit type [`StandardDataDictionary`]
/// already provides a lazy loaded singleton implementing the necessary traits.
#[inline]
pub fn registry() -> &'static StandardUidRegistry {
    &DICT
}

/// The data struct actually containing the standard UID dictionary.
///
/// This structure is made opaque via the unit type [`StandardUidDictionary`],
/// which provides a lazy loaded singleton.
#[derive(Debug)]
pub struct StandardUidRegistry {
    /// mapping: keyword → entry
    by_keyword: HashMap<&'static str, &'static UidDictionaryEntryRef<'static>>,
    /// mapping: uid → entry
    by_uid: HashMap<&'static str, &'static UidDictionaryEntryRef<'static>>,
}

impl StandardUidRegistry {
    fn new() -> StandardUidRegistry {
        StandardUidRegistry {
            by_keyword: HashMap::with_capacity(320),
            by_uid: HashMap::with_capacity(320),
        }
    }

    /// record the given dictionary entry reference
    fn index(&mut self, entry: &'static UidDictionaryEntryRef<'static>) -> &mut Self {
        self.by_keyword.insert(entry.alias, entry);
        self.by_uid.insert(entry.uid, entry);
        self
    }
}

/// An SOP class dictionary which consults
/// the library's global DICOM SOP class registry.
///
/// This is the type which would generally be used
/// whenever a program needs to translate an SOP class UID
/// to its name or from its keyword (alias) back to a UID
/// during a program's execution.
/// Note that the [`uids`](crate::uids) module
/// already provides easy to use constants for SOP classes.
///
/// The dictionary index is automatically initialized upon the first use.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
pub struct StandardSopClassDictionary;

fn init_dictionary() -> StandardUidRegistry {
    let mut d = StandardUidRegistry::new();

    // only index SOP classes in this one
    for entry in SOP_CLASSES {
        d.index(entry);
    }
    d
}
