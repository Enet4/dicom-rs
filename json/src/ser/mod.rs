//! DICOM JSON serialization module

use std::marker::PhantomData;

use dicom_core::{DicomValue, PrimitiveValue, VR, header::Header};
use dicom_object::{InMemDicomObject, mem::InMemElement};
use serde::{Serializer, Serialize, ser::SerializeMap};

use self::value::{AsPersonNames, AsStrings, AsNumbers, InlineBinary};
mod value;


pub fn serialize_to_string(data: &InMemDicomObject) -> Result<String, serde_json::Error> {
    serde_json::to_string(&DatasetDef::from(data))
}

pub fn serialize_to_value(data: &InMemDicomObject) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(&DatasetDef::from(data))
}

#[derive(Debug, Clone)]
pub struct DatasetDef<'a, I>(I, PhantomData<&'a I>);

impl<'a, D> From<&'a InMemDicomObject<D>> for DatasetDef<'a, &'a InMemDicomObject<D>> {
    fn from(value: &'a InMemDicomObject<D>) -> Self {
        DatasetDef(value, PhantomData)
    }
}

impl<'a, I, D> Serialize for DatasetDef<'a, I>
where
    D: 'a,
    I: Copy,
    I: IntoIterator<Item = &'a InMemElement<D>>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_map(self.0.into_iter().map(|e| {
            let tag = e.tag();
            let tag = format!("{:04X}{:04X}", tag.0, tag.1);
            (tag, DataElementDef(e))
        }))
    }
}

#[derive(Debug, Clone)]
pub struct ItemsDef<'a, D>(&'a [InMemDicomObject<D>]);

impl<'a, D> From<&'a [InMemDicomObject<D>]> for ItemsDef<'a, D> {
    fn from(value: &'a [InMemDicomObject<D>]) -> Self {
        ItemsDef(value)
    }
}

impl<D> Serialize for ItemsDef<'_, D> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(self.0.into_iter().map(DatasetDef::from))
    }
}

#[derive(Debug, Clone)]
pub struct DataElementDef<'a, D>(&'a InMemElement<D>);

impl<'a, D> From<&'a InMemElement<D>> for DataElementDef<'a, D> {
    fn from(value: &'a InMemElement<D>) -> Self {
        DataElementDef(value)
    }
}

impl<D> Serialize for DataElementDef<'_, D> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serializer = serializer.serialize_map(None)?;
        let vr = self.0.vr();
        serializer.serialize_entry("vr", vr.to_string())?;

        match self.0.value() {
            DicomValue::Sequence(seq) => {
                serializer.serialize_entry("Value", &ItemsDef(seq.items()))?;
            }
            DicomValue::PixelSequence(_seq) => {
                // TODO encode basic offset table and fragments
                todo!("encapsulated pixel data")
            }
            DicomValue::Primitive(PrimitiveValue::Empty) => {
                // no-op
            }
            DicomValue::Primitive(v) => match vr {
                VR::AE
                | VR::AS
                | VR::AT
                | VR::CS
                | VR::DA
                | VR::DT
                | VR::LO
                | VR::LT
                | VR::SH
                | VR::UC
                | VR::UI
                | VR::UR
                | VR::TM
                | VR::ST
                | VR::UT => {
                    serializer.serialize_entry("Value", &AsStrings::from(v))?;
                }
                VR::PN => {
                    serializer.serialize_entry("Value", &AsPersonNames::from(v))?;
                }
                VR::FD
                | VR::IS
                | VR::FL
                | VR::DS
                | VR::SL
                | VR::SS
                | VR::SV
                | VR::UL
                | VR::US
                | VR::UV => {
                    serializer.serialize_entry("Value", &AsNumbers::from(v))?;
                }
                VR::OB | VR::OD | VR::OF | VR::OL | VR::OV | VR::OW | VR::UN => {
                    serializer.serialize_entry("InlineBinary", &InlineBinary::from(v))?;
                }
                VR::SQ => unreachable!("unexpected VR SQ in primitive value"),
            },
        }

        serializer.end()
    }
}

#[cfg(test)]
mod tests {

    use dicom_core::{smallvec, VR};
    use dicom_core::{dicom_value, value::DicomDate, Tag};
    use serde_json::json;

    use super::*;

    #[test]
    fn serialize_simple_data_elements() {
        let all_data = vec![
            InMemElement::new(
                Tag(0x0008, 0x0005),
                VR::CS,
                PrimitiveValue::from("ISO_IR 192"),
            ),
            InMemElement::new(
                Tag(0x0008, 0x0020),
                VR::DA,
                PrimitiveValue::from(DicomDate::from_ymd(2013, 4, 9).unwrap()),
            ),
            InMemElement::new(
                Tag(0x0008, 0x0061),
                VR::CS,
                dicom_value!(Strs, ["CT", "PET"]),
            ),
            InMemElement::new(
                Tag(0x0008, 0x0090),
                VR::PN,
                PrimitiveValue::from("^Bob^^Dr."),
            ),
            InMemElement::new(
                Tag(0x0009, 0x1002),
                VR::UN,
                dicom_value!(U8, [0xcf, 0x4c, 0x7d, 0x73, 0xcb, 0xfb]),
            ),
            InMemElement::new(
                dicom_dictionary_std::tags::PATIENT_AGE,
                VR::AS,
                PrimitiveValue::from("30Y"),
            ),
        ];

        let obj = InMemDicomObject::from_element_iter(all_data);

        assert_eq!(
            serialize_to_value(&obj).unwrap(),
            json!({
                "00080005": {
                    "vr": "CS",
                    "Value": [ "ISO_IR 192" ]
                },
                "00080020": {
                    "vr": "DA",
                    "Value": [ "20130409" ]
                },
                "00080061": {
                    "vr": "CS",
                    "Value": [
                        "CT",
                        "PET"
                    ]
                },
                "00080090": {
                    "vr": "PN",
                    "Value": [
                      {
                        "Alphabetic": "^Bob^^Dr."
                      }
                    ]
                },
                "00091002": {
                    "vr": "UN",
                    "InlineBinary": "z0x9c8v7"
                },
                "00101010": {
                    "vr": "AS",
                    "Value": [ "30Y" ]
                },
            }),
        );
    }
}
