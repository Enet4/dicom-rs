//! This module implements the standard attribute dictionary.
//!
//! This dictionary is a singleton containing all information about the
//! DICOM attributes specified in the standard according to DICOM PS3.6 2016c,
//! and it will be used by default
//!
//! When not using private tags, this dictionary should suffice.

mod entries;

use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use dictionary::{DataDictionary, DictionaryEntryRef};
use header::{Tag, VR};
use self::entries::ENTRIES;

lazy_static! {
    static ref DICT: StandardDictionaryRegistry = {
        init_dictionary()
    };
}

/// Retrieve a singleton instance of the standard dictionary registry.
pub fn registry() -> &'static StandardDictionaryRegistry {
    &DICT
}

/// The data struct containing the standard dictionary.
#[derive(Debug)]
pub struct StandardDictionaryRegistry {
    by_name: HashMap<&'static str, &'static DictionaryEntryRef<'static>>,
    by_tag: HashMap<Tag, &'static DictionaryEntryRef<'static>>,
}

impl StandardDictionaryRegistry {
    fn new() -> StandardDictionaryRegistry {
        StandardDictionaryRegistry {
            by_name: HashMap::new(),
            by_tag: HashMap::new(),
        }
    }

    fn index(&mut self, entry: &'static DictionaryEntryRef<'static>) -> &mut Self {
        self.by_name.insert(entry.alias, entry);
        self.by_tag.insert(entry.tag, entry);
        self
    }
}

/// A data dictionary which consults the library's global DICOM attribute registry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardDataDictionary;

impl DataDictionary for StandardDataDictionary {
    type Entry = DictionaryEntryRef<'static>;

    fn by_name(&self, name: &str) -> Option<&Self::Entry> {
        registry().by_name.get(name).cloned()
    }

    fn by_tag(&self, tag: Tag) -> Option<&Self::Entry> {
        registry().by_tag.get(&tag).cloned()
    }
}

impl<'a> DataDictionary for &'a StandardDataDictionary {
    type Entry = DictionaryEntryRef<'static>;

    fn by_name(&self, name: &str) -> Option<&'static DictionaryEntryRef<'static>> {
        registry().by_name.get(name).cloned()
    }

    fn by_tag(&self, tag: Tag) -> Option<&'static DictionaryEntryRef<'static>> {
        registry().by_tag.get(&tag).cloned()
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
        d.index(&entry);
    }
    for entry in META_ENTRIES {
        d.index(&entry);
    }
    d
}

// meta information entries
type E<'a> = DictionaryEntryRef<'a>;
const META_ENTRIES: &'static [E<'static>] = &[
    E {
        tag: Tag(0x0002, 0x0000),
        alias: "FileMetaInformationGroupLength",
        vr: VR::UL,
    },
    E {
        tag: Tag(0x0002, 0x0001),
        alias: "FileMetaInformationVersion",
        vr: VR::OB,
    },
    E {
        tag: Tag(0x0002, 0x0002),
        alias: "MediaStorageSOPClassUID",
        vr: VR::UI,
    },
    E {
        tag: Tag(0x0002, 0x0003),
        alias: "MediaStorageSOPInstanceUID",
        vr: VR::UI,
    },
    E {
        tag: Tag(0x0002, 0x0010),
        alias: "TransferSyntaxUID",
        vr: VR::UI,
    },
    E {
        tag: Tag(0x0002, 0x0012),
        alias: "ImplementationClassUID",
        vr: VR::UI,
    },
    E {
        tag: Tag(0x0002, 0x0013),
        alias: "ImplentationVersionName",
        vr: VR::SH,
    },
    E {
        tag: Tag(0x0002, 0x0016),
        alias: "SourceApplicationEntityTitle",
        vr: VR::AE,
    },
    E {
        tag: Tag(0x0002, 0x0017),
        alias: "SendingApplicationEntityTitle",
        vr: VR::AE,
    },
    E {
        tag: Tag(0x0002, 0x0018),
        alias: "ReceivingApplicationEntityTitle",
        vr: VR::AE,
    },
    E {
        tag: Tag(0x0002, 0x0100),
        alias: "PrivateInformationCreatorUID",
        vr: VR::UI,
    },
    E {
        tag: Tag(0x0002, 0x0102),
        alias: "PrivateInformation",
        vr: VR::OB,
    },
];
