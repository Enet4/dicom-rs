//! A reimplementation of dcmdump in Rust.
//! This is a work in progress, it is not guaranteed to work yet!
use dicom_core::dictionary::{DataDictionary, DictionaryEntry};
use dicom_core::header::Header;
use dicom_core::value::{PrimitiveValue, Value as DicomValue};
use dicom_core::VR;
use dicom_object::file::open_file;
use dicom_object::mem::{InMemDicomObject, InMemElement};
use dicom_object::StandardDataDictionary;

use std::borrow::Cow;
use std::io::{stdout, Write};

type DynResult<T> = Result<T, Box<::std::error::Error>>;

fn main() -> DynResult<()> {
    let filename = ::std::env::args()
        .nth(1)
        .expect("Missing path to DICOM file");

    let obj = open_file(filename)?;
    let mut to = stdout();
    write!(to, "# Dicom-File-Format\n\n")?;
    dump(&mut to, &obj, 40, 0)?;

    Ok(())
}

fn dump<W, D>(to: &mut W, obj: &InMemDicomObject<D>, width: u32, depth: u32) -> DynResult<()>
where
    W: Write,
    D: DataDictionary,
{
    for elem in obj {
        dump_element(&mut *to, &elem, width, depth)?;
    }

    Ok(())
}

fn dump_element<W, D>(to: &mut W, elem: &InMemElement<D>, width: u32, depth: u32) -> DynResult<()>
where
    W: Write,
    D: DataDictionary,
{
    let indent = vec![b' '; (depth * 2) as usize];
    let tag_alias = StandardDataDictionary
        .by_tag(elem.tag())
        .map(DictionaryEntry::alias)
        .unwrap_or("«Unknown Attribute»");
    to.write(&indent)?;
    let vm = match elem.vr() {
        VR::OB | VR::OW | VR::UN => 1,
        _ => elem.value().multiplicity(),
    };

    if let &DicomValue::Sequence { ref items, .. } = elem.value() {
        writeln!(
            to,
            "{} {}                        # {},    {}",
            elem.tag(),
            elem.vr(),
            vm,
            tag_alias
        )?;
        for item in items {
            dump_item(&mut *to, item, width, depth + 1)?;
        }
    } else {
        let vr = elem.vr();
        let value = elem.value().primitive().unwrap();
        let byte_len = value.calculate_byte_len();
        writeln!(
            to,
            "{} {} {:40} # {}, {} {}",
            elem.tag(),
            vr,
            value_summary(&value, vr, width),
            byte_len,
            vm,
            tag_alias
        )?;
    }

    Ok(())
}

fn dump_item<W, D>(to: &mut W, item: &InMemDicomObject<D>, width: u32, depth: u32) -> DynResult<()>
where
    W: Write,
    D: DataDictionary,
{
    let indent: String = std::iter::repeat(' ').take((depth * 2) as usize).collect();
    let trail: String = std::iter::repeat(' ')
        .take(usize::max(21, width as usize - 21 - indent.len()))
        .collect();
    writeln!(to, "{}(FFFE,E000) na Item {} # 0, 0 Item", indent, trail)?;
    dump(to, item, width, depth + 1)?;
    writeln!(
        to,
        "(FFFE,E00D) na (ItemDelimitationItem)  # 0, 0 ItemDelimitationItem"
    )?;
    Ok(())
}

fn value_summary(value: &PrimitiveValue, vr: VR, max_characters: u32) -> Cow<str> {
    use PrimitiveValue::*;
    match (value, vr) {
        (F32(values), _) => format_value_list(values, max_characters).into(),
        (F64(values), _) => format_value_list(values, max_characters).into(),
        (I32(values), _) => format_value_list(values, max_characters).into(),
        (U32(values), _) => format_value_list(values, max_characters).into(),
        (I16(values), _) => format_value_list(values, max_characters).into(),
        (U16(values), VR::OW) => format_value_list(
            values.into_iter().map(|n| format!("{:#x}", n)),
            max_characters,
        )
        .into(),
        (U16(values), _) => format_value_list(values, max_characters).into(),
        (U8(values), VR::OB) => format_value_list(
            values.into_iter().map(|n| format!("{:#x}", n)),
            max_characters,
        )
        .into(),
        (U8(values), _) => format_value_list(values, max_characters).into(),
        (Tags(values), _) => format_value_list(values, max_characters).into(),
        (Strs(values), _) => format_value_list(values, max_characters).into(),
        (Date(values), _) => format_value_list(values, max_characters).into(),
        (Time(values), _) => format_value_list(values, max_characters).into(),
        (DateTime(values), _) => format_value_list(values, max_characters).into(),
        (Str(value), _) => cut_str(&value.to_string(), max_characters)
            .into_owned()
            .into(),
        (Empty, _) => "".into(),
    }
}

fn format_value_list<I>(values: I, max_characters: u32) -> String
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    let pieces = values.into_iter().take(64);
    let max = max_characters as usize;
    let mut o = String::with_capacity(max);
    for piece in pieces {
        o.extend(piece.to_string().chars());
        o.push(',');
        if o.len() > max {
            break;
        }
    }
    o.pop();
    cut_str(&o, max_characters).into_owned()
}

fn cut_str(s: &str, max_characters: u32) -> Cow<str> {
    let max = (max_characters - 3) as usize;
    if s.len() > max {
        format!("{}...", &s[..max]).into()
    } else {
        s.into()
    }
}
