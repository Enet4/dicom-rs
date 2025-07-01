//! Data element dictionary implementation

use crate::tags::ENTRIES;
use dicom_core::dictionary::{DataDictionary, DataDictionaryEntryRef, TagRange::*, VirtualVr};
use dicom_core::header::Tag;
use dicom_core::VR;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::{Display, Formatter};

static DICT: Lazy<StandardDataDictionaryRegistry> = Lazy::new(init_dictionary);

/// Retrieve a singleton instance of the standard dictionary registry.
///
/// Note that one does not generally have to call this
/// unless when retrieving the underlying registry is important.
/// The unit type [`StandardDataDictionary`]
/// already provides a lazy loaded singleton implementing the necessary traits.
#[inline]
pub fn registry() -> &'static StandardDataDictionaryRegistry {
    &DICT
}

/// The data struct actually containing the standard dictionary.
///
/// This structure is made opaque via the unit type [`StandardDataDictionary`],
/// which provides a lazy loaded singleton.
#[derive(Debug)]
pub struct StandardDataDictionaryRegistry {
    /// mapping: name → entry
    by_name: HashMap<&'static str, &'static DataDictionaryEntryRef<'static>>,
    /// mapping: tag → entry
    by_tag: HashMap<Tag, &'static DataDictionaryEntryRef<'static>>,
    /// repeating elements of the form (ggxx, eeee). The `xx` portion is zeroed.
    repeating_ggxx: HashSet<Tag>,
    /// repeating elements of the form (gggg, eexx). The `xx` portion is zeroed.
    repeating_eexx: HashSet<Tag>,
}

impl StandardDataDictionaryRegistry {
    fn new() -> StandardDataDictionaryRegistry {
        StandardDataDictionaryRegistry {
            by_name: HashMap::with_capacity(5000),
            by_tag: HashMap::with_capacity(5000),
            repeating_ggxx: HashSet::with_capacity(75),
            repeating_eexx: HashSet::new(),
        }
    }

    /// record the given dictionary entry reference
    fn index(&mut self, entry: &'static DataDictionaryEntryRef<'static>) -> &mut Self {
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

/// Generic Group Length dictionary entry.
static GROUP_LENGTH_ENTRY: DataDictionaryEntryRef<'static> = DataDictionaryEntryRef {
    tag: GroupLength,
    alias: "GenericGroupLength",
    vr: VirtualVr::Exact(VR::UL),
};

/// Generic Private Creator dictionary entry.
static PRIVATE_CREATOR_ENTRY: DataDictionaryEntryRef<'static> = DataDictionaryEntryRef {
    tag: PrivateCreator,
    alias: "PrivateCreator",
    vr: VirtualVr::Exact(VR::LO),
};

/// A data element dictionary which consults
/// the library's global DICOM attribute registry.
///
/// This is the type which would generally be used
/// whenever a data element dictionary is needed,
/// such as when reading DICOM objects.
///
/// The dictionary index is automatically initialized upon the first use.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardDataDictionary;

impl StandardDataDictionary {
    fn indexed_tag(tag: Tag) -> Option<&'static DataDictionaryEntryRef<'static>> {
        let r = registry();

        r.by_tag
            .get(&tag)
            .or_else(|| {
                // check tags repeating in different groups
                let group_trimmed = Tag(tag.0 & 0xFF00, tag.1);
                if r.repeating_ggxx.contains(&group_trimmed) {
                    return r.by_tag.get(&group_trimmed);
                }
                // check tags repeating in different elements
                let elem_trimmed = Tag(tag.0, tag.1 & 0xFF00);
                if r.repeating_eexx.contains(&elem_trimmed) {
                    return r.by_tag.get(&elem_trimmed);
                }

                None
            })
            .cloned()
            .or_else(|| {
                // check for private creator
                if tag.0 & 1 == 1 && (0x0010..=0x00FF).contains(&tag.1) {
                    return Some(&PRIVATE_CREATOR_ENTRY);
                }
                // check for group length
                if tag.element() == 0x0000 {
                    return Some(&GROUP_LENGTH_ENTRY);
                }

                None
            })
    }
}

impl DataDictionary for StandardDataDictionary {
    type Entry = DataDictionaryEntryRef<'static>;

    fn by_name(&self, name: &str) -> Option<&Self::Entry> {
        registry().by_name.get(name).cloned()
    }

    fn by_tag(&self, tag: Tag) -> Option<&Self::Entry> {
        StandardDataDictionary::indexed_tag(tag)
    }
}

impl DataDictionary for &'_ StandardDataDictionary {
    type Entry = DataDictionaryEntryRef<'static>;

    fn by_name(&self, name: &str) -> Option<&'static DataDictionaryEntryRef<'static>> {
        registry().by_name.get(name).cloned()
    }

    fn by_tag(&self, tag: Tag) -> Option<&'static DataDictionaryEntryRef<'static>> {
        StandardDataDictionary::indexed_tag(tag)
    }
}

impl Display for StandardDataDictionary {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.write_str("Standard DICOM Data Dictionary")
    }
}

fn init_dictionary() -> StandardDataDictionaryRegistry {
    let mut d = StandardDataDictionaryRegistry::new();
    for entry in ENTRIES {
        d.index(entry);
    }
    // generic group length is not a generated entry,
    // inserting it manually
    d.by_name.insert("GenericGroupLength", &GROUP_LENGTH_ENTRY);
    d
}

#[cfg(test)]
mod tests {
    use crate::tags;

    use super::StandardDataDictionary;
    use dicom_core::dictionary::{DataDictionary, DataDictionaryEntryRef, TagRange::*, VirtualVr};
    use dicom_core::header::{Tag, VR};
    use dicom_core::ops::AttributeSelector;

    // tests for just a few attributes to make sure that the entries
    // were well installed into the crate
    #[test]
    fn smoke_test() {
        let dict = StandardDataDictionary;

        assert_eq!(
            dict.by_name("PatientName"),
            Some(&DataDictionaryEntryRef {
                tag: Single(Tag(0x0010, 0x0010)),
                alias: "PatientName",
                vr: VR::PN.into(),
            })
        );

        assert_eq!(
            dict.by_name("Modality"),
            Some(&DataDictionaryEntryRef {
                tag: Single(Tag(0x0008, 0x0060)),
                alias: "Modality",
                vr: VR::CS.into(),
            })
        );

        let pixel_data = dict
            .by_tag(Tag(0x7FE0, 0x0010))
            .expect("Pixel Data attribute should exist");
        eprintln!("{:X?}", pixel_data.tag);
        assert_eq!(pixel_data.tag, Single(Tag(0x7FE0, 0x0010)));
        assert_eq!(pixel_data.alias, "PixelData");
        assert!(pixel_data.vr == VirtualVr::Px);

        let overlay_data = dict
            .by_tag(Tag(0x6000, 0x3000))
            .expect("Overlay Data attribute should exist");
        assert_eq!(overlay_data.tag, Group100(Tag(0x6000, 0x3000)));
        assert_eq!(overlay_data.alias, "OverlayData");
        assert!(overlay_data.vr == VirtualVr::Ox);

        // repeated overlay data
        let overlay_data = dict
            .by_tag(Tag(0x60EE, 0x3000))
            .expect("Repeated Overlay Data attribute should exist");
        assert_eq!(overlay_data.tag, Group100(Tag(0x6000, 0x3000)));
        assert_eq!(overlay_data.alias, "OverlayData");
        assert!(overlay_data.vr == VirtualVr::Ox);
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
            Some(&DataDictionaryEntryRef {
                tag: Single(crate::tags::PATIENT_NAME),
                alias: "PatientName",
                vr: VR::PN.into(),
            })
        );

        assert_eq!(
            dict.by_expr("0008,0060"),
            Some(&DataDictionaryEntryRef {
                tag: Single(crate::tags::MODALITY),
                alias: "Modality",
                vr: VR::CS.into(),
            })
        );

        assert_eq!(
            dict.by_expr("OperatorsName"),
            Some(&DataDictionaryEntryRef {
                tag: Single(crate::tags::OPERATORS_NAME),
                alias: "OperatorsName",
                vr: VR::PN.into(),
            })
        );

        // can't handle these
        assert_eq!(dict.parse_tag("0080 0010"), None);
        assert_eq!(dict.parse_tag("(0000.0600)"), None);
        assert_eq!(dict.parse_tag("OPERATORSNAME"), None);
    }

    #[test]
    fn has_group_length_tags() {
        use crate::tags::*;
        assert_eq!(COMMAND_GROUP_LENGTH, Tag(0x0000, 0x0000));
        assert_eq!(FILE_META_INFORMATION_GROUP_LENGTH, Tag(0x0002, 0x0000));

        let dict = StandardDataDictionary;

        assert_eq!(
            dict.by_tag(FILE_META_INFORMATION_GROUP_LENGTH),
            Some(&DataDictionaryEntryRef {
                tag: Single(FILE_META_INFORMATION_GROUP_LENGTH),
                alias: "FileMetaInformationGroupLength",
                vr: VR::UL.into(),
            }),
        );

        assert_eq!(
            dict.by_tag(COMMAND_GROUP_LENGTH),
            Some(&DataDictionaryEntryRef {
                tag: Single(COMMAND_GROUP_LENGTH),
                alias: "CommandGroupLength",
                vr: VR::UL.into(),
            }),
        );

        // generic group length

        assert_eq!(
            dict.by_tag(Tag(0x7FE0, 0x0000)),
            Some(&DataDictionaryEntryRef {
                tag: GroupLength,
                alias: "GenericGroupLength",
                vr: VR::UL.into(),
            }),
        );

        assert_eq!(
            dict.by_name("GenericGroupLength"),
            Some(&DataDictionaryEntryRef {
                tag: GroupLength,
                alias: "GenericGroupLength",
                vr: VR::UL.into(),
            }),
        );
    }

    #[test]
    fn has_private_creator() {
        let dict = StandardDataDictionary;

        let private_creator = DataDictionaryEntryRef {
            tag: PrivateCreator,
            alias: "PrivateCreator",
            vr: VR::LO.into(),
        };

        assert_eq!(dict.by_tag(Tag(0x0009, 0x0010)), Some(&private_creator));
        assert_eq!(dict.by_tag(Tag(0x0009, 0x0011)), Some(&private_creator));
        assert_eq!(dict.by_tag(Tag(0x000B, 0x0010)), Some(&private_creator));
        assert_eq!(dict.by_tag(Tag(0x00ED, 0x00FF)), Some(&private_creator));
    }

    #[test]
    fn can_parse_selectors() {
        let dict = StandardDataDictionary;
        // - `(0002,0010)`:
        //   _Transfer Syntax UID_
        let selector: AttributeSelector = dict.parse_selector("(0002,0010)").unwrap();
        assert_eq!(selector, AttributeSelector::from(tags::TRANSFER_SYNTAX_UID));

        // - `00101010`:
        //   _Patient Age_
        let selector: AttributeSelector = dict.parse_selector("00101010").unwrap();
        assert_eq!(selector, AttributeSelector::from(tags::PATIENT_AGE));

        // - `0040A168[0].CodeValue`:
        //   _Code Value_ in first item of _Concept Code Sequence_
        let selector: AttributeSelector = dict.parse_selector("0040A168[0].CodeValue").unwrap();
        assert_eq!(
            selector,
            AttributeSelector::from((tags::CONCEPT_CODE_SEQUENCE, 0, tags::CODE_VALUE)),
        );
        // - `0040,A730[1].ContentSequence`:
        //   _Content Sequence_ in second item of _Content Sequence_
        let selector: AttributeSelector =
            dict.parse_selector("0040,A730[1].ContentSequence").unwrap();
        assert_eq!(
            selector,
            AttributeSelector::from((tags::CONTENT_SEQUENCE, 1, tags::CONTENT_SEQUENCE,)),
        );

        // - `SequenceOfUltrasoundRegions.RegionSpatialFormat`:
        //   _Region Spatial Format_ in first item of _Sequence of Ultrasound Regions_
        let selector: AttributeSelector = dict
            .parse_selector("SequenceOfUltrasoundRegions.RegionSpatialFormat")
            .unwrap();
        assert_eq!(
            selector,
            AttributeSelector::from((
                tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                0,
                tags::REGION_SPATIAL_FORMAT
            )),
        );
    }

    /// Can go to is text form and back without losing info
    #[test]
    fn print_and_parse_selectors() {
        let selectors = [
            AttributeSelector::from((tags::CONTENT_SEQUENCE, 1, tags::CONTENT_SEQUENCE)),
            AttributeSelector::new([
                (tags::CONTENT_SEQUENCE, 1).into(),
                (tags::CONTENT_SEQUENCE, 3).into(),
                (tags::CONTENT_SEQUENCE, 5).into(),
                (tags::CONCEPT_NAME_CODE_SEQUENCE, 0).into(),
                tags::CODE_VALUE.into(),
            ])
            .unwrap(),
        ];

        for selector in selectors {
            let selector2: AttributeSelector = StandardDataDictionary
                .parse_selector(&selector.to_string())
                .unwrap();
            assert_eq!(selector, selector2);
        }
    }
}
