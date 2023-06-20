//! Dictionary builder for unique identifier (UID) entries.
//!
//! Currently includes all UIDs found in [PS3.6 table A-1][1].
//!
//! [1]: https://dicom.nema.org/medical/dicom/current/output/chtml/part06/chapter_A.html#table_A-1

use std::{
    fs::{create_dir_all, File},
    io::{BufWriter, Write},
    path::Path,
    str::FromStr,
};

use clap::Parser;
use eyre::{Context, ContextCompat, Result};
use heck::ToShoutySnakeCase;
use sxd_document::parser;
use sxd_xpath::{Factory, Value};

use crate::common::RetiredOptions;

/// URL to DICOM standard Part 6 in XML
const DEFAULT_LOCATION: &str =
    "https://dicom.nema.org/medical/dicom/current/source/docbook/part06/part06.xml";

/// Fetch and build a dictionary of DICOM unique identifiers
#[derive(Debug, Parser)]
#[clap(name = "uids", alias = "uid")]
pub struct UidApp {
    /// Path or URL to the XML file containing the UID values tables
    #[clap(default_value(DEFAULT_LOCATION))]
    from: String,

    /// The output file
    #[clap(short('o'), default_value("uids.rs"))]
    output: String,

    /// Ignore retired UIDs
    #[clap(long)]
    ignore_retired: bool,

    /// Mark retired UIDs as deprecated
    #[clap(long)]
    deprecate_retired: bool,

    /// Whether to gate different UID types on Cargo features
    #[clap(long)]
    feature_gate: bool,
}

pub fn run(app: UidApp) -> Result<()> {
    let UidApp {
        from,
        output,
        ignore_retired,
        deprecate_retired,
        feature_gate,
    } = app;

    let src = from;
    let dst = output;

    let retired_options = RetiredOptions::from_flags(ignore_retired, deprecate_retired);

    let xml_data = if src.starts_with("http:") || src.starts_with("https:") {
        // read from URL
        println!("Downloading DICOM dictionary ...");
        let resp = ureq::get(&src).call()?;
        resp.into_string()?
    } else {
        // read from File
        println!("Reading from file {}", src);
        std::fs::read_to_string(src)?
    };

    // collect all UID values

    let entries = retrieve_uid_values(&xml_data)?;

    to_code_file(dst, entries, retired_options, feature_gate)?;

    Ok(())
}

/// A DICOM unique identifier descriptor.
struct UidEntry {
    uid: String,
    name: String,
    keyword: String,
    r#type: UidType,
    retired: bool,
}

#[derive(Debug, PartialEq)]
enum UidType {
    SopClass,
    MetaSopClass,
    TransferSyntax,
    WellKnownSopInstance,
    DicomUidsAsCodingScheme,
    CodingScheme,
    ApplicationContextName,
    ServiceClass,
    ApplicationHostingModel,
    MappingResource,
    LdapOid,
    SynchronizationFrameOfReference,
}

impl FromStr for UidType {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim() {
            "SOP Class" => Ok(UidType::SopClass),
            "Meta SOP Class" => Ok(UidType::MetaSopClass),
            "Transfer Syntax" => Ok(UidType::TransferSyntax),
            "Well-known SOP Instance" => Ok(UidType::WellKnownSopInstance),
            "DICOM UIDs as a Coding Scheme" => Ok(UidType::DicomUidsAsCodingScheme),
            "Coding Scheme" => Ok(UidType::CodingScheme),
            "Application Context Name" => Ok(UidType::ApplicationContextName),
            "Service Class" => Ok(UidType::ServiceClass),
            "Application Hosting Model" => Ok(UidType::ApplicationHostingModel),
            "Mapping Resource" => Ok(UidType::MappingResource),
            "LDAP OID" => Ok(UidType::LdapOid),
            "Synchronization Frame of Reference" => Ok(UidType::SynchronizationFrameOfReference),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for UidType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            UidType::SopClass => "SOP Class",
            UidType::MetaSopClass => "Meta SOP Class",
            UidType::TransferSyntax => "Transfer Syntax",
            UidType::WellKnownSopInstance => "Well-known SOP Instance",
            UidType::DicomUidsAsCodingScheme => "DICOM UIDs as a Coding Scheme",
            UidType::CodingScheme => "Coding Scheme",
            UidType::ApplicationContextName => "Application Context Name",
            UidType::ServiceClass => "Service Class",
            UidType::ApplicationHostingModel => "Application Hosting Modle",
            UidType::MappingResource => "Mapping Resource",
            UidType::LdapOid => "LDAP OID",
            UidType::SynchronizationFrameOfReference => "Synchronization Frame of Reference",
        };
        f.write_str(str)
    }
}

/// Collects UID values from PS3.6 table A-1
fn retrieve_uid_values<'a>(xml_data: &'a str) -> Result<Vec<UidEntry>> {
    let xml = parser::parse(xml_data)?;
    let doc = xml.as_document();

    let context = {
        let mut ctx = sxd_xpath::Context::new();
        ctx.set_namespace("xmlns", "http://docbook.org/ns/docbook");
        ctx.set_namespace("xml", "http://www.w3.org/1999/xlink");
        ctx
    };

    let factory = Factory::new();
    let table_rows_xpath = factory
        .build("//xmlns:chapter[@label='A']/xmlns:table[@label='A-1']/xmlns:tbody/xmlns:tr")
        .context("Could not compile XPath to table")?;
    let xpath = table_rows_xpath.context("No XPath was compiled")?;
    let nodes = xpath.evaluate(&context, doc.root())?;

    let uid_xpath = factory
        .build("xmlns:td[1]/xmlns:para/text()")
        .context("Could not compile XPath to UID")?
        .context("No XPath was compiled")?;
    let uid_xpath_retired = factory
        .build("xmlns:td[1]/xmlns:para/xmlns:emphasis/text()")
        .context("Could not compile XPath to retired UID")?
        .context("No XPath was compiled")?;

    let name_xpath = factory
        .build("xmlns:td[2]/xmlns:para/text()")
        .context("Could not compile XPath to UID name")?
        .context("No XPath was compiled")?;
    let name_xpath_retired = factory
        .build("xmlns:td[2]/xmlns:para/xmlns:emphasis/text()")
        .context("Could not compile XPath to retired UID name")?
        .context("No XPath was compiled")?;

    let keyword_xpath = factory
        .build("xmlns:td[3]/xmlns:para/text()")
        .context("Could not compile XPath to UID keyword")?
        .context("No XPath was compiled")?;
    let keyword_xpath_retired = factory
        .build("xmlns:td[3]/xmlns:para/xmlns:emphasis/text()")
        .context("Could not compile XPath to retired UID keyword")?
        .context("No XPath was compiled")?;

    let type_xpath = factory
        .build("xmlns:td[4]/xmlns:para/text()")
        .context("Could not compile XPath to UID type")?
        .context("No XPath was compiled")?;
    let type_xpath_retired = factory
        .build("xmlns:td[4]/xmlns:para/xmlns:emphasis/text()")
        .context("Could not compile XPath to retired UID type")?
        .context("No XPath was compiled")?;

    let nodeset = match nodes {
        Value::Nodeset(nodeset) => nodeset,
        _ => eyre::bail!("Expected node set"),
    };

    if nodeset.size() == 0 {
        eyre::bail!("No UID table found");
    }

    let mut uids = vec![];

    for node in nodeset {
        let elem = if let Some(elem) = node.element() {
            elem
        } else {
            continue;
        };

        let mut retired = false;

        // get UID type first
        let uid_type = type_xpath.evaluate(&context, elem)?;
        let uid_type = uid_type.into_string();

        // nothing but whitespace means that it is retired,
        // and the content is in emphasis
        let uid_type = if uid_type.trim().is_empty() {
            retired = true;
            type_xpath_retired.evaluate(&context, elem)?.into_string()
        } else {
            uid_type
        };

        let Ok(uid_type) = UidType::from_str(&uid_type) else {
            eprintln!("Unsupported UID type `{uid_type}`");
            continue;
        };

        // get UID
        let uid_xpath = if !retired {
            &uid_xpath
        } else {
            &uid_xpath_retired
        };
        let uid = uid_xpath.evaluate(&context, elem)?;
        let uid = uid.into_string();
        // remove whitespace and zero width spaces in UID
        let uid = uid.trim().replace('\u{200b}', "");

        // get name
        let name_xpath = if !retired {
            &name_xpath
        } else {
            &name_xpath_retired
        };
        let name = name_xpath.evaluate(&context, elem)?.into_string();
        let name = name.trim().replace('\u{200b}', "");

        if name.is_empty() {
            continue;
        }

        // get keyword
        let keyword_xpath = if !retired {
            &keyword_xpath
        } else {
            &keyword_xpath_retired
        };
        let keyword = keyword_xpath.evaluate(&context, elem)?.into_string();
        let keyword = keyword.trim().replace('\u{200b}', "");

        if keyword.is_empty() {
            continue;
        }

        uids.push(UidEntry {
            uid,
            name,
            keyword,
            r#type: uid_type,
            retired,
        });
    }

    uids.sort_by(|a, b| a.uid.cmp(&b.uid));

    println!("Retrieved {} UIDs", uids.len());

    Ok(uids)
}

/// Write the tag dictionary as Rust code.
fn to_code_file<P>(
    dest_path: P,
    entries: Vec<UidEntry>,
    retired_options: RetiredOptions,
    feature_gate: bool,
) -> Result<()>
where
    P: AsRef<Path>,
{
    if let Some(p_dir) = dest_path.as_ref().parent() {
        create_dir_all(&p_dir)?;
    }
    let mut f = BufWriter::new(File::create(&dest_path)?);

    f.write_all(b"//! UID declarations\n")?;
    f.write_all(b"// Automatically generated. Edit at your own risk.\n")?;

    if matches!(retired_options, RetiredOptions::Include { deprecate: true }) {
        f.write_all(b"#![allow(deprecated)]\n")?;
    }

    f.write_all(b"\nuse dicom_core::dictionary::UidDictionaryEntryRef;\n")?;

    for e in &entries {
        if e.retired && retired_options == RetiredOptions::Ignore {
            continue;
        }

        writeln!(f, "/// {}: {}", e.r#type, e.name,)?;
        if e.retired
            && matches!(
                retired_options,
                RetiredOptions::Include {
                    deprecate: true,
                    ..
                }
            )
        {
            writeln!(f, "#[deprecated(note = \"Retired DICOM UID\")]")?;
        }
        writeln!(
            f,
            "#[rustfmt::skip]\npub const {}: &str = {:?};",
            e.keyword.to_shouty_snake_case(),
            &e.uid,
        )?;
    }

    f.write_all(b"\n#[allow(unused_imports)]\nuse dicom_core::dictionary::UidType::*;\n")?;
    f.write_all(b"#[allow(dead_code)]\ntype E = UidDictionaryEntryRef<'static>;\n")?;

    // define an array for each kind of UID
    let listings = [
        (UidType::SopClass, "SOP_CLASSES", "sop-class"),
        (
            UidType::TransferSyntax,
            "TRANSFER_SYNTAXES",
            "transfer-syntax",
        ),
        (UidType::MetaSopClass, "META_SOP_CLASSES", "meta-sop-class"),
        (
            UidType::WellKnownSopInstance,
            "WELL_KNOWN_SOP_INSTANCES",
            "well-known-sop-instance",
        ),
        (
            UidType::DicomUidsAsCodingScheme,
            "DICOM_UIDS_AS_CODING_SCHEMES",
            "dicom-uid-as-coding-scheme",
        ),
        (UidType::CodingScheme, "CODING_SCHEMES", "coding-scheme"),
        (
            UidType::ApplicationContextName,
            "APPLICATION_CONTEXT_NAMES",
            "application-context-name",
        ),
        (UidType::ServiceClass, "SERVICE_CLASSES", "service-class"),
        (
            UidType::ApplicationHostingModel,
            "APPLICATION_HOSTING_MODELS",
            "application-hosting-model",
        ),
        (
            UidType::MappingResource,
            "MAPPING_RESOURCES",
            "mapping-resource",
        ),
        (UidType::LdapOid, "LDAP_OIDS", "ldap-oid"),
        (
            UidType::SynchronizationFrameOfReference,
            "SYNCHRONIZATION_FRAME_OF_REFERENCES",
            "synchronization-frame-of-reference",
        ),
    ];

    for (typ, entries_name, feature_name) in listings {
        write_entries(
            &mut f,
            entries_name,
            if feature_gate {
                Some(feature_name)
            } else {
                None
            },
            entries.iter().filter(|e| e.r#type == typ),
        )?;
    }

    Ok(())
}

fn write_entries<'a>(
    f: &mut BufWriter<impl Write>,
    entries_name: &'static str,
    feature_name: Option<&'static str>,
    entries: impl IntoIterator<Item = &'a UidEntry>,
) -> Result<()> {
    f.write_all(b"\n#[rustfmt::skip]\n")?;
    if let Some(feature) = feature_name {
        // conditionall include based on feature
        writeln!(f, "#[cfg(feature = \"{feature}\")]")?;
    }
    writeln!(f, "pub(crate) const {entries_name}: &[E] = &[")?;

    for e in entries {
        let uid = e.uid.replace('"', "\\\"");
        let name = e.name.replace('"', "\\\"");
        let keyword = e.keyword.replace('"', "\\\"");

        writeln!(
            f,
            "    E::new(\"{}\", \"{}\", \"{}\", {:?}, {}),",
            uid, name, keyword, e.r#type, e.retired
        )?;
    }

    f.write_all(b"];\n")?;

    Ok(())
}
