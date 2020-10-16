//! A CLI tool for inspecting the contents of a DICOM file.
//! Despite the name, this tool may have a different interface and output
//! from other `dcmdump` tools, and does not aim to make a drop-in
//! replacement.
//!
//! Usage:
//!
//! ```none
//! dcmdump <file.dcm>
//! ```
use colored::*;
use dicom::core::dictionary::{DataDictionary, DictionaryEntry};
use dicom::core::header::Header;
use dicom::core::value::{PrimitiveValue, Value as DicomValue};
use dicom::core::VR;
use dicom::encoding::transfer_syntax::TransferSyntaxIndex;
use dicom::object::mem::{InMemDicomObject, InMemElement};
use dicom::object::{open_file, DefaultDicomObject, FileMetaTable, StandardDataDictionary};
use dicom::transfer_syntax::TransferSyntaxRegistry;
use snafu::ErrorCompat;
use std::borrow::Cow;
use std::fmt;
use std::io::{stdout, ErrorKind, Result as IoResult, Write};

/// Exit code for missing CLI arguments or --help
const ERROR_NO: i32 = -1;
/// Exit code for when an error emerged while reading the DICOM file.
const ERROR_READ: i32 = -2;
/// Exit code for when an error emerged while dumping the file.
const ERROR_PRINT: i32 = -3;
/// Exit code for failure to set OS compatibility
const ERROR_COMPAT: i32 = -4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DumpValue<T>
where
    T: ToString,
{
    TagNum(T),
    Alias(T),
    Num(T),
    Str(T),
    DateTime(T),
    Invalid(T),
    Nothing(T),
}

impl<T> fmt::Display for DumpValue<T>
where
    T: ToString,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = match self {
            DumpValue::TagNum(v) => v.to_string().dimmed(),
            DumpValue::Alias(v) => v.to_string().bold(),
            DumpValue::Num(v) => v.to_string().cyan(),
            DumpValue::Str(v) => v.to_string().yellow(),
            DumpValue::DateTime(v) => v.to_string().green(),
            DumpValue::Invalid(v) => v.to_string().red(),
            DumpValue::Nothing(v) => v.to_string().dimmed(),
        };
        write!(f, "{}", value)
    }
}

fn report<E: 'static>(err: E)
where
    E: std::error::Error,
    E: ErrorCompat,
{
    eprintln!("[ERROR] {}", err);
    if let Some(source) = err.source() {
        eprintln!();
        eprintln!("Caused by:");
        for (i, e) in std::iter::successors(Some(source), |e| e.source()).enumerate() {
            eprintln!("   {}: {}", i, e);
        }
    }

    let env_backtrace = std::env::var("RUST_BACKTRACE").unwrap_or_default();
    let env_lib_backtrace = std::env::var("RUST_LIB_BACKTRACE").unwrap_or_default();
    if env_lib_backtrace == "1" || (env_backtrace == "1" && env_lib_backtrace != "0") {
        if let Some(backtrace) = ErrorCompat::backtrace(&err) {
            eprintln!();
            eprintln!("Backtrace:");
            eprintln!("{}", backtrace);
        }
    }
}

#[cfg(windows)]
fn os_compatibility() -> Result<(), ()> {
    control::set_virtual_terminal(true)
}

#[cfg(not(windows))]
fn os_compatibility() -> Result<(), ()> {
    Ok(())
}

fn main() {
    os_compatibility().unwrap_or_else(|_| {
        println!("Error setting OS compatibility");
        std::process::exit(ERROR_COMPAT);
    });

    let filename = ::std::env::args()
        .nth(1)
        .unwrap_or_else(|| "--help".to_string());

    if filename == "--help" || filename == "-h" {
        println!("Usage: dcmdump <FILE>");
        std::process::exit(ERROR_NO);
    }

    let obj = open_file(filename).unwrap_or_else(|e| {
        report(e);
        std::process::exit(ERROR_READ);
    });

    match dump_file(obj) {
        Err(ref e) if e.kind() == ErrorKind::BrokenPipe => {
            // handle broken pipe separately with a no-op
        }
        Err(e) => {
            eprintln!("[ERROR] {}", e);
            std::process::exit(ERROR_PRINT);
        }
        _ => {} // all good
    }
}

fn dump_file(obj: DefaultDicomObject) -> IoResult<()> {
    let mut to = stdout();
    write!(to, "# Dicom-File-Format\n\n")?;

    let meta = obj.meta();

    let width = if let Some((width, _)) = term_size::dimensions() {
        width as u32
    } else {
        120
    };

    meta_dump(&mut to, &meta, width)?;

    println!("{:-<46}", "");

    dump(&mut to, &obj, width, 0)?;

    Ok(())
}

fn meta_dump<W>(to: &mut W, meta: &FileMetaTable, width: u32) -> IoResult<()>
where
    W: ?Sized + Write,
{
    writeln!(
        to,
        "Media Storage SOP Class UID: {}",
        meta.media_storage_sop_class_uid
    )?;
    writeln!(
        to,
        "Media Storage SOP Instance UID: {}",
        meta.media_storage_sop_instance_uid
    )?;
    if let Some(ts) = TransferSyntaxRegistry.get(&meta.transfer_syntax) {
        writeln!(to, "Transfer Syntax: {} ({})", ts.uid(), ts.name())?;
    } else {
        writeln!(to, "Transfer Syntax: {} («UNKNOWN»)", meta.transfer_syntax)?;
    }
    writeln!(
        to,
        "Implementation Class UID: {}",
        meta.implementation_class_uid
    )?;

    if let Some(v) = meta.implementation_version_name.as_ref() {
        writeln!(to, "Implementation version name: {}", v)?;
    }
    if let Some(v) = meta.source_application_entity_title.as_ref() {
        writeln!(to, "Source Application Entity Title: {}", v)?;
    }

    if let Some(v) = meta.sending_application_entity_title.as_ref() {
        writeln!(to, "Sending Application Entity Title: {}", v)?;
    }

    if let Some(v) = meta.receiving_application_entity_title.as_ref() {
        writeln!(to, "Receiving Application Entity Title: {}", v)?;
    }

    if let Some(v) = meta.private_information_creator_uid.as_ref() {
        writeln!(to, "Private Information Creator UID: {}", v)?;
    }

    if let Some(v) = meta.private_information.as_ref() {
        writeln!(
            to,
            "Private Information: {}",
            format_value_list(v.iter().map(|n| format!("{:#x}", n)), width, false)
        )?;
    }

    writeln!(to)?;
    Ok(())
}

fn dump<W, D>(to: &mut W, obj: &InMemDicomObject<D>, width: u32, depth: u32) -> IoResult<()>
where
    W: ?Sized + Write,
    D: DataDictionary,
{
    for elem in obj {
        dump_element(&mut *to, &elem, width, depth)?;
    }

    Ok(())
}

fn dump_element<W, D>(to: &mut W, elem: &InMemElement<D>, width: u32, depth: u32) -> IoResult<()>
where
    W: ?Sized + Write,
    D: DataDictionary,
{
    let indent = vec![b' '; (depth * 2) as usize];
    let tag_alias = StandardDataDictionary
        .by_tag(elem.tag())
        .map(DictionaryEntry::alias)
        .unwrap_or("«Unknown Attribute»");
    to.write_all(&indent)?;
    let vm = match elem.vr() {
        VR::OB | VR::OW | VR::UN => 1,
        _ => elem.value().multiplicity(),
    };

    match elem.value() {
        DicomValue::Sequence { items, .. } => {
            writeln!(
                to,
                "{} {:35} {} ({} Item{})",
                DumpValue::TagNum(elem.tag()),
                DumpValue::Alias(tag_alias),
                elem.vr(),
                vm,
                if vm == 1 { "" } else { "s" },
            )?;
            for item in items {
                dump_item(&mut *to, item, width, depth + 2)?;
            }
            to.write_all(&indent)?;
            writeln!(
                to,
                "{} {}",
                DumpValue::TagNum("(FFFE, E0DD)"),
                DumpValue::Alias("ItemDelimitationItem"),
            )?;
        }
        DicomValue::PixelSequence {
            fragments,
            offset_table,
        } => {
            // write pixel sequence start line
            let vr = elem.vr();
            let num_items = 1 + fragments.len();
            writeln!(
                to,
                "{} PixelData {:25} {} (PixelSequence, {} Item{})",
                DumpValue::TagNum(elem.tag()),
                "",
                vr,
                num_items,
                if num_items == 1 { "" } else { "s" },
            )?;

            // write offset table
            let byte_len = offset_table.len();
            writeln!(
                to,
                "  {} pi ({:>3} bytes, 1 Item): {:48}",
                DumpValue::TagNum("(FFFE,E000)"),
                byte_len,
                item_value_summary(&offset_table, width.saturating_sub(42 + depth * 2)),
            )?;

            // write compressed fragments
            for fragment in fragments {
                let byte_len = fragment.len();
                writeln!(
                    to,
                    "  {} pi ({:>3} bytes, 1 Item): {:48}",
                    DumpValue::TagNum("(FFFE,E000)"),
                    byte_len,
                    item_value_summary(&fragment, width.saturating_sub(42 + depth * 2)),
                )?;
            }
        }
        DicomValue::Primitive(value) => {
            let vr = elem.vr();
            let byte_len = value.calculate_byte_len();
            writeln!(
                to,
                "{} {:35} {} ({},{:>3} bytes): {}",
                DumpValue::TagNum(elem.tag()),
                DumpValue::Alias(tag_alias),
                vr,
                vm,
                byte_len,
                value_summary(&value, vr, width.saturating_sub(68 + depth * 2)),
            )?;
        }
    }

    Ok(())
}

fn dump_item<W, D>(to: &mut W, item: &InMemDicomObject<D>, width: u32, depth: u32) -> IoResult<()>
where
    W: ?Sized + Write,
    D: DataDictionary,
{
    let indent: String = std::iter::repeat(' ').take((depth * 2) as usize).collect();
    writeln!(
        to,
        "{}{} na {}",
        DumpValue::TagNum("(FFFE,E000)"),
        indent,
        DumpValue::Alias("Item"),
    )?;
    dump(to, item, width, depth + 1)?;
    writeln!(
        to,
        "{}{} {}",
        DumpValue::TagNum("(FFFE,E00D)"),
        indent,
        DumpValue::Alias("ItemDelimitationItem"),
    )?;
    Ok(())
}

fn value_summary(value: &PrimitiveValue, vr: VR, max_characters: u32) -> DumpValue<String> {
    use PrimitiveValue::*;
    match (value, vr) {
        (F32(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (F64(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (I32(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (I64(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (U32(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (U64(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (I16(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (U16(values), VR::OW) => DumpValue::Num(format_value_list(
            values.into_iter().map(|n| format!("{:02X}", n)),
            max_characters,
            false,
        )),
        (U16(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (U8(values), VR::OB) | (U8(values), VR::UN) => DumpValue::Num(format_value_list(
            values.into_iter().map(|n| format!("{:02X}", n)),
            max_characters,
            false,
        )),
        (U8(values), _) => DumpValue::Num(format_value_list(values, max_characters, false)),
        (Tags(values), _) => DumpValue::Str(format_value_list(values, max_characters, false)),
        (Strs(values), VR::DA) => {
            match value.to_multi_date() {
                Ok(values) => {
                    // print as reformatted date
                    DumpValue::DateTime(format_value_list(values, max_characters, false))
                }
                Err(_e) => {
                    // print as text
                    DumpValue::Invalid(format_value_list(values, max_characters, true))
                }
            }
        }
        (Strs(values), VR::TM) => {
            match value.to_multi_time() {
                Ok(values) => {
                    // print as reformatted date
                    DumpValue::DateTime(format_value_list(values, max_characters, false))
                }
                Err(_e) => {
                    // print as text
                    DumpValue::Invalid(format_value_list(values, max_characters, true))
                }
            }
        }
        (Strs(values), VR::DT) => {
            match value.to_multi_datetime(dicom::core::chrono::FixedOffset::east(0)) {
                Ok(values) => {
                    // print as reformatted date
                    DumpValue::DateTime(format_value_list(values, max_characters, false))
                }
                Err(_e) => {
                    // print as text
                    DumpValue::Invalid(format_value_list(values, max_characters, true))
                }
            }
        }
        (Strs(values), _) => DumpValue::Str(format_value_list(values, max_characters, true)),
        (Date(values), _) => DumpValue::DateTime(format_value_list(values, max_characters, true)),
        (Time(values), _) => DumpValue::DateTime(format_value_list(values, max_characters, true)),
        (DateTime(values), _) => {
            DumpValue::DateTime(format_value_list(values, max_characters, true))
        }
        (Str(value), _) => {
            DumpValue::Str(cut_str(&format!("\"{}\"", value), max_characters).to_string())
        }
        (Empty, _) => DumpValue::Nothing("".to_string()),
    }
}

fn item_value_summary(data: &[u8], max_characters: u32) -> String {
    format_value_list(
        data.iter().map(|n| format!("{:02X}", n)),
        max_characters,
        false,
    )
}

fn format_value_list<I>(values: I, max_characters: u32, quoted: bool) -> String
where
    I: IntoIterator,
    I::IntoIter: ExactSizeIterator,
    I::Item: std::fmt::Display,
{
    use itertools::Itertools;
    let values = values.into_iter();
    let len = values.len();
    let mut pieces = values.take(64).map(|piece| {
        if quoted {
            format!("\"{}\"", piece)
        } else {
            piece.to_string()
        }
    });
    let mut pieces = pieces.join(", ");
    if len > 1 {
        pieces = format!("[{}]", pieces);
    }
    cut_str(&pieces, max_characters).into_owned()
}

fn cut_str(s: &str, max_characters: u32) -> Cow<str> {
    let max = (max_characters.saturating_sub(3)) as usize;
    if s.len() > max {
        format!("{}...", &s[..max]).into()
    } else {
        s.into()
    }
}
