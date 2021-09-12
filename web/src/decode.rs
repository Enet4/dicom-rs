use crate::DicomResponse;
use dicom::core::chrono::FixedOffset;
// use dicom_dump::dump::dump_element;
use serde_json::Value;
use dicom::object::mem::InMemDicomObject;
use dicom::core::{DataElement, DicomValue, Length, Tag, VR};
use dicom::core::value::deserialize::{parse_date, parse_time, parse_datetime};
use dicom::core::dicom_value;
use std::i64;

pub fn decode_response_item(item: &Value) -> DicomResponse {
    let mut obj = InMemDicomObject::create_empty();
    match item {
        Value::Object(item) => {
            item.into_iter().for_each(|(k, v)| {
                let a = i64::from_str_radix(&k[..4], 16).unwrap() as u16;
                let b = i64::from_str_radix(&k[4..], 16).unwrap() as u16;
                let tag: Tag = (a, b).into();
                if let Value::String(raw_vr) = &v["vr"] {
                    let vr = raw_vr.parse::<VR>().unwrap();
                    let value = &v["Value"];
                    match vr {
                        VR::AE => {
                            todo!()
                        }
                        // VR::AS => {
                        //     /* todo!("VR::AS") */
                        //     eprintln!("TODO: VR::AS");
                        // }
                        VR::AT => {
                            todo!()
                        }
                        VR::AS | VR::CS | VR::LO | VR::SH | VR::UI | VR::UR | VR::OW | VR::ST | VR::UN | VR::LT | VR::OB => match value {
                            Value::Array(_) => {
                                let v: Vec<String> =
                                    serde_json::from_value(value.to_owned()).unwrap();
                                let vv = &v[0];
                                let elt = DataElement::new(tag, vr, dicom_value!(Str, vv));
                                obj.put(elt);
                            }
                            Value::Null => {
                                let elt = DataElement::new(tag, vr, dicom_value!());
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("{:?} unexpected value: {:?}", vr, other);
                            }
                        },
                        VR::DA => match value {
                            Value::Array(_) => {
                                let v: Vec<String> =
                                    serde_json::from_value(value.to_owned()).unwrap();
                                let vv = &v[0];
                                let (date, _bytes) = parse_date(vv.as_bytes()).unwrap();
                                let elt = DataElement::new(tag, vr, dicom_value!(date));
                                obj.put(elt);
                            }
                            Value::Null => {
                                let elt = DataElement::new(tag, vr, dicom_value!());
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("{:?} unexpected value: {:?}", vr, other);
                            }
                        },
                        VR::DS => match value {
                            Value::Array(_) => {
                                let v: Vec<f64> =
                                    serde_json::from_value(value.to_owned()).unwrap();
                                let vv = v[0];
                                let elt = DataElement::new(tag, vr, dicom_value!(F64, vv));
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("{:?} unexpected value: {:?}", vr, other);
                            }
                        }
                        VR::DT =>  match value {
                            Value::Array(_) => {
                                let v: Vec<String> =
                                    serde_json::from_value(value.to_owned()).unwrap();
                                let vv = &v[0];
                                let default_offset = FixedOffset::east(0);
                                eprintln!("VR:DT: {:?}", v);
                                let datetime = parse_datetime(vv.as_bytes(), default_offset).unwrap();
                                let elt = DataElement::new(tag, vr, dicom_value!(datetime));
                                obj.put(elt);
                            }
                            Value::Null => {
                                let elt = DataElement::new(tag, vr, dicom_value!());
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("{:?} unexpected value: {:?}", vr, other);
                            }
                        }
                        VR::FL => match value {
                            Value::Array(_) => {
                                let v: Vec<f32> =
                                    serde_json::from_value(value.to_owned()).unwrap();
                                let vv = v[0];
                                let elt = DataElement::new(tag, vr, dicom_value!(F32, vv));
                                obj.put(elt);
                            }
                            Value::Null => {
                                let elt = DataElement::new(tag, vr, dicom_value!());
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("{:?} unexpected value: {:?}", vr, other);
                            }
                            /* todo!(VR::DS) */
                        }
                        VR::FD => {
                            /* todo!("VR::FD") */
                            eprintln!("TODO: VR::FD");
                        }
                        VR::IS => match value {
                            Value::Array(_) => {
                                let v: Vec<i16> = serde_json::from_value(value.to_owned()).unwrap();
                                let vv = &v[0];
                                let elt =
                                    DataElement::new(tag, vr, dicom_value!(Str, vv.to_string()));
                                obj.put(elt);
                            }
                            Value::Null => {
                                let elt = DataElement::new(tag, vr, dicom_value!());
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("{:?} unexpected value: {:?}", vr, other);
                            }
                        },
                        VR::OD => {
                            todo!()
                        }
                        VR::OF => {
                            todo!()
                        }
                        VR::OL => {
                            todo!()
                        }
                        VR::OV => {
                            todo!()
                        }
                        VR::PN => match value {
                            Value::Array(array) => {
                                let v = &array[0];
                                let name = match v {
                                    Value::Object(hm) => {
                                        if let Some(name) = hm.get("Alphabetic").and_then(|v| v.as_str()) {
                                           name 
                                        }
                                        else {
                                            ""
                                        }
                                    },
                                    _other => { "" }
                                };
                                let elt = DataElement::new(tag, vr, dicom_value!(Str, name));
                                obj.put(elt);

                            }
                            Value::Null => {
                                let elt = DataElement::new(tag, vr, dicom_value!());
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("VR:PN unexpected value: {:?}", other);
                            }
                        }
                        VR::SL => {
                            eprintln!("TODO: VR::SL");
                        }
                        VR::SQ => match value {
                            Value::Array(array) => {
                                // eprintln!("TODO: VR::SQ: {:?}", array);
                                let value = DicomValue::new_sequence(
                                    array.iter().map(|v| {
                                        decode_response_item(v)
                                    }).collect::<Vec<_>>(),
                                    Length::UNDEFINED
                                );
                                // let value: Vec<_> = array.iter().map(|v| {
                                //     decode_response_item(v)
                                // }).collect();
                                let elt = DataElement::new(
                                    tag,
                                    VR::SQ,
                                    value);
                              //  eprintln!("Check: SEQ: {:#?}", elt);

                                // FIXME: don't work check dcmdump side
                                //    note: expected struct `dicom::dicom_core::DataElement<_, std::vec::Vec<u8>>`
                                //    found struct `dicom::dicom_core::DataElement<_, [u8; 0]>`
                                // obj.put(elt);
                                // let _ = dump_element(&mut std::io::stderr(), &elt, 120, 0, true);

                            }
                            Value::Null => {
                                let elt = DataElement::new(
                                    tag,
                                    VR::SQ,
                                    dicom_value!());
                                obj.put(elt);

                            }
                            other => {
                                eprintln!("VR::SEQ unexpected value: {:?}", other);
                            }
                        }
                        VR::SS => {
                            todo!()
                        }
                        VR::SV => {
                            todo!()
                        }
                        VR::TM => {
                            /* todo!() */
                            match value {
                                Value::Array(_) => {
                                    let v: Vec<String> =
                                        serde_json::from_value(value.to_owned()).unwrap();
                                    let vv = &v[0];
                                    let (time, _bytes) = parse_time(vv.as_bytes()).unwrap();
                                    let elt = DataElement::new(tag, vr, dicom_value!(time));
                                    obj.put(elt);
                                }
                                Value::Null => {
                                    let elt = DataElement::new(tag, vr, dicom_value!());
                                    obj.put(elt);
                                }
                                other => {
                                    eprintln!("{:?} unexpected value: {:?}", vr, other);
                                }
                            }
                        }
                        VR::UC => {
                            todo!()
                        }
                        VR::UL => {
                            todo!()
                        }
                        VR::US => match value {
                            Value::Array(_) => {
                                let v: Vec<u16> = serde_json::from_value(value.to_owned()).unwrap();
                                let vv = v[0];
                                let elt = DataElement::new(tag, vr, dicom_value!(vv));
                                obj.put(elt);
                            }
                            Value::Null => {
                                let elt = DataElement::new(tag, vr, dicom_value!());
                                obj.put(elt);
                            }
                            other => {
                                eprintln!("{:?} unexpected value: {:?}", vr, other);
                            }
                        },
                        VR::UT => {
                            todo!()
                        }
                        VR::UV => {
                            todo!()
                        }
                    }
                } else {
                    eprintln!("error, invalid VR: {:?}", v["vr"]);
                }
            })
        }
        other => {
            println!("Unexpected: {:?}", other);
        }
    }
    obj
}
