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
use dicom::core::dictionary::{DataDictionary, DictionaryEntry};
use dicom::core::header::Header;
use dicom::core::value::{PrimitiveValue, Value as DicomValue};
use dicom::core::VR;
use dicom::object::mem::{InMemDicomObject, InMemElement};
use dicom::object::{open_file, DefaultDicomObject, FileMetaTable, StandardDataDictionary};
use dicom::encoding::transfer_syntax::TransferSyntaxIndex;
use dicom::transfer_syntax::TransferSyntaxRegistry;
use snafu::ErrorCompat;

use term_size;

use std::borrow::Cow;
use std::io::{stdout, ErrorKind, Result as IoResult, Write};

/// Exit code for when an error emerged while reading the DICOM file.
const ERROR_READ: i32 = -2;
/// Exit code for when an error emerged while dumping the file.
const ERROR_PRINT: i32 = -3;

fn main() {
    let filename = ::std::env::args()
        .nth(1)
        .expect("Missing path to DICOM file");

    let obj = open_file(filename)
        .unwrap_or_else(|e| {
            if let Some(backtrace) = e.backtrace() {
                eprintln!("[ERROR] {}\n{}", e, backtrace);
            } else {
                eprintln!("[ERROR] {}", e);
            }
            std::process::exit(ERROR_READ);
        });

    match dump_file(obj) {
        Err(ref e) if e.kind() == ErrorKind::BrokenPipe => {
            // handle broken pipe separately with a no-op
        }
        Err(e) => {
            eprintln!("[ERROR] {}", e);
            std::process::exit(ERROR_PRINT);
        },
        _ => {},             // all good
    }
}

fn dump_file(obj: DefaultDicomObject) -> IoResult<()> {
    let mut to = stdout();
    write!(to, "# Dicom-File-Format\n\n")?;

    let meta = obj.meta();

    let width = if let Some((width, _)) = term_size::dimensions() {
        width as u32
    } else {
        48
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
    writeln!(to, "Implementation Class UID: {}", meta.implementation_class_uid)?;

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
                elem.tag(),
                tag_alias,
                elem.vr(),
                vm,
                if vm == 1 { "" } else { "s" },
            )?;
            for item in items {
                dump_item(&mut *to, item, width, depth + 2)?;
            }
            to.write_all(&indent)?;
            writeln!(to, "(FFFE, E0DD) SequenceDelimitationItem",)?;
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
                elem.tag(),
                "",
                vr,
                num_items,
                if num_items == 1 { "" } else { "s" },
            )?;

            // write offset table
            let byte_len = offset_table.len();
            writeln!(
                to,
                "  (FFFE,E000) pi ({:>3} bytes, 1 Item): {:48}",
                byte_len,
                item_value_summary(&offset_table, width.saturating_sub(42 + depth * 2)),
            )?;

            // write compressed fragments
            for fragment in fragments {
                let byte_len = fragment.len();
                writeln!(
                    to,
                    "  (FFFE,E000) pi ({:>3} bytes, 1 Item): {:48}",
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
                elem.tag(),
                tag_alias,
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
    writeln!(to, "{}(FFFE,E000) na Item", indent)?;
    dump(to, item, width, depth + 1)?;
    writeln!(to, "{}(FFFE,E00D) ItemDelimitationItem", indent,)?;
    Ok(())
}

fn value_summary(value: &PrimitiveValue, vr: VR, max_characters: u32) -> Cow<str> {
    use PrimitiveValue::*;
    match (value, vr) {
        (F32(values), _) => format_value_list(values, max_characters, false).into(),
        (F64(values), _) => format_value_list(values, max_characters, false).into(),
        (I32(values), _) => format_value_list(values, max_characters, false).into(),
        (I64(values), _) => format_value_list(values, max_characters, false).into(),
        (U32(values), _) => format_value_list(values, max_characters, false).into(),
        (U64(values), _) => format_value_list(values, max_characters, false).into(),
        (I16(values), _) => format_value_list(values, max_characters, false).into(),
        (U16(values), VR::OW) => format_value_list(
            values.into_iter().map(|n| format!("{:02X}", n)),
            max_characters,
            false,
        )
        .into(),
        (U16(values), _) => format_value_list(values, max_characters, false).into(),
        (U8(values), VR::OB) | (U8(values), VR::UN) => format_value_list(
            values.into_iter().map(|n| format!("{:02X}", n)),
            max_characters,
            false,
        )
        .into(),
        (U8(values), _) => format_value_list(values, max_characters, false).into(),
        (Tags(values), _) => format_value_list(values, max_characters, false).into(),
        (Strs(values), _) => format_value_list(values, max_characters, true).into(),
        (Date(values), _) => format_value_list(values, max_characters, true).into(),
        (Time(values), _) => format_value_list(values, max_characters, true).into(),
        (DateTime(values), _) => format_value_list(values, max_characters, true).into(),
        (Str(value), _) => cut_str(&format!("\"{}\"", value), max_characters)
            .into_owned()
            .into(),
        (Empty, _) => "".into(),
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
