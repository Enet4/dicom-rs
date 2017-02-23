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
pub enum VR {
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

impl VR {
    /// Obtain the value representation corresponding to the given two bytes.
    /// Each byte should represent an alphabetic character in upper case.
    pub fn from_binary(chars: [u8; 2]) -> Option<VR> {
        from_utf8(chars.as_ref()).ok().and_then(|s| VR::from_str(s))
    }

    /// Obtain the value representation corresponding to the given string.
    /// The string should hold exactly two UTF-8 encoded alphabetic characters
    /// in upper case, otherwise no match is made.
    pub fn from_str(string: &str) -> Option<VR> {
        match string {
            "AE" => Some(VR::AE),
            "AS" => Some(VR::AS),
            "AT" => Some(VR::AT),
            "CS" => Some(VR::CS),
            "DA" => Some(VR::DA),
            "DS" => Some(VR::DS),
            "DT" => Some(VR::DT),
            "FL" => Some(VR::FL),
            "FD" => Some(VR::FD),
            "IS" => Some(VR::IS),
            "LO" => Some(VR::LO),
            "LT" => Some(VR::LT),
            "OB" => Some(VR::OB),
            "OD" => Some(VR::OD),
            "OF" => Some(VR::OF),
            "OL" => Some(VR::OL),
            "OW" => Some(VR::OW),
            "PN" => Some(VR::PN),
            "SH" => Some(VR::SH),
            "SL" => Some(VR::SL),
            "SQ" => Some(VR::SQ),
            "SS" => Some(VR::SS),
            "ST" => Some(VR::ST),
            "TM" => Some(VR::TM),
            "UC" => Some(VR::UC),
            "UI" => Some(VR::UI),
            "UL" => Some(VR::UL),
            "UN" => Some(VR::UN),
            "UR" => Some(VR::UR),
            "US" => Some(VR::US),
            "UT" => Some(VR::UT),
            _ => None,
        }
    }

    /// Retrieve a string representation of this VR.
    pub fn to_string(&self) -> &'static str {
        match *self {
            VR::AE => "AE",
            VR::AS => "AS",
            VR::AT => "AT",
            VR::CS => "CS",
            VR::DA => "DA",
            VR::DS => "DS",
            VR::DT => "DT",
            VR::FL => "FL",
            VR::FD => "FD",
            VR::IS => "IS",
            VR::LO => "LO",
            VR::LT => "LT",
            VR::OB => "OB",
            VR::OD => "OD",
            VR::OF => "OF",
            VR::OL => "OL",
            VR::OW => "OW",
            VR::PN => "PN",
            VR::SH => "SH",
            VR::SL => "SL",
            VR::SQ => "SQ",
            VR::SS => "SS",
            VR::ST => "ST",
            VR::TM => "TM",
            VR::UC => "UC",
            VR::UI => "UI",
            VR::UL => "UL",
            VR::UN => "UN",
            VR::UR => "UR",
            VR::US => "US",
            VR::UT => "UT",
        }
    }

    /// Retrieve a copy of this VR's byte representation.
    /// The function returns two alphabetic characters in upper case.
    pub fn to_bytes(&self) -> [u8;2] {
        let bytes = self.to_string().as_bytes();
        [bytes[0], bytes[1]]
    }
}

impl fmt::Display for VR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.to_string())
    }
}
