//! A reimplementation of dcmdump in Rust.
//! This is a work in progress, it is not guaranteed to work yet!
extern crate dicom_core;
extern crate dicom_object;

use dicom_core::header::Header;
use dicom_core::dictionary::{DataDictionary, DictionaryEntry};
use dicom_core::value::{Value as DicomValue, PrimitiveValue};
use dicom_core::VR;
use dicom_object::StandardDataDictionary;
use dicom_object::mem::{InMemElement, InMemDicomObject};
use dicom_object::file::open_file;

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
    dump(&mut to, &obj, 0)?;

    Ok(())
}

fn dump<W, D>(to: &mut W, obj: &InMemDicomObject<D>, depth: u32) -> DynResult<()>
where
    W: Write,
    D: DataDictionary,
{
    for elem in obj {
        dump_element(&mut *to, &elem, depth)?;
    }

    Ok(())
}

fn dump_element<W, D>(to: &mut W, elem: &InMemElement<D>, depth: u32) -> DynResult<()>
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
    let len = elem.len();
    let vm = elem.value().multiplicity();


    if let &DicomValue::Sequence { ref items, .. } = elem.value() {
        writeln!(
            to,
            "{} {}                        # {}, {} {}",
            elem.tag(),
            elem.vr(),
            len,
            vm,
            tag_alias
        )?;
        for item in items {
            dump_item(&mut *to, item, depth + 1)?;
        }
    } else {
        let vr = elem.vr();
        let value = elem.value().primitive().unwrap();
        writeln!(
            to,
            "{} {} {:40} # {}, {} {}",
            elem.tag(),
            vr,
            value_summary(&value, vr, 34),
            len,
            vm,
            tag_alias
        )?;
    }

    Ok(())
}

fn dump_item<W, D>(to: &mut W, item: &InMemDicomObject<D>, depth: u32) -> DynResult<()>
where
    W: Write,
    D: DataDictionary,
{
    let indent: String = std::iter::repeat(' ').take((depth * 2) as usize).collect();
    let trail: String = std::iter::repeat(' ').take(usize::max(21, 19 - indent.len())).collect();
    writeln!(to, "{}(FFFE,E000) na Item {} # 0, 0 Item", indent, trail)?;
    dump(to, item, depth + 1)?;
    writeln!(
        to,
        "(FFFE,E00D) na (ItemDelimitationItem)  # 0, 0 ItemDelimitationItem"
    )?;
    Ok(())
}


fn value_summary(value: &PrimitiveValue, _vr: VR, max_characters: u32) -> Cow<str> {
    match value {
        PrimitiveValue::F32(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::F64(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::I32(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::U32(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::I16(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::U16(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::U8(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::Tags(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::Strs(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::Date(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::Time(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::DateTime(values) => format_value_list(values, max_characters).into(),
        PrimitiveValue::Str(value) => cut_str(&value.to_string(), max_characters).into_owned().into(),
        PrimitiveValue::Empty => "".into(),
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
