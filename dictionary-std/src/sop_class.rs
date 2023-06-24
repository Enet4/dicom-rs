//! SOP class dictionary implementation

use std::collections::HashMap;

use dicom_core::dictionary::{UidDictionaryEntryRef, UidDictionary};
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
            by_keyword: HashMap::new(),
            by_uid: HashMap::new(),
        }
    }

    /// record all of the given dictionary entries
    fn index_all(&mut self, entries: &'static [UidDictionaryEntryRef<'static>]) -> &mut Self {
        let entries_by_keyword = entries.iter().map(|e| (e.alias, e));
        self.by_keyword.extend(entries_by_keyword);

        let entries_by_uid = entries.iter().map(|e| (e.uid, e));
        self.by_uid.extend(entries_by_uid);

        self
    }
}

impl UidDictionary for StandardUidRegistry {
    type Entry = UidDictionaryEntryRef<'static>;

    #[inline]
    fn by_keyword(&self, keyword: &str) -> Option<&Self::Entry> {
        self.by_keyword.get(keyword).copied()
    }

    #[inline]
    fn by_uid(&self, uid: &str) -> Option<&Self::Entry> {
        self.by_uid.get(uid).copied()
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


impl UidDictionary for StandardSopClassDictionary {
    type Entry = UidDictionaryEntryRef<'static>;

    #[inline]
    fn by_keyword(&self, keyword: &str) -> Option<&Self::Entry> {
        DICT.by_keyword(keyword)
    }

    #[inline]
    fn by_uid(&self, uid: &str) -> Option<&Self::Entry> {
        DICT.by_uid(uid)
    }
}

fn init_dictionary() -> StandardUidRegistry {
    let mut d = StandardUidRegistry::new();

    // only index SOP classes in this one
    d.index_all(SOP_CLASSES);
    d
}



#[cfg(test)]
mod tests {
    use dicom_core::dictionary::{UidDictionaryEntryRef, UidDictionary, UidType};
    use crate::StandardSopClassDictionary;

    // tests for just a few SOP classes to make sure that the entries
    // were well installed into the dictionary index
    #[test]
    fn can_fetch_sop_classes() {
        let dict = StandardSopClassDictionary::default();

        let entry = dict.by_uid("1.2.840.10008.1.1");
        assert_eq!(
            entry,
            Some(&UidDictionaryEntryRef {
                uid: "1.2.840.10008.1.1",
                alias: "Verification",
                name: "Verification SOP Class",
                retired: false,
                r#type: UidType::SopClass,
            })
        );

        let entry = dict.by_keyword("ComputedRadiographyImageStorage");
        assert_eq!(
            entry,
            Some(&UidDictionaryEntryRef {
                uid: crate::uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE,
                alias: "ComputedRadiographyImageStorage",
                name: "Computed Radiography Image Storage",
                retired: false,
                r#type: UidType::SopClass,
            })
        );

        let entry = dict.by_uid("1.2.840.10008.5.1.4.1.1.3");
        assert_eq!(
            entry,
            Some(&UidDictionaryEntryRef {
                uid: "1.2.840.10008.5.1.4.1.1.3",
                alias: "UltrasoundMultiFrameImageStorageRetired",
                name: "Ultrasound Multi-frame Image Storage (Retired)",
                retired: true,
                r#type: UidType::SopClass,
            })
        );

        // no transfer syntaxes, only SOP classes
        let entry = dict.by_uid(crate::uids::EXPLICIT_VR_LITTLE_ENDIAN);
        assert_eq!(entry, None);
   }
}