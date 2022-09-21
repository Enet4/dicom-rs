//! Service-Object-Pair (SOP) class UID dictionary builder

use std::{
    fs::{create_dir_all, File},
    io::{BufWriter, Write},
    path::Path,
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

/// Fetch and build a dictionary of DICOM SOP classes
#[derive(Debug, Parser)]
#[clap(name = "sop", alias = "sop-class")]
pub struct SopClassApp {
    /// Path or URL to the SOP class dictionary
    #[clap(default_value(DEFAULT_LOCATION))]
    from: String,

    /// The output file
    #[clap(short('o'), default_value("sop.rs"))]
    output: String,

    /// Ignore retired SOP classes
    #[clap(long)]
    ignore_retired: bool,

    /// Mark retired SOP classes as deprecated
    #[clap(long)]
    deprecate_retired: bool,
}

pub fn run(app: SopClassApp) -> Result<()> {
    let SopClassApp {
        from,
        output,
        ignore_retired,
        deprecate_retired,
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

    let sop_classes = retrieve_sop_classes(&xml_data)?;

    to_code_file(dst, sop_classes, retired_options)?;

    Ok(())
}

/// An SOP class descriptor
struct SopClass {
    uid: String,
    name: String,
    keyword: String,
    retired: bool,
}

fn retrieve_sop_classes<'a>(xml_data: &'a str) -> Result<Vec<SopClass>> {
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

    let mut sop_classes = vec![];

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

        if uid_type.trim() != "SOP Class" {
            continue;
        }

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
        let name = name.trim().to_owned();

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

        sop_classes.push(SopClass {
            uid,
            name,
            keyword,
            retired,
        });
    }

    sop_classes.sort_by(|a, b| a.uid.cmp(&b.uid));

    println!("Retrieved {} SOP classes", sop_classes.len());

    Ok(sop_classes)
}

/// Write the tag dictionary as Rust code.
fn to_code_file<P>(
    dest_path: P,
    entries: Vec<SopClass>,
    retired_options: RetiredOptions,
) -> Result<()>
where
    P: AsRef<Path>,
{
    if let Some(p_dir) = dest_path.as_ref().parent() {
        create_dir_all(&p_dir)?;
    }
    let mut f = BufWriter::new(File::create(&dest_path)?);

    f.write_all(b"//! Automatically generated. Edit at your own risk.\n")?;

    if matches!(retired_options, RetiredOptions::Include { deprecate: true }) {
        f.write_all(b"#![allow(deprecated)]\n")?;
    }

    for e in &entries {
        if e.retired && retired_options == RetiredOptions::Ignore {
            continue;
        }

        writeln!(
            f,
            "/// {}{}",
            e.name,
            if e.retired { " (RETIRED)" } else { "" }
        )?;
        if e.retired
            && matches!(
                retired_options,
                RetiredOptions::Include {
                    deprecate: true,
                    ..
                }
            )
        {
            writeln!(f, "#[deprecated(note = \"Retired DICOM SOP Class\")]")?;
        }
        writeln!(
            f,
            "#[rustfmt::skip]\npub const {}: &str = {:?};",
            e.keyword.to_shouty_snake_case(),
            &e.uid,
        )?;
    }

    f.write_all(
        b"\n\n\
    type E = SopEntryRef<'static>;\n\n\
    #[rustfmt::skip]\n\
    pub(crate) const ENTRIES: &[E] = &[\n",
    )?;

    for e in entries {
        let uid = e.uid.replace('"', "\\\"");
        let name = e.name.replace('"', "\\\"");
        let keyword = e.keyword.replace('"', "\\\"");
        let end = if e.retired { " // RETIRED" } else { "" };

        writeln!(
            f,
            "    E {{ uid: \"{}\", name: \"{}\", keyword: \"{}\" }},{}",
            uid, name, keyword, end,
        )?;
    }

    f.write_all(b"];\n")?;

    Ok(())
}
