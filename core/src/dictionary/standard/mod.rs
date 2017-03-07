#![allow(unsafe_code)]
//! This module implements the standard attribute dictionary.
//!
//! This dictionary is a singleton containing all information about the
//! DICOM attributes specified in the standard according to DICOM PS3.6 2016c,
//! and it will be used by default
//!
//! When not using private tags, this dictionary should suffice.

extern crate lazy_static;

mod entries;

use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use data::Tag;
use dictionary::{DataDictionary, DictionaryEntryRef};
use data::VR;
use self::entries::ENTRIES;

lazy_static! {
    static ref DICT: StandardDataDictionary = {
        init_dictionary()
    };
}

/// Retrieve a singleton instance of the standard dictionary.
pub fn get_instance() -> &'static StandardDataDictionary {
    &DICT
}


/// The data struct for the standard dictionary.
#[derive(Debug)]
pub struct StandardDataDictionary {
    name_to_pair: HashMap<&'static str, &'static DictionaryEntryRef<'static>>,
    pair_to_name: HashMap<Tag, &'static DictionaryEntryRef<'static>>
}

impl StandardDataDictionary {
    fn new() -> StandardDataDictionary {
        StandardDataDictionary {
            name_to_pair: HashMap::new(),
            pair_to_name: HashMap::new()
        }
    }
    
    fn index(&mut self, entry: &'static DictionaryEntryRef<'static>) -> &mut Self {
        self.name_to_pair.insert(entry.alias, entry);
        self.pair_to_name.insert(entry.tag, entry);
        self
    }
}

impl DataDictionary for StandardDataDictionary {
    type Entry = DictionaryEntryRef<'static>;

    fn get_by_name(&self, name: &str) -> Option<&Self::Entry> {
        self.name_to_pair.get(name).map(|r| { *r })
    }

    fn get_by_tag(&self, tag: Tag) -> Option<&Self::Entry> {
        self.pair_to_name.get(&tag).map(|r| { *r })
    }
}

impl<'a> DataDictionary for &'a StandardDataDictionary {
    type Entry = DictionaryEntryRef<'static>;

    fn get_by_name(&self, name: &str) -> Option<&'static DictionaryEntryRef<'static>> {
        (*self).name_to_pair.get(name).map(|r| { *r })
    }

    fn get_by_tag(&self, tag: Tag) -> Option<&'static DictionaryEntryRef<'static>> {
        (*self).pair_to_name.get(&tag).map(|r| { *r })
    }
}

impl DataDictionary for Box<StandardDataDictionary> {
    type Entry = DictionaryEntryRef<'static>;

    fn get_by_name(&self, name: &str) -> Option<&'static DictionaryEntryRef<'static>> {
        (*self).name_to_pair.get(name).map(|r| { *r })
    }

    fn get_by_tag(&self, tag: Tag) -> Option<&'static DictionaryEntryRef<'static>> {
        (*self).pair_to_name.get(&tag).map(|r| { *r })
    }
}

impl Display for StandardDataDictionary {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.write_str("Standard Attribute Dictionary")
    }
}

fn init_dictionary() -> StandardDataDictionary {
    let mut d = StandardDataDictionary::new();
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
const META_ENTRIES : &'static [E<'static>] = &[
    E {tag: Tag(0x0002,0x0000), alias: "FileMetaInformationGroupLength", vr: VR::UL},
    E {tag: Tag(0x0002,0x0001), alias: "FileMetaInformationVersion", vr: VR::OB},
    E {tag: Tag(0x0002,0x0002), alias: "MediaStorageSOPClassUID", vr: VR::UI},
    E {tag: Tag(0x0002,0x0003), alias: "MediaStorageSOPInstanceUID", vr: VR::UI},
    E {tag: Tag(0x0002,0x0010), alias: "TransferSyntaxUID", vr: VR::UI},
    E {tag: Tag(0x0002,0x0012), alias: "ImplementationClassUID", vr: VR::UI},
    E {tag: Tag(0x0002,0x0013), alias: "ImplentationVersionName", vr: VR::SH},
    E {tag: Tag(0x0002,0x0016), alias: "SourceApplicationEntityTitle", vr: VR::AE},
    E {tag: Tag(0x0002,0x0017), alias: "SendingApplicationEntityTitle", vr: VR::AE},
    E {tag: Tag(0x0002,0x0018), alias: "ReceivingApplicationEntityTitle", vr: VR::AE},
    E {tag: Tag(0x0002,0x0100), alias: "PrivateInformationCreatorUID", vr: VR::UI},
    E {tag: Tag(0x0002,0x0102), alias: "PrivateInformation", vr: VR::OB},
];
