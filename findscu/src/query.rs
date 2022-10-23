//! Module for parsing query text pieces into DICOM queries.

use std::str::FromStr;

use dicom_core::header::HasLength;
use dicom_core::DataDictionary;
use dicom_core::DataElement;
use dicom_core::PrimitiveValue;
use dicom_core::Tag;
use dicom_core::VR;
use dicom_dictionary_std::StandardDataDictionary;
use dicom_object::InMemDicomObject;
use snafu::whatever;
use snafu::{OptionExt, ResultExt, Whatever};

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct TermQuery {
    field: Tag,
    match_value: String,
}

/// Term queries can be parsed with the syntax `«tag»=«value»`,
/// where `«tag»` is either a DICOM tag group-element pair
/// or the respective tag keyword, 
/// and `=«value»` is optional.
impl FromStr for TermQuery {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('=');

        let tag_part = parts.next().whatever_context("empty query")?;
        let value_part = parts.next().unwrap_or_default();

        let field: Tag = tag_part.parse().or_else(|_| {
            // look for tag in standard data dictionary
            let data_entry = StandardDataDictionary
                .by_name(tag_part)
                .whatever_context("could not resolve query field name")?;
            Ok(data_entry.tag.inner())
        })?;

        Ok(TermQuery {
            field,
            match_value: value_part.to_owned(),
        })
    }
}

pub fn parse_queries<T>(qs: &[T]) -> Result<InMemDicomObject, Whatever>
where
    T: AsRef<str>,
{
    let mut obj = InMemDicomObject::new_empty();

    for q in qs {
        let term_query: TermQuery = q.as_ref().parse()?;
        obj.put(term_to_element(term_query.field, &term_query.match_value)?);
    }
    Ok(obj)
}

fn term_to_element<I, O>(tag: Tag, txt_value: &str) -> Result<DataElement<I, O>, Whatever>
where
    I: HasLength,
{
    let vr = {
        StandardDataDictionary
            .by_tag(tag)
            .map(|e| e.vr)
            .unwrap_or(VR::LO)
    };
    let value = match vr {
        VR::AE
        | VR::AS
        | VR::CS
        | VR::DA
        | VR::DS
        | VR::IS
        | VR::LO
        | VR::LT
        | VR::SH
        | VR::PN
        | VR::ST
        | VR::TM
        | VR::UI
        | VR::UC
        | VR::UR
        | VR::UT
        | VR::DT => PrimitiveValue::from(txt_value),
        VR::AT => whatever!("Unsupported VR AT"),
        VR::OB => whatever!("Unsupported VR OB"),
        VR::OD => whatever!("Unsupported VR OD"),
        VR::OF => whatever!("Unsupported VR OF"),
        VR::OL => whatever!("Unsupported VR OL"),
        VR::OV => whatever!("Unsupported VR OV"),
        VR::OW => whatever!("Unsupported VR OW"),
        VR::UN => whatever!("Unsupported VR UN"),
        VR::SQ => whatever!("Unsuppoted sequence-based query"),
        VR::SS => {
            let ss: i16 = txt_value
                .parse()
                .whatever_context("Failed to parse value as SS")?;
            PrimitiveValue::from(ss)
        }
        VR::SL => {
            let sl: i32 = txt_value
                .parse()
                .whatever_context("Failed to parse value as SL")?;
            PrimitiveValue::from(sl)
        }
        VR::SV => {
            let sv: i64 = txt_value
                .parse()
                .whatever_context("Failed to parse value as SV")?;
            PrimitiveValue::from(sv)
        }
        VR::US => {
            let us: u16 = txt_value
                .parse()
                .whatever_context("Failed to parse value as US")?;
            PrimitiveValue::from(us)
        }
        VR::UL => {
            let ul: u32 = txt_value
                .parse()
                .whatever_context("Failed to parse value as UL")?;
            PrimitiveValue::from(ul)
        }
        VR::UV => {
            let uv: u64 = txt_value
                .parse()
                .whatever_context("Failed to parse value as UV")?;
            PrimitiveValue::from(uv)
        }
        VR::FL => {
            let fl: f32 = txt_value
                .parse()
                .whatever_context("Failed to parse value as FL")?;
            PrimitiveValue::from(fl)
        }
        VR::FD => {
            let fd: f64 = txt_value
                .parse()
                .whatever_context("Failed to parse value as FD")?;
            PrimitiveValue::from(fd)
        }
    };
    Ok(DataElement::new(tag, vr, value))
}
