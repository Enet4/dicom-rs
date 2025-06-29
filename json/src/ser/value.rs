//! DICOM value serialization

use dicom_core::PrimitiveValue;
use serde::ser::SerializeSeq;
use serde::Serialize;

use crate::{INFINITY, NAN, NEG_INFINITY};

/// Wrapper type for [primitive values][1]
/// which should always be encoded as strings.
///
/// Should be used for the value representations
/// AE, AS, AT, CS, DA, DT, LO, LT, SH, ST, TM, UC, UI, UR, and UT.
/// Can also be used for the value representations
/// DS, IS, SV, and UV.
///
/// [1]: dicom_core::PrimitiveValue
#[derive(Debug, Clone)]
pub struct AsStrings<'a>(&'a PrimitiveValue);

impl<'a> From<&'a PrimitiveValue> for AsStrings<'a> {
    fn from(value: &'a PrimitiveValue) -> Self {
        AsStrings(value)
    }
}

impl Serialize for AsStrings<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let strings = self.0.to_multi_str();
        serializer.collect_seq(&*strings)
    }
}

/// Wrapper type for [primitive values][1]
/// which should preferably be encoded as numbers,
/// unless the value is already a string,
/// or if serialization would result in precision loss.
///
/// Should be used for the value representations
/// DS, FL, FD, IS, SL, SS, SV, UL, US, and UV.
///
/// [1]: dicom_core::PrimitiveValue
#[derive(Debug, Clone)]
pub struct AsNumbers<'a>(&'a PrimitiveValue);

impl<'a> From<&'a PrimitiveValue> for AsNumbers<'a> {
    fn from(value: &'a PrimitiveValue) -> Self {
        AsNumbers(value)
    }
}

impl Serialize for AsNumbers<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self.0 {
            // empty
            PrimitiveValue::Empty => serializer.serialize_seq(Some(0))?.end(),
            // not numeric nor stringly
            PrimitiveValue::Date(_) => panic!("wrong impl: cannot encode Date as numbers"),
            PrimitiveValue::DateTime(_) => panic!("wrong impl: cannot encode DateTime as numbers"),
            PrimitiveValue::Time(_) => panic!("wrong impl: cannot encode Time as numbers"),
            PrimitiveValue::Tags(_) => panic!("wrong impl: cannot encode Tags as numbers"),
            // strings
            PrimitiveValue::Strs(strings) => serializer.collect_seq(strings),
            PrimitiveValue::Str(string) => serializer.collect_seq([string]),
            // no risk of precision loss
            PrimitiveValue::U8(numbers) => serializer.collect_seq(numbers),
            PrimitiveValue::I16(numbers) => serializer.collect_seq(numbers),
            PrimitiveValue::U16(numbers) => serializer.collect_seq(numbers),
            PrimitiveValue::I32(numbers) => serializer.collect_seq(numbers),
            PrimitiveValue::U32(numbers) => serializer.collect_seq(numbers),
            // possible precision loss
            PrimitiveValue::I64(numbers) => {
                let mut ser = serializer.serialize_seq(None)?;
                for number in numbers {
                    let narrowed: Option<i32> = num_traits::NumCast::from(*number);
                    if let Some(narrowed) = narrowed {
                        ser.serialize_element(&narrowed)?;
                    } else {
                        ser.serialize_element(&number.to_string())?;
                    }
                }
                ser.end()
            }
            PrimitiveValue::U64(numbers) => {
                let mut ser = serializer.serialize_seq(None)?;
                for number in numbers {
                    let narrowed: Option<i32> = num_traits::NumCast::from(*number);
                    if let Some(narrowed) = narrowed {
                        ser.serialize_element(&narrowed)?;
                    } else {
                        ser.serialize_element(&number.to_string())?;
                    }
                }
                ser.end()
            }
            // floating point
            PrimitiveValue::F32(numbers) => {
                let mut ser = serializer.serialize_seq(None)?;
                for number in numbers {
                    if number.is_finite() {
                        ser.serialize_element(&number)?;
                    } else if number.is_nan() {
                        ser.serialize_element(NAN)?;
                    } else if number.is_infinite() && number.is_sign_positive() {
                        ser.serialize_element(INFINITY)?;
                    } else if number.is_infinite() && number.is_sign_negative() {
                        ser.serialize_element(NEG_INFINITY)?;
                    } else {
                        ser.serialize_element(&Option::<()>::None)?;
                    }
                }
                ser.end()
            }
            PrimitiveValue::F64(numbers) => {
                let mut ser = serializer.serialize_seq(None)?;
                for number in numbers {
                    if number.is_finite() {
                        ser.serialize_element(&number)?;
                    } else if number.is_nan() {
                        ser.serialize_element(NAN)?;
                    } else if number.is_infinite() && number.is_sign_positive() {
                        ser.serialize_element(INFINITY)?;
                    } else if number.is_infinite() && number.is_sign_negative() {
                        ser.serialize_element(NEG_INFINITY)?;
                    } else {
                        ser.serialize_element(&Option::<()>::None)?;
                    }
                }
                ser.end()
            }
        }
    }
}

/// Wrapper type for primitive binary values
/// which should be encoded as base64 inline strings.
///
/// Should be used for the value representations
/// OB, OW, OL, OF, OD, OV, and UN.
#[derive(Debug, Clone)]
pub struct InlineBinary<'a>(&'a PrimitiveValue);

impl<'a> From<&'a PrimitiveValue> for InlineBinary<'a> {
    fn from(value: &'a PrimitiveValue) -> Self {
        InlineBinary(value)
    }
}

impl Serialize for InlineBinary<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = self.0.to_bytes();
        use base64::Engine;
        let str = base64::engine::general_purpose::STANDARD.encode(value);
        serializer.serialize_str(&str)
    }
}

/// Wrapper type for [primitive values][1]
/// which should always be encoded as person names.
///
/// Should only used for the value representation PN.
///
/// [1]: dicom_core::PrimitiveValue
#[derive(Debug, Clone)]
pub struct AsPersonNames<'a>(&'a PrimitiveValue);

impl<'a> From<&'a PrimitiveValue> for AsPersonNames<'a> {
    fn from(value: &'a PrimitiveValue) -> Self {
        AsPersonNames(value)
    }
}

impl Serialize for AsPersonNames<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let strings = self.0.to_multi_str();
        serializer.collect_seq(strings.iter().map(|p| PersonNameDef::from(p.as_str())))
    }
}

/// Wrapper type for a string
/// to be interpreted as a person's name.
///
/// Should only used for the value representation PN.
#[derive(Debug, Clone, Serialize)]
pub struct PersonNameDef<'a> {
    #[serde(rename = "Alphabetic")]
    alphabetic: &'a str,
}

impl<'a> From<&'a str> for PersonNameDef<'a> {
    fn from(value: &'a str) -> Self {
        PersonNameDef { alphabetic: value }
    }
}

#[cfg(test)]
mod tests {
    use dicom_core::dicom_value;
    use dicom_core::value::DicomDate;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use serde_json::Value;

    use super::*;

    #[test]
    fn serialize_primitive_value_as_strings() {
        let v = PrimitiveValue::from("Test Hospital");
        let json = serde_json::to_value(AsStrings(&v)).unwrap();
        assert_eq!(
            json,
            Value::Array(vec![Value::String("Test Hospital".to_string())]),
        );

        let v = PrimitiveValue::Empty;
        let json = serde_json::to_value(AsStrings(&v)).unwrap();
        assert_eq!(json, json!([]));

        let v = dicom_value!(U16, [20, 40, 60]);
        let json = serde_json::to_value(AsStrings(&v)).unwrap();
        assert_eq!(
            json,
            Value::Array(vec![
                Value::from("20"),
                Value::from("40"),
                Value::from("60"),
            ]),
        );

        let v = dicom_value!(Date, [DicomDate::from_ymd(2023, 6, 13).unwrap()]);
        let json = serde_json::to_value(AsStrings(&v)).unwrap();
        assert_eq!(json, Value::Array(vec![Value::from("20230613")]));
    }

    #[test]
    fn serialize_primitive_value_as_numbers() {
        let v = PrimitiveValue::from(23.5_f64);
        let json = serde_json::to_value(AsNumbers(&v)).unwrap();
        assert_eq!(json, json!([23.5]),);

        let v = PrimitiveValue::from([f64::NAN, f64::INFINITY, f64::NEG_INFINITY]);
        let json = serde_json::to_value(AsNumbers(&v)).unwrap();
        assert_eq!(json, json!(["NaN", "inf", "-inf"]),);

        let v = PrimitiveValue::from([f32::NAN, f32::INFINITY, f32::NEG_INFINITY]);
        let json = serde_json::to_value(AsNumbers(&v)).unwrap();
        assert_eq!(json, json!(["NaN", "inf", "-inf"]),);

        let v = PrimitiveValue::Empty;
        let json = serde_json::to_value(AsNumbers(&v)).unwrap();
        assert_eq!(json, json!([]));

        let v = PrimitiveValue::from("5");
        let json = serde_json::to_value(AsNumbers(&v)).unwrap();
        assert_eq!(json, json!(["5"]),);

        let v = dicom_value!(U16, [20, 40, 60]);
        let json = serde_json::to_value(AsNumbers(&v)).unwrap();
        assert_eq!(json, json!([20, 40, 60]));

        // too large for a 32-bit integer
        let v = dicom_value!(U64, [876543245678]);
        let json = serde_json::to_value(AsNumbers(&v)).unwrap();
        assert_eq!(json, json!(["876543245678"]),);
    }
}
