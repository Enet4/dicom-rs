//! DICOM data element (tag) dictionary builder
use std::{
    fs::{create_dir_all, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

use clap::Parser;
use eyre::{Context, Result};
use heck::ToShoutySnakeCase;
use regex::Regex;
use serde::Serialize;

use crate::common::RetiredOptions;

/// url to DCMTK dic file
const DEFAULT_LOCATION: &str =
    "https://raw.githubusercontent.com/DCMTK/dcmtk/master/dcmdata/data/dicom.dic";

/// Fetch and build a dictionary of DICOM data elements
/// (tags)
#[derive(Debug, Parser)]
#[clap(name = "data-element", alias = "tags")]
pub struct DataElementApp {
    /// Path or URL to the data element dictionary
    #[clap(default_value(DEFAULT_LOCATION))]
    from: String,
    /// The output file
    #[clap(short('o'), default_value("tags.rs"))]
    output: String,
    /// Ignore retired DICOM tags
    #[clap(long)]
    ignore_retired: bool,
    /// Mark retired DICOM tags as deprecated
    #[clap(long)]
    deprecate_retired: bool,
}

pub fn run(args: DataElementApp) -> Result<()> {
    let DataElementApp {
        from,
        ignore_retired,
        deprecate_retired,
        output,
    } = args;

    let retired = RetiredOptions::from_flags(ignore_retired, deprecate_retired);

    let src = from;
    let dst = output;

    let preamble: String;
    let entries = if src.starts_with("http:") || src.starts_with("https:") {
        // read from URL
        println!("Downloading DICOM dictionary ...");
        let resp = ureq::get(&src).call()?;
        let mut data = vec![];
        std::io::copy(&mut resp.into_body().as_reader(), &mut data)?;

        let notice = data
            .split(|&b| b == b'\n')
            .filter_map(|l| std::str::from_utf8(l).ok())
            .find(|l| l.contains("Copyright"))
            .unwrap_or("");
        preamble = format!(
            "Adapted from the DCMTK project.\nURL: <{}>\nLicense: <{}>\n{}",
            src, "https://github.com/DCMTK/dcmtk/blob/master/COPYRIGHT", notice,
        );

        parse_entries(&*data)?
    } else {
        // read from File
        let file = File::open(src)?;
        preamble = "".to_owned();
        parse_entries(BufReader::new(file))?
    };

    println!("Writing to file ...");
    to_code_file(dst, entries, retired, &preamble).context("Failed to write file")?;

    Ok(())
}

fn parse_entries<R: BufRead>(source: R) -> Result<Vec<Entry>> {
    let mut result = vec![];

    let regex_tag = Regex::new(r"^\(([0-9A-F]{4}),([0-9A-F]{4})\)$")?;
    let regex_tag_group100 = Regex::new(r"^\(([0-9A-F]{2})00-[0-9A-F]{2}FF,([0-9A-F]{4})\)$")?;
    let regex_tag_element100 =
        Regex::new(r"^\(([0-9A-F]{4}),([0-9A-F]{2})00-[0-9A-F]{2}FF\)$")?;

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

        let vr = parts[1].to_string();

        if vr == "na" {
            // These are the "Item", "ItemDelimitationItem", "SequenceDelimitationItem", etc, values.
            // Do not include them for now.
            continue;
        }

        let mut alias = parts[2];
        if alias.starts_with("RETIRED_") {
            alias = alias.trim_start_matches("RETIRED_");
        }
        let alias = alias.to_string();

        let tag = parts[0].to_string();

        let cap = regex_tag.captures(tag.as_str());
        let tag_type;
        let tag_declaration = if let Some(cap) = cap {
            // single tag
            let group = cap.get(1).expect("capture group 1: group").as_str();
            let elem = cap.get(2).expect("capture group 2: element").as_str();
            tag_type = TagType::Single;
            format!("Tag(0x{group}, 0x{elem})")
        } else if let Some(cap) = regex_tag_group100.captures(tag.as_str()) {
            // tag range over groups: (ggxx, eeee)
            let group = cap.get(1).expect("capture group 1: group portion").as_str();
            let elem = cap.get(2).expect("capture group 2: element").as_str();
            tag_type = TagType::Group100;
            format!("Tag(0x{group}00, 0x{elem})")
        } else if let Some(cap) = regex_tag_element100.captures(tag.as_str()) {
            // tag range over elements: (gggg, eexx)
            let group = cap.get(1).expect("capture group 1: group").as_str();
            let elem = cap
                .get(2)
                .expect("capture group 2: element portion")
                .as_str();
            tag_type = TagType::Element100;
            format!("Tag(0x{group}, 0x{elem}00)")
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
) -> Result<()>
where
    P: AsRef<Path>,
{
    if let Some(p_dir) = dest_path.as_ref().parent() {
        create_dir_all(p_dir)?;
    }
    let mut f = BufWriter::new(File::create(&dest_path)?);

    f.write_all(b"//! Data element tag declarations\n//!\n")?;

    for line in preamble.split('\n') {
        writeln!(f, "//! {line}\\")?;
    }
    f.write_all(b"// Automatically generated. Edit at your own risk.\n")?;

    if matches!(retired_options, RetiredOptions::Include { deprecate: true }) {
        f.write_all(b"#![allow(deprecated)]\n")?;
    }

    f.write_all(
        b"\n\
    use dicom_core::dictionary::{DataDictionaryEntryRef, TagRange, TagRange::*, VirtualVr::*};\n\
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

        if e.is_retired
            && matches!(
                retired_options,
                RetiredOptions::Include {
                    deprecate: true,
                    ..
                }
            )
        {
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
        b"\ntype E = DataDictionaryEntryRef<'static>;\n\n\
    #[rustfmt::skip]\n\
    pub(crate) const ENTRIES: &[E] = &[\n",
    )?;
    for e in &entries {
        if retired_options == RetiredOptions::Ignore && e.is_retired {
            continue;
        }
        // Some fields are dependent upon context
        let (vr1, vr2, vr3) = match &*e.vr {
            "xs" => ("Xs", "", ""),
            "ox" => ("Ox", "", ""),
            "px" => ("Px", "", ""),
            "lt" => ("Lt", "", ""),
            // for now we will always map "up" to "UL"
            "up" => ("Exact(", "UL", ")"),
            // assume exact in all other cases
            _ => ("Exact(", &*e.vr, ")"),
        };

        let tag_set = match e.tag_type {
            TagType::Single => format!("Single({})", e.alias.to_shouty_snake_case()),
            _ => e.alias.to_shouty_snake_case(),
        };

        writeln!(
            f,
            "    E {{ tag: {}, alias: \"{}\", vr: {}{}{} }}, // {}",
            tag_set, e.alias, vr1, vr2, vr3, e.obs
        )?;
    }
    f.write_all(b"];\n")?;

    Ok(())
}
