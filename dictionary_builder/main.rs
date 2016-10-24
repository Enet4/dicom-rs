//! A simple application that downloads the data dictionary
//! from the latest DICOM standard found online, then creates
//! code to reproduce it in the core library.
//!
//! This is a work in progress. It can already retrieve attributes with
//! very specific tags, but might skip some patterns found in the standard
//! (such as (60xx,3000), which is for overlay data). A better way to handle
//! these cases is due.
//!
//! ### How to use
//!
//! Simply run the application. It will automatically retrieve the dictionary
//! from the official DICOM website and store the result in "entries.rs".

extern crate hyper;
extern crate xml;
extern crate regex;

use hyper::client::Client;
use hyper::client::Response;
use xml::reader::{EventReader, XmlEvent};
use std::io;
use std::io::{Read, Write};
use std::fs::{File, create_dir_all};
use std::path::Path;
use regex::Regex;

/// url to PS3.6 XML file
const DEFAULT_LOCATION: &'static str = "http://dicom.nema.\
                                        org/medical/dicom/current/source/docbook/part06/part06.xml";

fn main() {

    let src = DEFAULT_LOCATION;
    let dst = Path::new("entries.rs");

    let resp = xml_from_site(src).expect("should obtain response");
    let xml_entries = XmlEntryIterator::new(resp).map(|item| item.expect("Each item should be ok"));
    to_file(dst, xml_entries).expect("Should write file");

}

fn xml_from_site<U: AsRef<str>>(url: U) -> Result<Response, hyper::Error> {
    let client = Client::new();
    client.get(url.as_ref()).send()
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct Entry {
    tag: String,
    name: Option<String>,
    alias: Option<String>,
    vr: Option<String>,
    vm: Option<String>,
    obs: Option<String>,
}

type EIt = Iterator<Item = xml::reader::Result<Entry>>;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum XmlReadingState {
    Off,
    InTableHead,
    InTable,
    InCellTag,
    InCellName,
    InCellKeyword,
    InCellVR,
    InCellVM,
    InCellObs,
    InCellUnknown,
}

struct XmlEntryIterator<R: Read> {
    parser: EventReader<R>,
    depth: u32,
    tag: Option<String>,
    name: Option<String>,
    keyword: Option<String>,
    vr: Option<String>,
    vm: Option<String>,
    obs: Option<String>,
    state: XmlReadingState,
}

impl<R: Read> XmlEntryIterator<R> {
    pub fn new(xml: R) -> XmlEntryIterator<R> {
        XmlEntryIterator {
            parser: EventReader::new(xml),
            depth: 0,
            tag: None,
            name: None,
            keyword: None,
            vr: None,
            vm: None,
            obs: None,
            state: XmlReadingState::Off,
        }
    }
}

impl<R: Read> Iterator for XmlEntryIterator<R> {
    type Item = xml::reader::Result<Entry>;
    fn next(&mut self) -> Option<xml::reader::Result<Entry>> {

        loop {
            match self.parser.next() {
                Ok(XmlEvent::StartElement { name, attributes, .. }) => {
                    self.depth += 1;
                    
                    match self.state {
                        XmlReadingState::Off => {
                            // check for attribute xml:id="table_6-1"
                            if let Some(attr_id) = attributes.iter().find(|attr| attr.name.local_name == "label") {
                                if attr_id.value == "6-1" {
                                    // entered the table!
                                    self.state = XmlReadingState::InTableHead;
                                }
                            }
                        },
                        XmlReadingState::InTableHead => {
                            if name.local_name == "tbody" {
                                self.state = XmlReadingState::InTable;
                            }
                        },
                        XmlReadingState::InTable => {
                            if name.local_name == "para" {
                                self.state = XmlReadingState::InCellTag;
                            }
                        },
                        XmlReadingState::InCellTag => {
                            if name.local_name == "para" {
                                self.state = XmlReadingState::InCellName;
                            }
                        },
                        XmlReadingState::InCellName => {
                            if name.local_name == "para" {
                                self.state = XmlReadingState::InCellKeyword;
                            }
                        },
                        XmlReadingState::InCellKeyword => {
                            if name.local_name == "para" {
                                self.state = XmlReadingState::InCellVR;
                            }
                        },
                        XmlReadingState::InCellVR => {
                            if name.local_name == "para" {
                                self.state = XmlReadingState::InCellVM;
                            }
                        },
                        XmlReadingState::InCellVM => {
                            if name.local_name == "para" {
                                self.state = XmlReadingState::InCellObs;
                            }
                        },
                        XmlReadingState::InCellObs => {
                            if name.local_name == "para" {
                                self.state = XmlReadingState::InCellUnknown;
                            }
                        },
                        _ => {}
                    }
                }
                Ok(XmlEvent::EndElement { name }) => {
                    self.depth -= 1;

                    match self.state {
                        XmlReadingState::Off => {
                        },
                        _ => if name.local_name == "tr" && self.tag.is_some() {
                            let tag = self.tag.take().unwrap();

                            let out = Entry {
                                tag: tag,
                                name: self.name.take(),
                                alias: self.keyword.take(),
                                vr: self.vr.take(),
                                vm: self.vm.take(),
                                obs: self.obs.take(),
                            };
                            self.state = XmlReadingState::InTable;
                            return Some(Ok(out));
                        } else if name.local_name == "tbody" {
                            // the table ended!
                            break;
                        }
                    }
                }
                Ok(XmlEvent::Characters(data)) => {
                    let v = Some(String::from(data.trim().replace("\u{200b}", "")));
                    match self.state {
                        XmlReadingState::InCellTag => {
                            self.tag = v;
                        },
                        XmlReadingState::InCellName => {
                            self.name = v;
                        },
                        XmlReadingState::InCellKeyword => {
                            self.keyword = v;
                        },
                        XmlReadingState::InCellVR => {
                            self.vr = v;
                        },
                        XmlReadingState::InCellVM => {
                            self.vm = v;
                        },
                        XmlReadingState::InCellObs => {
                            self.obs = v;
                        },
                        _ => {}
                    }
                }
                Ok(XmlEvent::EndDocument { .. }) => {
                    break;
                }
                Err(ref e) if e.kind() == &xml::reader::ErrorKind::UnexpectedEof => {
                    break;
                }
                Err(e) => {
                    return Some(Err(e));
                }
                _ => {}
            }
        }

        None
    }
}

fn to_file<P: AsRef<Path>, I>(dest_path: P, entries: I) -> io::Result<()>
    where I: Iterator<Item = Entry>
{
    if let Some(p_dir) = dest_path.as_ref().parent() {
        try!(create_dir_all(&p_dir));
    }
    let mut f = try!(File::create(&dest_path));

    try!(f.write_all(b"//! Automatically generated. DO NOT EDIT!\n\n\
    use attribute::dictionary::{AttributeDictionary, DictionaryEntry};\n\
    use attribute::ValueRepresentation;\n\n\
    type E<'a> = DictionaryEntry<'a>;\n\n\
    pub const ENTRIES: &'static [E<'static>] = &[\n"));

    let regex_tag = Regex::new(r"^\(([0-9A-F]{4}),([0-9A-F]{4})\)$").unwrap();

    for e in entries {
        let Entry {tag, alias, vr, obs, ..} = e;

        // sanitize components
        
        if alias.is_none() {
            continue;
        }

        let cap = regex_tag.captures(tag.as_str());
        if cap.is_none() {
            continue;
        }
        let cap = cap.unwrap();
        let group = cap.at(1).expect("capture group 1");
        let elem = cap.at(2).expect("capture group 2");

        let mut vr = vr.unwrap_or_else(|| String::from(""));
        if vr == "See Note" {
            vr = String::from("UN");
        }

        let (vr1, vr2) = vr.split_at(2);

        let mut second_vr = String::from(vr2);
        if vr2 != "" {
            second_vr = String::from("/* or ") + vr2 + " */";
        }

        let mut obs = obs.unwrap_or_else(|| String::new());
        if obs != "" {
            obs = String::from(" // ") + obs.as_str();
        }

        try!(writeln!(f, "    E {{ tag: (0x{}, 0x{}), alias: \"{}\", vr: ValueRepresentation::{}{} }},{}",
                group, elem, alias.unwrap(), vr1, second_vr, obs));
    }
    try!(f.write_all(b"];\n"));
    Ok(())
}