//! A reimplementation of dcmdump in Rust.
//! This is a work in progress, it is not guaranteed to work yet!
extern crate dicom_core;
extern crate dicom_object;

use dicom_core::header::Header;
use dicom_core::dictionary::{DataDictionary, DictionaryEntry};
use dicom_core::value::Value as DicomValue;
use dicom_object::StandardDataDictionary;
use dicom_object::mem::{InMemElement, InMemDicomObject};
use dicom_object::file::open_file;

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
    writeln!(
        to,
        "{} {}                    # {}, {} {}",
        elem.tag(),
        elem.vr(),
        len,
        vm,
        tag_alias
    )?;

    if let &DicomValue::Sequence { ref items, .. } = elem.value() {
        for item in items {
            dump_item(&mut *to, item, depth + 1)?;
        }
    }

    Ok(())
}

fn dump_item<W, D>(to: &mut W, item: &InMemDicomObject<D>, depth: u32) -> DynResult<()>
where
    W: Write,
    D: DataDictionary,
{
    let indent = vec![b' '; (depth * 2) as usize];
    to.write(&indent)?;
    writeln!(to, "(FFFE,E000) na Item                    # 0, 0 Item")?;
    dump(to, item, depth + 1)?;
    writeln!(
        to,
        "(FFFE,E00D) na (ItemDelimitationItem)  # 0, 0 ItemDelimitationItem"
    )?;
    Ok(())
}
