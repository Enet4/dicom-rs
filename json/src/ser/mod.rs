//! DICOM JSON serialization module
#![warn(missing_docs)]

use dicom_core::{header::Header, DicomValue, PrimitiveValue, VR};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_object::{mem::InMemElement, DefaultDicomObject, InMemDicomObject};
use serde::{ser::SerializeMap, Serialize, Serializer};

use self::value::{AsNumbers, AsPersonNames, AsStrings, InlineBinary};
mod value;

/// A wrapper type for DICOM JSON serialization using [Serde](serde).
///
/// Serializing this type will yield JSON data according to the standard.
/// 
/// # Example
/// 
/// Convert a reference to a DICOM object or element using [`From`] or [`Into`],
/// then use [`serde_json`] to serialize it to the intended type.
/// 
/// ```
/// # use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
/// # use dicom_object::InMemDicomObject;
/// use dicom_json::DicomJson;
///
/// // creating a DICOM object with a single attribute
/// let obj = InMemDicomObject::from_element_iter([
///     DataElement::new(
///         Tag(0x0010, 0x0020),
///         VR::LO,
///         PrimitiveValue::from("ID0001"),
///     )
/// ]);
/// // wrap it with DicomJson
/// let json_obj = DicomJson::from(&obj);
/// // serializing it to a JSON Value
/// let serialized = serde_json::to_value(&json_obj)?;
/// assert_eq!(
///   serialized,
///   serde_json::json!({
///       "00100020": {
///           "vr": "LO",
///           "Value": [ "ID0001" ]
///       }
///   })
/// );
/// # Result::<_, serde_json::Error>::Ok(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DicomJson<T>(T);

/// Serialize a piece of DICOM data to a JSON string
pub fn to_string<T>(data: T) -> Result<String, serde_json::Error>
where
    DicomJson<T>: From<T> + Serialize,
{
    serde_json::to_string(&DicomJson::from(data))
}

/// Serialize a piece of DICOM data to a serde JSON value
pub fn to_value<T>(data: T) -> Result<serde_json::Value, serde_json::Error>
where
    DicomJson<T>: From<T> + Serialize,
{
    serde_json::to_value(&DicomJson::from(data))
}

impl<'a, D> From<&'a DefaultDicomObject<D>> for DicomJson<&'a DefaultDicomObject<D>> {
    fn from(value: &'a DefaultDicomObject<D>) -> Self {
        Self(value)
    }
}

impl<'a, D> Serialize for DicomJson<&'a DefaultDicomObject<D>>
where
    D: 'a,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = serializer.serialize_map(None)?;

        for e in self.0.meta().to_element_iter() {
            let tag = e.tag();
            let tag = format!("{:04X}{:04X}", tag.0, tag.1);
            let DicomValue::Primitive(value) = e.value() else {
                continue;
            };
            let e = InMemElement::<StandardDataDictionary>::new(e.tag(), e.vr(), value.clone());
            ser.serialize_entry(&tag, &DicomJson(&e))?;
        }

        let inner: &InMemDicomObject<_> = &**self.0;
        for e in inner {
            let tag = e.tag();
            let tag = format!("{:04X}{:04X}", tag.0, tag.1);
            ser.serialize_entry(&tag, &DicomJson(e))?;
        }

        ser.end()
    }
}

impl<'a, D> From<&'a InMemDicomObject<D>> for DicomJson<&'a InMemDicomObject<D>> {
    fn from(value: &'a InMemDicomObject<D>) -> Self {
        Self(value)
    }
}

impl<'a, D> Serialize for DicomJson<&'a InMemDicomObject<D>>
where
    D: 'a,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_map(self.0.into_iter().map(|e| {
            let tag = e.tag();
            let tag = format!("{:04X}{:04X}", tag.0, tag.1);
            (tag, DicomJson(e))
        }))
    }
}

impl<'a, D> From<&'a [InMemDicomObject<D>]> for DicomJson<&'a [InMemDicomObject<D>]> {
    fn from(value: &'a [InMemDicomObject<D>]) -> Self {
        Self(value)
    }
}

impl<'a, D> Serialize for DicomJson<&'a [InMemDicomObject<D>]> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(self.0.into_iter().map(DicomJson::from))
    }
}

impl<'a, D> From<&'a InMemElement<D>> for DicomJson<&'a InMemElement<D>> {
    fn from(value: &'a InMemElement<D>) -> Self {
        Self(value)
    }
}

impl<D> Serialize for DicomJson<&'_ InMemElement<D>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serializer = serializer.serialize_map(None)?;
        let vr = self.0.vr();
        serializer.serialize_entry("vr", vr.to_string())?;

        match self.0.value() {
            DicomValue::Sequence(seq) => {
                serializer.serialize_entry("Value", &DicomJson(seq.items()))?;
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

    use dicom_core::VR;
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
            to_value(&obj).unwrap(),
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

    #[test]
    fn write_full_file_to_json() {
        let sc_rgb_rle = dicom_test_files::path("pydicom/SC_rgb_rle.dcm").unwrap();

        let obj = dicom_object::OpenFileOptions::new()
            .read_until(Tag(0x0010, 0))
            .open_file(sc_rgb_rle)
            .expect("Failed to open test file");

        let value = to_value(&obj).unwrap();

        assert_eq!(
            value,
            json!({
                "00020000": {
                    "vr": "UL",
                    "Value": [238]
                },
                "00020001": {
                    "vr": "OB",
                    "InlineBinary": "AAE="
                },
                "00020002": {
                    "vr": "UI",
                    "Value": ["1.2.840.10008.5.1.4.1.1.7"]
                },
                "00020003": {
                    "vr": "UI",
                    "Value": ["1.2.826.0.1.3680043.8.498.49043964482360854182530167603505525116"]
                },
                "00020010": {
                    "vr": "UI",
                    "Value": ["1.2.840.10008.1.2.5"]
                },
                "00020012": {
                    "vr": "UI",
                    "Value": ["1.2.826.0.1.3680043.2.1143.107.104.103.115.2.8.4"]
                },
                "00020013": {
                    "vr": "SH",
                    "Value": ["GDCM 2.8.4"]
                },
                "00020016": {
                    "vr": "AE",
                    "Value": ["gdcmconv"]
                },
                "00080005": {
                    "vr": "CS",
                    "Value": ["ISO_IR 192"]
                },
                "00080008": {
                    "vr": "CS",
                    "Value": ["DERIVED", "SECONDARY", "OTHER"]
                },
                "00080016": {
                    "vr": "UI",
                    "Value": ["1.2.840.10008.5.1.4.1.1.7"]
                },
                "00080018": {
                    "vr": "UI",
                    "Value": ["1.2.826.0.1.3680043.8.498.49043964482360854182530167603505525116"]
                },
                "00080020": {
                    "vr": "DA",
                    "Value": ["20170101"]
                },
                "00080023": { "vr": "DA" },
                "0008002A": { "vr": "DT" },
                "00080030": {
                    "vr": "TM",
                    "Value": ["120000"],
                },
                "00080033": { "vr": "TM" },
                "00080050": { "vr": "SH" },
                "00080060": {
                    "vr": "CS",
                    "Value": ["OT"]
                },
                "00080064": {
                    "vr": "CS",
                    "Value": ["SYN"]
                },
                "00080090": {
                    "vr": "PN",
                    "Value": [
                        {
                            "Alphabetic": "Moriarty^James"
                        }
                    ]
                }
            })
        );
    }
}
