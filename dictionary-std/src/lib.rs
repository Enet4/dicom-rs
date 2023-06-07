//! This crate implements the standard attribute dictionary.
//!
//! This dictionary is a singleton containing all information about the
//! DICOM attributes specified in the standard according to DICOM PS3.6 2019c,
//! and it will be used by default in most other abstractions available.
//!
//! When not using private tags, this dictionary should suffice.

pub mod tags;

use crate::tags::ENTRIES;
use dicom_core::dictionary::{DataDictionary, DictionaryEntryRef, TagRange::*};
use dicom_core::header::Tag;
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::{Display, Formatter};

lazy_static! {
    static ref DICT: StandardDictionaryRegistry = init_dictionary();
}

/// Retrieve a singleton instance of the standard dictionary registry.
pub fn registry() -> &'static StandardDictionaryRegistry {
    &DICT
}

/// The data struct containing the standard dictionary.
#[derive(Debug)]
pub struct StandardDictionaryRegistry {
    /// mapping: name → tag
    by_name: HashMap<&'static str, &'static DictionaryEntryRef<'static>>,
    /// mapping: tag → name
    by_tag: HashMap<Tag, &'static DictionaryEntryRef<'static>>,
    /// repeating elements of the form (ggxx, eeee). The `xx` portion is zeroed.
    repeating_ggxx: HashSet<Tag>,
    /// repeating elements of the form (gggg, eexx). The `xx` portion is zeroed.
    repeating_eexx: HashSet<Tag>,
}

impl StandardDictionaryRegistry {
    fn new() -> StandardDictionaryRegistry {
        StandardDictionaryRegistry {
            by_name: HashMap::with_capacity(5000),
            by_tag: HashMap::with_capacity(5000),
            repeating_ggxx: HashSet::with_capacity(75),
            repeating_eexx: HashSet::new(),
        }
    }

    /// record the given dictionary entry reference
    fn index(&mut self, entry: &'static DictionaryEntryRef<'static>) -> &mut Self {
        self.by_name.insert(entry.alias, entry);
        self.by_tag.insert(entry.tag.inner(), entry);
        match entry.tag {
            Group100(tag) => {
                self.repeating_ggxx.insert(tag);
            }
            Element100(tag) => {
                self.repeating_eexx.insert(tag);
            }
            _ => {}
        }
        self
    }
}

/// A data dictionary which consults the library's global DICOM attribute registry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardDataDictionary;

impl StandardDataDictionary {
    fn indexed_tag(tag: Tag) -> Option<&'static DictionaryEntryRef<'static>> {
        let r = registry();

        r.by_tag
            .get(&tag)
            .or_else(|| {
                let group_trimmed = Tag(tag.0 & 0xFF00, tag.1);

                if r.repeating_ggxx.contains(&group_trimmed) {
                    r.by_tag.get(&group_trimmed)
                } else {
                    let elem_trimmed = Tag(tag.0, tag.1 & 0xFF00);
                    if r.repeating_eexx.contains(&elem_trimmed) {
                        r.by_tag.get(&elem_trimmed)
                    } else {
                        None
                    }
                }
            })
            .cloned()
    }
}

impl DataDictionary for StandardDataDictionary {
    type Entry = DictionaryEntryRef<'static>;

    fn by_name(&self, name: &str) -> Option<&Self::Entry> {
        registry().by_name.get(name).cloned()
    }

    fn by_tag(&self, tag: Tag) -> Option<&Self::Entry> {
        StandardDataDictionary::indexed_tag(tag)
    }
}

impl<'a> DataDictionary for &'a StandardDataDictionary {
    type Entry = DictionaryEntryRef<'static>;

    fn by_name(&self, name: &str) -> Option<&'static DictionaryEntryRef<'static>> {
        registry().by_name.get(name).cloned()
    }

    fn by_tag(&self, tag: Tag) -> Option<&'static DictionaryEntryRef<'static>> {
        StandardDataDictionary::indexed_tag(tag)
    }
}

impl Display for StandardDataDictionary {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.write_str("Standard DICOM Data Dictionary")
    }
}

fn init_dictionary() -> StandardDictionaryRegistry {
    let mut d = StandardDictionaryRegistry::new();
    for entry in ENTRIES {
        d.index(entry);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::StandardDataDictionary;
    use dicom_core::dictionary::{DataDictionary, DictionaryEntryRef, TagRange::*};
    use dicom_core::header::{Tag, VR};

    // tests for just a few attributes to make sure that the entries
    // were well installed into the crate
    #[test]
    fn smoke_test() {
        let dict = StandardDataDictionary::default();

        assert_eq!(
            dict.by_name("PatientName"),
            Some(&DictionaryEntryRef {
                tag: Single(Tag(0x0010, 0x0010)),
                alias: "PatientName",
                vr: VR::PN,
            })
        );

        assert_eq!(
            dict.by_name("Modality"),
            Some(&DictionaryEntryRef {
                tag: Single(Tag(0x0008, 0x0060)),
                alias: "Modality",
                vr: VR::CS,
            })
        );

        let pixel_data = dict
            .by_tag(Tag(0x7FE0, 0x0010))
            .expect("Pixel Data attribute should exist");
        eprintln!("{:X?}", pixel_data.tag);
        assert_eq!(pixel_data.tag, Single(Tag(0x7FE0, 0x0010)));
        assert_eq!(pixel_data.alias, "PixelData");
        assert!(pixel_data.vr == VR::OB || pixel_data.vr == VR::OW);

        let overlay_data = dict
            .by_tag(Tag(0x6000, 0x3000))
            .expect("Overlay Data attribute should exist");
        assert_eq!(overlay_data.tag, Group100(Tag(0x6000, 0x3000)));
        assert_eq!(overlay_data.alias, "OverlayData");
        assert!(overlay_data.vr == VR::OB || overlay_data.vr == VR::OW);

        // repeated overlay data
        let overlay_data = dict
            .by_tag(Tag(0x60EE, 0x3000))
            .expect("Repeated Overlay Data attribute should exist");
        assert_eq!(overlay_data.tag, Group100(Tag(0x6000, 0x3000)));
        assert_eq!(overlay_data.alias, "OverlayData");
        assert!(overlay_data.vr == VR::OB || overlay_data.vr == VR::OW);
    }

    // tests for just a few attributes to make sure that the tag constants
    // were well installed into the crate
    #[test]
    fn constants_available() {
        use crate::tags::*;
        assert_eq!(PATIENT_NAME, Tag(0x0010, 0x0010));
        assert_eq!(MODALITY, Tag(0x0008, 0x0060));
        assert_eq!(PIXEL_DATA, Tag(0x7FE0, 0x0010));
        assert_eq!(STATUS, Tag(0x0000, 0x0900));
    }

    #[test]
    fn can_parse_tags() {
        let dict = StandardDataDictionary;

        assert_eq!(dict.parse_tag("(7FE0,0010)"), Some(crate::tags::PIXEL_DATA));
        assert_eq!(dict.parse_tag("0010,21C0"), Some(Tag(0x0010, 0x21C0)));
        assert_eq!(
            dict.parse_tag("OperatorsName"),
            Some(crate::tags::OPERATORS_NAME)
        );

        // can't parse these
        assert_eq!(dict.parse_tag(""), None);
        assert_eq!(dict.parse_tag("1111,2222,3333"), None);
        assert_eq!(dict.parse_tag("OperatorNickname"), None);
    }


    #[test]
    fn can_query_by_expression() {
        let dict = StandardDataDictionary;

        assert_eq!(
            dict.by_expr("(0010,0010)"),
            Some(&DictionaryEntryRef {
                tag: Single(crate::tags::PATIENT_NAME),
                alias: "PatientName",
                vr: VR::PN,
            })
        );

        assert_eq!(
            dict.by_expr("0008,0060"),
            Some(&DictionaryEntryRef {
                tag: Single(crate::tags::MODALITY),
                alias: "Modality",
                vr: VR::CS,
            })
        );

        assert_eq!(
            dict.by_expr("OperatorsName"),
            Some(&DictionaryEntryRef {
                tag: Single(crate::tags::OPERATORS_NAME),
                alias: "OperatorsName",
                vr: VR::PN,
            })
        );

        // can't handle these
        assert_eq!(dict.parse_tag("0080 0010"), None);
        assert_eq!(dict.parse_tag("(0000.0600)"), None);
        assert_eq!(dict.parse_tag("OPERATORSNAME"), None);
    }

}
