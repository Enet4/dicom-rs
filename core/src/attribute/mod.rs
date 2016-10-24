//! This module contains basic data structures and traits for DICOM attributes.
//! It is also the home for the concept of data dictionary, which provides
//! a mapping from tags or aliases to their respective attributes.

pub mod tag;
pub mod dictionary;
pub mod value;

use std::str::from_utf8;
use std::fmt;

/// An enum type for a DICOM value representation.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueRepresentation {
    /// Application Entity
    AE,
    /// Age String
    AS,
    /// Attribute Tag
    AT,
    /// Code String
    CS,
    /// Date
    DA,
    /// Decimal String
    DS,
    /// Date Time
    DT,
    /// Floating Point Single
    FL,
    /// Floating Point Double
    FD,
    /// Integer String
    IS,
    /// Long String
    LO,
    /// Long Text
    LT,
    /// Other Byte
    OB,
    /// Other Double
    OD,
    /// Other Float
    OF,
    /// Other Long
    OL,
    /// Other Word
    OW,
    /// Person Name
    PN,
    /// Short String
    SH,
    /// Signed Long
    SL,
    /// Sequence of Items
    SQ,
    /// Signed Short
    SS,
    /// Short Text
    ST,
    /// Time
    TM,
    /// Unlimited Characters
    UC,
    /// Unique Identifier (UID)
    UI,
    /// Unsigned Long
    UL,
    /// Unknown
    UN,
    /// Universal Resource Identifier or Universal Resource Locator (URI/URL)
    UR,
    /// Unsigned Short
    US,
    /// Unlimited Text
    UT,
}

impl ValueRepresentation {
    /// Obtain the value representation corresponding to the given two bytes.
    /// Each byte should represent an alphabetic character in upper case.
    pub fn from_binary(chars: [u8; 2]) -> Option<ValueRepresentation> {
        from_utf8(chars.as_ref()).ok().and_then(|s| ValueRepresentation::from_str(s))
    }

    /// Obtain the value representation corresponding to the given string.
    /// The string should hold exactly two UTF-8 encoded alphabetic characters
    /// in upper case, otherwise no match is made.
    pub fn from_str(string: &str) -> Option<ValueRepresentation> {
        match string {
            "AE" => Some(ValueRepresentation::AE),
            "AS" => Some(ValueRepresentation::AS),
            "AT" => Some(ValueRepresentation::AT),
            "CS" => Some(ValueRepresentation::CS),
            "DA" => Some(ValueRepresentation::DA),
            "DS" => Some(ValueRepresentation::DS),
            "DT" => Some(ValueRepresentation::DT),
            "FL" => Some(ValueRepresentation::FL),
            "FD" => Some(ValueRepresentation::FD),
            "IS" => Some(ValueRepresentation::IS),
            "LO" => Some(ValueRepresentation::LO),
            "LT" => Some(ValueRepresentation::LT),
            "OB" => Some(ValueRepresentation::OB),
            "OD" => Some(ValueRepresentation::OD),
            "OF" => Some(ValueRepresentation::OF),
            "OL" => Some(ValueRepresentation::OL),
            "OW" => Some(ValueRepresentation::OW),
            "PN" => Some(ValueRepresentation::PN),
            "SH" => Some(ValueRepresentation::SH),
            "SL" => Some(ValueRepresentation::SL),
            "SQ" => Some(ValueRepresentation::SQ),
            "SS" => Some(ValueRepresentation::SS),
            "ST" => Some(ValueRepresentation::ST),
            "TM" => Some(ValueRepresentation::TM),
            "UC" => Some(ValueRepresentation::UC),
            "UI" => Some(ValueRepresentation::UI),
            "UL" => Some(ValueRepresentation::UL),
            "UN" => Some(ValueRepresentation::UN),
            "UR" => Some(ValueRepresentation::UR),
            "US" => Some(ValueRepresentation::US),
            "UT" => Some(ValueRepresentation::UT),
            _ => None,
        }
    }

    /// Retrieve a string representation of this VR.
    pub fn to_string(&self) -> &'static str {
        match *self {
            ValueRepresentation::AE => "AE",
            ValueRepresentation::AS => "AS",
            ValueRepresentation::AT => "AT",
            ValueRepresentation::CS => "CS",
            ValueRepresentation::DA => "DA",
            ValueRepresentation::DS => "DS",
            ValueRepresentation::DT => "DT",
            ValueRepresentation::FL => "FL",
            ValueRepresentation::FD => "FD",
            ValueRepresentation::IS => "IS",
            ValueRepresentation::LO => "LO",
            ValueRepresentation::LT => "LT",
            ValueRepresentation::OB => "OB",
            ValueRepresentation::OD => "OD",
            ValueRepresentation::OF => "OF",
            ValueRepresentation::OL => "OL",
            ValueRepresentation::OW => "OW",
            ValueRepresentation::PN => "PN",
            ValueRepresentation::SH => "SH",
            ValueRepresentation::SL => "SL",
            ValueRepresentation::SQ => "SQ",
            ValueRepresentation::SS => "SS",
            ValueRepresentation::ST => "ST",
            ValueRepresentation::TM => "TM",
            ValueRepresentation::UC => "UC",
            ValueRepresentation::UI => "UI",
            ValueRepresentation::UL => "UL",
            ValueRepresentation::UN => "UN",
            ValueRepresentation::UR => "UR",
            ValueRepresentation::US => "US",
            ValueRepresentation::UT => "UT",
        }
    }

    /// Retrieve a copy of this VR's byte representation.
    /// The function returns two alphabetic characters in upper case.
    pub fn to_bytes(&self) -> [u8;2] {
        let bytes = self.to_string().as_bytes();
        [bytes[0], bytes[1]]
    }
}

impl fmt::Display for ValueRepresentation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.to_string())
    }
}
