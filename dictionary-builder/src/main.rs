//! A simple application that downloads the data dictionary and creates code or
//! data to reproduce it in the core library.
//!
//! ### How to use
//!
//! Simply run the application. It will automatically retrieve the dictionary
//! from the DCMTK data dictionary and store the result in "tags.rs".
//! Future versions will enable different kinds of outputs.
//!
//! Please use the `--help` flag for the full usage information.

use clap::{App, Arg};
use serde::Serialize;
use regex::Regex;

use heck::ShoutySnakeCase;
use std::{fs::{create_dir_all, File}, io::BufWriter};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// url to DCMTK dic file
const DEFAULT_LOCATION: &str = "https://raw.githubusercontent.com/DCMTK/dcmtk/master/dcmdata/data/dicom.dic";

#[derive(Debug, Copy, Clone, PartialEq)]
enum RetiredOptions {
    /// ignore retired data attributes
    Ignore,
    /// include retired data attributes
    Include {
        /// mark constants as deprecated
        deprecate: bool,
    }
}

fn main() {
    let matches = App::new("DICOM Dictionary Builder")
        .version("0.1.0")
        .arg(
            Arg::with_name("FROM")
                .default_value(DEFAULT_LOCATION)
                .help("Where to fetch the dictionary from"),
        )
        .arg(
            Arg::with_name("no-retired")
                .long("no-retired")
                .help("Whether to ignore retired tags")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("deprecate-retired")
                .long("deprecate-retired")
                .help("Whether to mark tag constants as deprecated")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("OUTPUT")
                .short("o")
                .help("The path to the output file")
                .default_value("tags.rs")
                .takes_value(true),
        )
        .get_matches();

    let ignore_retired = matches.is_present("no-retired");

    let retired =  if ignore_retired {
        RetiredOptions::Ignore
    } else {
        RetiredOptions::Include {
            deprecate: matches.is_present("deprecate-retired"),
        }
    };

    let src = matches.value_of("FROM").unwrap();

    let dst = Path::new(matches.value_of("OUTPUT").unwrap());

    if src.starts_with("http:") || src.starts_with("https:") {
        // read from URL
        println!("Downloading DICOM dictionary ...");
        let mut resp = reqwest::blocking::get(src).unwrap();
        let mut data = vec![];
        resp.copy_to(&mut data).unwrap();

        let preamble = data.split(|&b| b == b'\n')
            .filter_map(|l| std::str::from_utf8(l).ok())
            .find(|l| l.contains("Copyright"))
            .unwrap_or("");
        let preamble = format!(
            "Adapted from the DCMTK project.\nURL: {}\nLicense: {}\n{}",
            src,
            "https://github.com/DCMTK/dcmtk/blob/master/COPYRIGHT",
            preamble,
        );

        let entries = parse_entries(&*data).unwrap();
        println!("Writing to file ...");
        to_code_file(dst, entries, retired, &preamble).expect("Failed to write file");
    } else {
        // read from File
        let file = File::open(src).unwrap();
        let entries = parse_entries(BufReader::new(file)).unwrap();
        println!("Writing to file ...");
        to_code_file(dst, entries, retired, "").expect("Failed to write file");
    }
}

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;

fn parse_entries<R: BufRead>(source: R) -> DynResult<Vec<Entry>> {
    let mut result = vec![];

    for line in source.lines() {
        let line = line?;
        if line.starts_with('#') {
            continue;
        }

        // (0010,0010)	PN	PatientName	1	DICOM
        let parts: Vec<&str> = line.split('\t').collect();

        if parts[4] == "ILLEGAL" || parts[4] == "PRIVATE" || parts[4] == "GENERIC" {
            continue;
        }

        let mut vr = parts[1].to_string();

        // Some fields are dependent upon context.
        // We may want to support this at some point, but for now,
        // let's just choose some good defaults.
        if vr == "up" {
            vr = "UL".to_string();
        }
        if vr == "xs" {
            vr = "US".to_string();
        }
        if vr == "ox" {
            vr = "OB".to_string();
        }
        if vr == "px" {
            vr = "OB".to_string();
        }
        if vr == "lt" {
            vr = "OW".to_string();
        }
        if vr == "na" {
            // These are the "Item", "ItemDelimitationItem", "SequenceDelimitationItem", etc, values.
            // Question, should we generate const values for these, and use them internally?
            continue;
        }

        let mut alias = parts[2];
        if alias.starts_with("RETIRED_") {
            alias = alias.trim_start_matches("RETIRED_");
        }
        let alias = alias.to_string();

        let tag = parts[0].to_string();

        let regex_tag = Regex::new(r"^\(([0-9A-F]{4}),([0-9A-F]{4})\)$")?;
        let regex_tag_group100 = Regex::new(r"^\(([0-9A-F]{2})00-[0-9A-F]{2}FF,([0-9A-F]{4})\)$")?;
        let regex_tag_element100 =
            Regex::new(r"^\(([0-9A-F]{4}),([0-9A-F]{2})00-[0-9A-F]{2}FF\)$")?;

        let cap = regex_tag.captures(tag.as_str());
        let tag_type;
        let tag_declaration = if let Some(cap) = cap {
            // single tag
            let group = cap.get(1).expect("capture group 1: group").as_str();
            let elem = cap.get(2).expect("capture group 2: element").as_str();
            tag_type = TagType::Single;
            format!("Tag(0x{}, 0x{})", group, elem)
        } else if let Some(cap) = regex_tag_group100.captures(tag.as_str()) {
            // tag range over groups: (ggxx, eeee)
            let group = cap.get(1).expect("capture group 1: group portion").as_str();
            let elem = cap.get(2).expect("capture group 2: element").as_str();
            tag_type = TagType::Group100;
            format!("Tag(0x{}00, 0x{})", group, elem)
        } else if let Some(cap) = regex_tag_element100.captures(tag.as_str()) {
            // tag range over elements: (gggg, eexx)
            let group = cap.get(1).expect("capture group 1: group").as_str();
            let elem = cap
                .get(2)
                .expect("capture group 2: element portion")
                .as_str();
            tag_type = TagType::Element100;
            format!("Tag(0x{}, 0x{}00)", group, elem)
        } else {
            panic!("invalid tag: {}", alias);
        };

        result.push(Entry {
            tag,
            vr,
            alias,
            vm: parts[3].to_string(),
            obs: parts[4].to_string(),
            is_retired: parts[4].contains("retired"),
            tag_declaration,
            tag_type,
        });
    }

    Ok(result)
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Serialize)]
enum TagType {
    Single,
    Group100,
    Element100,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Serialize)]
struct Entry {
    /// Tag. example: (0010,0010)
    tag: String,
    /// VR. example: PN
    vr: String,
    /// alias. example: PatientName
    alias: String,
    /// VM. example: 1
    vm: String,
    /// observation (usually "DICOM")
    obs: String,
    /// Retired field?
    is_retired: bool,
    /// Tag declaration. Example: "Tag(0x6000, 0x1102)"
    tag_declaration: String,
    /// The type the tag represents
    tag_type: TagType,
}

/// Write the tag dictionary as Rust code.
fn to_code_file<P>(
    dest_path: P,
    entries: Vec<Entry>,
    retired_options: RetiredOptions,
    preamble: &str,
) -> DynResult<()>
where
    P: AsRef<Path>,
{
    if let Some(p_dir) = dest_path.as_ref().parent() {
        create_dir_all(&p_dir)?;
    }
    let mut f = BufWriter::new(File::create(&dest_path)?);

    f.write_all(
        b"//! Automatically generated. Edit at your own risk.\n"
    )?;

    for line in preamble.split('\n') {
        writeln!(f, "//! {}", line)?;
    }

    if matches!(retired_options, RetiredOptions::Include { deprecate: true}) {
        f.write_all(b"#![allow(deprecated)]\n")?;
    }

    f.write_all(b"\n\
    use dicom_core::dictionary::{DictionaryEntryRef, TagRange, TagRange::*};\n\
    use dicom_core::Tag;\n\
    use dicom_core::VR::*;\n\n",
    )?;

    for e in &entries {
        if retired_options == RetiredOptions::Ignore && e.is_retired {
            continue;
        }

        writeln!(f, "/// {} {} {} {} {}", e.alias, e.tag, e.vr, e.vm, e.obs)?;

        let tag_type = match e.tag_type {
            TagType::Single => "Tag",
            _ => "TagRange",
        };

        let tag_set = match e.tag_type {
            TagType::Single => e.tag_declaration.clone(),
            TagType::Group100 => format!("Group100({})", e.tag_declaration),
            TagType::Element100 => format!("Element100({})", e.tag_declaration),
        };

        if e.is_retired && matches!(retired_options, RetiredOptions::Include {
            deprecate: true, ..
        }) {
            writeln!(f, "#[deprecated(note = \"Retired DICOM tag\")]")?;
        }

        writeln!(
            f,
            "#[rustfmt::skip]\npub const {}: {} = {};",
            e.alias.to_shouty_snake_case(),
            tag_type,
            tag_set,
        )?;
    }

    f.write_all(
        b"\n\n\
    type E = DictionaryEntryRef<'static>;\n\n\
    #[rustfmt::skip]\n\
    pub(crate) const ENTRIES: &[E] = &[\n",
    )?;
    for e in &entries {
        if retired_options == RetiredOptions::Ignore && e.is_retired {
            continue;
        }

        let (vr1, vr2) = e.vr.split_at(2);

        let second_vr = if vr2.is_empty() {
            format!(" /*{} */", vr2)
        } else {
            vr2.to_string()
        };

        let tag_set = match e.tag_type {
            TagType::Single => format!("Single({})", e.alias.to_shouty_snake_case()),
            _ => e.alias.to_shouty_snake_case(),
        };

        writeln!(
            f,
            "    E {{ tag: {}, alias: \"{}\", vr: {}{} }}, // {}",
            tag_set, e.alias, vr1, second_vr, e.obs
        )?;
    }
    f.write_all(b"];\n")?;

    Ok(())
}
