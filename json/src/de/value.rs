//! DICOM value deserialization
use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DicomJsonPerson {
    #[serde(rename = "Alphabetic")]
    alphabetic: String,
    #[serde(rename = "Ideographic")]
    ideographic: Option<String>,
    #[serde(rename = "Phonetic")]
    phonetic: Option<String>,
}

impl fmt::Display for DicomJsonPerson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomJsonPerson {
                alphabetic,
                ideographic: None,
                phonetic: None,
            } => write!(f, "{alphabetic}"),
            DicomJsonPerson {
                alphabetic,
                ideographic: Some(ideographic),
                phonetic: None,
            } => write!(f, "{alphabetic}={ideographic}"),
            DicomJsonPerson {
                alphabetic,
                ideographic: None,
                phonetic: Some(phonetic),
            } => write!(f, "{alphabetic}=={phonetic}"),
            DicomJsonPerson {
                alphabetic,
                ideographic: Some(ideographic),
                phonetic: Some(phonetic),
            } => write!(f, "{alphabetic}={ideographic}={phonetic}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BulkDataUri(String);

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum NumberOrText<N> {
    Number(N),
    Text(String),
}

impl<N> NumberOrText<N>
where
    N: Clone,
    N: FromStr,
{
    pub fn to_num(&self) -> Result<N, <N as FromStr>::Err> {
        match self {
            NumberOrText::Number(num) => Ok(num.clone()),
            NumberOrText::Text(text) => text.parse(),
        }
    }
}

impl<N> std::fmt::Display for NumberOrText<N>
where
    N: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumberOrText::Number(number) => std::fmt::Display::fmt(number, f),
            NumberOrText::Text(text) => f.write_str(text),
        }
    }
}
