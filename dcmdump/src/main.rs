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
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::io::{stdout, ErrorKind, Result as IoResult, Write};
use std::path::PathBuf;
use structopt::StructOpt;

/// Exit code for when an error emerged while reading the DICOM file.
const ERROR_READ: i32 = -2;
/// Exit code for when an error emerged while dumping the file.
const ERROR_PRINT: i32 = -3;

#[derive(Debug)]
struct ColoringError { }

#[derive(Clone, Copy, Debug)]
enum Coloring {
    Never,
    Auto,
    Always,
}

impl Display for ColoringError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid color mode")
    }
}

impl std::error::Error for Coloring {}

impl FromStr for Coloring {
    type Err = ColoringError;
    fn from_str(color: &str) -> Result<Self, Self::Err> {
        match color {
            "never" => Ok(Coloring::Never),
            "auto" => Ok(Coloring::Auto),
            "always" => Ok(Coloring::Always),
            _ => Err(ColoringError{})
        }
    }
}

impl std::fmt::Display for Coloring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Coloring::Never => f.write_str("never"),
            Coloring::Auto => f.write_str("auto"),
            Coloring::Always => f.write_str("always"),
        }
    }
}

/// Dump the contents of DICOM files
#[derive(Debug, StructOpt)]
struct App {
    /// The DICOM file to read
    file: PathBuf,
    /// whether text value width limit is disabled
    /// (limited to `width` by default)
    #[structopt(long = "no-text-limit")]
    no_text_limit: bool,
    /// the width of the display
    /// (default is to check automatically)
    #[structopt(short = "w", long = "width")]
    width: Option<u32>,
    /// color mode
    #[structopt(long="color", default_value = "auto")]
    color: Coloring
}

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
    Nothing,
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
            DumpValue::Nothing => "(no value)".italic(),
        };
        if let Some(width) = f.width() {
            write!(f, "{:width$}", value, width = width)
        } else {
            write!(f, "{}", value)
        }
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
        eprintln!("Error setting OS compatibility for colored output");
    });

    let App {
        file: filename,
        no_text_limit,
        width,
        color,
    } = App::from_args();

    let obj = open_file(filename).unwrap_or_else(|e| {
        report(e);
        std::process::exit(ERROR_READ);
    });

    let width = width
        .or_else(|| term_size::dimensions().map(|(width, _)| width as u32))
        .unwrap_or(120);

    match color {
        Coloring::Never => colored::control::set_override(false),
        Coloring::Always => colored::control::set_override(true),
        _ => {}
    }

    match dump_file(obj, width, no_text_limit) {
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

fn dump_file(obj: DefaultDicomObject, width: u32, no_text_limit: bool) -> IoResult<()> {
    let mut to = stdout();
    let meta = obj.meta();

    meta_dump(&mut to, &meta, width)?;

    println!("{:-<58}", "");

    dump(&mut to, &obj, width, 0, no_text_limit)?;

    Ok(())
}

#[inline]
fn whitespace_or_null(c: char) -> bool {
    c.is_whitespace() || c == '\0'
}

fn meta_dump<W>(to: &mut W, meta: &FileMetaTable, width: u32) -> IoResult<()>
where
    W: ?Sized + Write,
{
    writeln!(
        to,
        "{}: {}",
        "Media Storage SOP Class UID".bold(),
        meta.media_storage_sop_class_uid
            .trim_end_matches(whitespace_or_null),
    )?;
    writeln!(
        to,
        "{}: {}",
        "Media Storage SOP Instance UID".bold(),
        meta.media_storage_sop_instance_uid
            .trim_end_matches(whitespace_or_null),
    )?;
    if let Some(ts) = TransferSyntaxRegistry.get(&meta.transfer_syntax) {
        writeln!(
            to,
            "{}: {} ({})",
            "Transfer Syntax".bold(),
            ts.uid(),
            ts.name()
        )?;
    } else {
        writeln!(
            to,
            "{}: {} («UNKNOWN»)",
            "Transfer Syntax".bold(),
            meta.transfer_syntax.trim_end_matches(whitespace_or_null)
        )?;
    }
    writeln!(
        to,
        "{}: {}",
        "Implementation Class UID".bold(),
        meta.implementation_class_uid
            .trim_end_matches(whitespace_or_null),
    )?;

    if let Some(v) = meta.implementation_version_name.as_ref() {
        writeln!(to, "{}: {}", "Implementation version name".bold(), v.trim_end())?;
    }

    if let Some(v) = meta.source_application_entity_title.as_ref() {
        writeln!(to, "{}: {}", "Source Application Entity Title".bold(), v.trim_end())?;
    }

    if let Some(v) = meta.sending_application_entity_title.as_ref() {
        writeln!(to, "{}: {}", "Sending Application Entity Title".bold(), v.trim_end())?;
    }

    if let Some(v) = meta.receiving_application_entity_title.as_ref() {
        writeln!(to, "{}: {}", "Receiving Application Entity Title".bold(), v.trim_end())?;
    }

    if let Some(v) = meta.private_information_creator_uid.as_ref() {
        writeln!(
            to,
            "{}: {}",
            "Private Information Creator UID".bold(),
            v.trim_end_matches(whitespace_or_null)
        )?;
    }

    if let Some(v) = meta.private_information.as_ref() {
        writeln!(to, "{}: {}",
            "Private Information".bold(),
            format_value_list(v.iter().map(|n| format!("{:02X}", n)), width, false)
        )?;
    }

    writeln!(to)?;
    Ok(())
}

fn dump<W, D>(
    to: &mut W,
    obj: &InMemDicomObject<D>,
    width: u32,
    depth: u32,
    no_text_limit: bool,
) -> IoResult<()>
where
    W: ?Sized + Write,
    D: DataDictionary,
{
    for elem in obj {
        dump_element(&mut *to, &elem, width, depth, no_text_limit)?;
    }

    Ok(())
}

fn dump_element<W, D>(
    to: &mut W,
    elem: &InMemElement<D>,
    width: u32,
    depth: u32,
    no_text_limit: bool,
) -> IoResult<()>
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
                "{} {:28} {} ({} Item{})",
                DumpValue::TagNum(elem.tag()),
                DumpValue::Alias(tag_alias),
                elem.vr(),
                vm,
                if vm == 1 { "" } else { "s" },
            )?;
            for item in items {
                dump_item(&mut *to, item, width, depth + 2, no_text_limit)?;
            }
            to.write_all(&indent)?;
            writeln!(
                to,
                "{} {}",
                DumpValue::TagNum("(FFFE,E0DD)"),
                DumpValue::Alias("SequenceDelimitationItem"),
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
                "{} {:28} {} (PixelSequence, {} Item{})",
                DumpValue::TagNum(elem.tag()),
                "PixelData".bold(),
                vr,
                num_items,
                if num_items == 1 { "" } else { "s" },
            )?;

            // write offset table
            let byte_len = offset_table.len();
            writeln!(
                to,
                "  {} offset table ({:>3} bytes, 1 Item): {:48}",
                DumpValue::TagNum("(FFFE,E000)"),
                byte_len,
                offset_table_summary(&offset_table, width.saturating_sub(38 + depth * 2)),
            )?;

            // write compressed fragments
            for fragment in fragments {
                let byte_len = fragment.len();
                writeln!(
                    to,
                    "  {} pi ({:>3} bytes, 1 Item): {:48}",
                    DumpValue::TagNum("(FFFE,E000)"),
                    byte_len,
                    item_value_summary(&fragment, width.saturating_sub(38 + depth * 2)),
                )?;
            }
        }
        DicomValue::Primitive(value) => {
            let vr = elem.vr();
            let byte_len = value.calculate_byte_len();
            writeln!(
                to,
                "{} {:28} {} ({},{:>3} bytes): {}",
                DumpValue::TagNum(elem.tag()),
                DumpValue::Alias(tag_alias),
                vr,
                vm,
                byte_len,
                value_summary(
                    &value,
                    vr,
                    width.saturating_sub(63 + depth * 2),
                    no_text_limit
                ),
            )?;
        }
    }

    Ok(())
}

fn dump_item<W, D>(
    to: &mut W,
    item: &InMemDicomObject<D>,
    width: u32,
    depth: u32,
    no_text_limit: bool,
) -> IoResult<()>
where
    W: ?Sized + Write,
    D: DataDictionary,
{
    let indent: String = std::iter::repeat(' ').take((depth * 2) as usize).collect();
    writeln!(
        to,
        "{}{} na {}",
        indent,
        DumpValue::TagNum("(FFFE,E000)"),
        DumpValue::Alias("Item"),
    )?;
    dump(to, item, width, depth + 1, no_text_limit)?;
    writeln!(
        to,
        "{}{} {}",
        indent,
        DumpValue::TagNum("(FFFE,E00D)"),
        DumpValue::Alias("ItemDelimitationItem"),
    )?;
    Ok(())
}

fn value_summary(
    value: &PrimitiveValue,
    vr: VR,
    max_characters: u32,
    no_text_limit: bool,
) -> DumpValue<String> {
    use PrimitiveValue::*;

    let max = if no_text_limit {
        match vr {
            VR::CS
            | VR::AE
            | VR::DA
            | VR::DS
            | VR::DT
            | VR::IS
            | VR::LO
            | VR::LT
            | VR::PN
            | VR::TM
            | VR::UC
            | VR::UI
            | VR::UR => u32::MAX,
            _ => max_characters,
        }
    } else {
        max_characters
    };
    match (value, vr) {
        (F32(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (F64(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (I32(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (I64(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (U32(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (U64(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (I16(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (U16(values), VR::OW) => DumpValue::Num(format_value_list(
            values.into_iter().map(|n| format!("{:02X}", n)),
            max,
            false,
        )),
        (U16(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (U8(values), VR::OB) | (U8(values), VR::UN) => DumpValue::Num(format_value_list(
            values.into_iter().map(|n| format!("{:02X}", n)),
            max,
            false,
        )),
        (U8(values), _) => DumpValue::Num(format_value_list(values, max, false)),
        (Tags(values), _) => DumpValue::Str(format_value_list(values, max, false)),
        (Strs(values), VR::DA) => {
            match value.to_multi_date() {
                Ok(values) => {
                    // print as reformatted date
                    DumpValue::DateTime(format_value_list(values, max, false))
                }
                Err(_e) => {
                    // print as text
                    DumpValue::Invalid(format_value_list(values, max, true))
                }
            }
        }
        (Strs(values), VR::TM) => {
            match value.to_multi_time() {
                Ok(values) => {
                    // print as reformatted date
                    DumpValue::DateTime(format_value_list(values, max, false))
                }
                Err(_e) => {
                    // print as text
                    DumpValue::Invalid(format_value_list(values, max, true))
                }
            }
        }
        (Strs(values), VR::DT) => {
            match value.to_multi_datetime(dicom::core::chrono::FixedOffset::east(0)) {
                Ok(values) => {
                    // print as reformatted date
                    DumpValue::DateTime(format_value_list(values, max, false))
                }
                Err(_e) => {
                    // print as text
                    DumpValue::Invalid(format_value_list(values, max, true))
                }
            }
        }
        (Strs(values), _) => DumpValue::Str(format_value_list(
            values
                .iter()
                .map(|s| s.trim_end_matches(whitespace_or_null)),
            max,
            true,
        )),
        (Date(values), _) => DumpValue::DateTime(format_value_list(values, max, true)),
        (Time(values), _) => DumpValue::DateTime(format_value_list(values, max, true)),
        (DateTime(values), _) => DumpValue::DateTime(format_value_list(values, max, true)),
        (Str(value), _) => DumpValue::Str(
            cut_str(
                &format!(
                    "\"{}\"",
                    value.to_string().trim_end_matches(whitespace_or_null)
                ),
                max,
            )
            .to_string(),
        ),
        (Empty, _) => DumpValue::Nothing,
    }
}

fn item_value_summary(data: &[u8], max_characters: u32) -> DumpValue<String> {
    DumpValue::Num(format_value_list(
        data.iter().map(|n| format!("{:02X}", n)),
        max_characters,
        false,
    ))
}

fn offset_table_summary(data: &[u32], max_characters: u32) -> String {
    if data.is_empty() {
        format!("{}", "(empty)".italic())
    } else {
        format_value_list(
            data.iter().map(|n| format!("{:02X}", n)),
            max_characters,
            false,
        )
    }
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
            let mut piece = piece.to_string();
            if piece.contains('\"') {
                format!("\"{}\"", piece.replace("\"", "\\\""))
            } else {
                piece.insert(0, '"');
                piece.push('"');
                piece
            }
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
    let len = s.chars().count();

    if len > max {
        s.chars()
            .take(max)
            .chain("...".chars())
            .collect::<String>()
            .into()
    } else {
        s.into()
    }
}

#[cfg(test)]
mod tests {

    use super::whitespace_or_null;

    #[test]
    fn trims_all_whitespace() {
        assert_eq!("   ".trim_end_matches(whitespace_or_null), "");
        assert_eq!("\0".trim_end_matches(whitespace_or_null), "");
        assert_eq!("1.4.5.6\0".trim_end_matches(whitespace_or_null), "1.4.5.6");
        assert_eq!("AETITLE ".trim_end_matches(whitespace_or_null), "AETITLE");
    }
}
