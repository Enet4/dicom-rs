//! This module contains reusable components for encoding and decoding text in DICOM
//! data structures, including support for character repertoires.
//!
//! At the moment the following character sets are supported:
//!
//! | Character Set                 | decoding support | encoding support |
//! |-------------------------------|------------------|------------------|
//! | ISO-IR 6 (default)            | ✓ | ✓ |
//! | ISO-IR 100 (ISO-8859-1): Right-hand part of the Latin alphabet no. 1, the Western Europe character set | ✓ | ✓ |
//! | ISO-IR 101 (ISO-8859-2): Right-hand part of the Latin alphabet no. 2, the Central/Eastern Europe character set | ✓ | ✓ |
//! | ISO-IR 109 (ISO-8859-3): Right-hand part of the Latin alphabet no. 3, the South Europe character set | ✓ | ✓ |
//! | ISO-IR 110 (ISO-8859-4): Right-hand part of the Latin alphabet no. 4, the North Europe character set | ✓ | ✓ |
//! | ISO-IR 144 (ISO-8859-5): The Latin/Cyrillic character set | ✓ | ✓ |
//! | ISO-IR 192: The Unicode character set based on the UTF-8 encoding | ✓ | ✓ |
//! | GB18030: The Simplified Chinese character set | ✓ | ✓ |
//! | JIS X 0201-1976: Code for Information Interchange | x | x |
//! | JIS X 0208-1990: Code for the Japanese Graphic Character set for information interchange | x | x |
//! | JIS X 0212-1990: Code of the supplementary Japanese Graphic Character set for information interchange | x | x |
//! | KS X 1001 (registered as ISO-IR 149) for Korean Language | x | x |
//! | TIS 620-2533 (1990) Thai Characters Code for Information Interchange | x | x |
//! | GB2312: Simplified Chinese character set | x | x |
//!
//! These capabilities are available through [`SpecificCharacterSet`].

use encoding::all::{GB18030, ISO_8859_1, ISO_8859_2, ISO_8859_3, ISO_8859_4, ISO_8859_5, UTF_8};
use encoding::{DecoderTrap, EncoderTrap, Encoding, RawDecoder, StringWriter};
use snafu::{Backtrace, Snafu};
use std::borrow::Cow;
use std::fmt::Debug;

/// An error type for text encoding issues.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum EncodeTextError {
    /// A custom error message,
    /// for when the underlying error type does not encode error semantics
    /// into type variants.
    #[snafu(display("{}", message))]
    EncodeCustom {
        /// The error message in plain text.
        message: Cow<'static, str>,
        /// The generated backtrace, if available.
        backtrace: Backtrace,
    },
}

/// An error type for text decoding issues.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum DecodeTextError {
    /// A custom error message,
    /// for when the underlying error type does not encode error semantics
    /// into type variants.
    #[snafu(display("{}", message))]
    DecodeCustom {
        /// The error message in plain text.
        message: Cow<'static, str>,
        /// The generated backtrace, if available.
        backtrace: Backtrace,
    },
}

type EncodeResult<T> = Result<T, EncodeTextError>;
type DecodeResult<T> = Result<T, DecodeTextError>;

/// A holder of encoding and decoding mechanisms for text in DICOM content,
/// which according to the standard, depends on the specific character set.
pub trait TextCodec {
    /// Obtain the defined term (unique name) of the text encoding,
    /// which may be used as the value of a
    /// Specific Character Set (0008, 0005) element to refer to this codec.
    ///
    /// Should contain no leading or trailing spaces.
    /// This method may be useful for testing purposes, considering that
    /// `TextCodec` is often used as a trait object.
    fn name(&self) -> Cow<'static, str>;

    /// Decode the given byte buffer as a single string. The resulting string
    /// _may_ contain backslash characters ('\') to delimit individual values,
    /// and should be split later on if required.
    fn decode(&self, text: &[u8]) -> DecodeResult<String>;

    /// Encode a text value into a byte vector. The input string can
    /// feature multiple text values by using the backslash character ('\')
    /// as the value delimiter.
    fn encode(&self, text: &str) -> EncodeResult<Vec<u8>>;
}

impl<T: ?Sized> TextCodec for Box<T>
where
    T: TextCodec,
{
    fn name(&self) -> Cow<'static, str> {
        self.as_ref().name()
    }

    fn decode(&self, text: &[u8]) -> DecodeResult<String> {
        self.as_ref().decode(text)
    }

    fn encode(&self, text: &str) -> EncodeResult<Vec<u8>> {
        self.as_ref().encode(text)
    }
}

impl<'a, T: ?Sized> TextCodec for &'a T
where
    T: TextCodec,
{
    fn name(&self) -> Cow<'static, str> {
        (**self).name()
    }

    fn decode(&self, text: &[u8]) -> DecodeResult<String> {
        (**self).decode(text)
    }

    fn encode(&self, text: &str) -> EncodeResult<Vec<u8>> {
        (**self).encode(text)
    }
}

/// A descriptor for a specific character set,
/// taking part in text encoding and decoding
/// as per [PS3.5 ch 6 6.1](https://dicom.nema.org/medical/dicom/2023e/output/chtml/part05/chapter_6.html#sect_6.1).
///
/// # Example
///
/// Use [`from_code`](SpecificCharacterSet::from_code)
/// or one of the associated constants to create a character set.
/// From there, use the [`TextCodec`] trait to encode and decode text.
///
/// ```
/// use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
///
/// let character_set = SpecificCharacterSet::from_code("ISO_IR 100").unwrap();
/// assert_eq!(character_set, SpecificCharacterSet::ISO_IR_100);
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SpecificCharacterSet(CharsetImpl);

impl SpecificCharacterSet {
    /// ISO IR 6: The default character set, as defined by the DICOM standard.
    pub const ISO_IR_6: SpecificCharacterSet = SpecificCharacterSet(CharsetImpl::Default);

    // ISO IR 100: ISO 8859-1, the Western Europe character set
    pub const ISO_IR_100: SpecificCharacterSet = SpecificCharacterSet(CharsetImpl::IsoIr100);

    /// ISO IR 192: UTF-8 encoding
    pub const ISO_IR_192: SpecificCharacterSet = SpecificCharacterSet(CharsetImpl::IsoIr192);

    /// Obtain the specific character set identified by the given code string.
    ///
    /// Supported code strings include the possible values
    /// in the respective DICOM element (0008, 0005).
    ///
    /// # Example
    ///
    /// ```
    /// use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
    ///
    /// let character_set = SpecificCharacterSet::from_code("ISO_IR 100").unwrap();
    /// assert_eq!(character_set.name(), "ISO_IR 100");
    /// ```
    pub fn from_code(code: &str) -> Option<Self> {
        CharsetImpl::from_code(code).map(SpecificCharacterSet)
    }
}

impl TextCodec for SpecificCharacterSet {
    fn name(&self) -> Cow<'static, str> {
        self.0.name()
    }

    fn decode(&self, text: &[u8]) -> DecodeResult<String> {
        self.0.decode(text)
    }

    fn encode(&self, text: &str) -> EncodeResult<Vec<u8>> {
        self.0.encode(text)
    }
}

/// An enum type for individual supported character sets.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
#[non_exhaustive]
enum CharsetImpl {
    /// **ISO-IR 6**: the default character set.
    #[default]
    Default,
    /// **ISO-IR 100** (ISO-8859-1): Right-hand part of the Latin alphabet no. 1,
    /// the Western Europe character set.
    IsoIr100,
    /// **ISO-IR 101** (ISO-8859-2): Right-hand part of the Latin alphabet no. 2,
    /// the Central/Eastern Europe character set.
    IsoIr101,
    /// **ISO-IR 109** (ISO-8859-3): Right-hand part of the Latin alphabet no. 3,
    /// the South Europe character set.
    IsoIr109,
    /// **ISO-IR 110** (ISO-8859-4): Right-hand part of the Latin alphabet no. 4,
    /// the North Europe character set.
    IsoIr110,
    /// **ISO-IR 144** (ISO-8859-5): The Latin/Cyrillic character set.
    IsoIr144,
    /// **ISO-IR 192**: The Unicode character set based on the UTF-8 encoding.
    IsoIr192,
    /// **GB18030**: The Simplified Chinese character set.
    Gb18030,
    // Support for more text encodings is tracked in issue #40.
}

impl CharsetImpl {
    /// Obtain the specific character set identified by the given code string.
    ///
    /// Supported code strings include the possible values
    /// in the respective DICOM element (0008, 0005).
    pub fn from_code(uid: &str) -> Option<Self> {
        use self::CharsetImpl::*;
        match uid.trim_end() {
            "Default" | "ISO_IR_6" | "ISO_IR 6" | "ISO 2022 IR 6" => Some(Default),
            "ISO_IR_100" | "ISO_IR 100" | "ISO 2022 IR 100" => Some(IsoIr100),
            "ISO_IR_101" | "ISO_IR 101" | "ISO 2022 IR 101" => Some(IsoIr101),
            "ISO_IR_109" | "ISO_IR 109" | "ISO 2022 IR 109" => Some(IsoIr109),
            "ISO_IR_110" | "ISO_IR 110" | "ISO 2022 IR 110" => Some(IsoIr110),
            "ISO_IR_144" | "ISO_IR 144" | "ISO 2022 IR 144" => Some(IsoIr144),
            "ISO_IR_192" | "ISO_IR 192" => Some(IsoIr192),
            "GB18030" => Some(Gb18030),
            _ => None,
        }
    }
}

impl TextCodec for CharsetImpl {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            CharsetImpl::Default => "ISO_IR 6",
            CharsetImpl::IsoIr100 => "ISO_IR 100",
            CharsetImpl::IsoIr101 => "ISO_IR 101",
            CharsetImpl::IsoIr109 => "ISO_IR 109",
            CharsetImpl::IsoIr110 => "ISO_IR 110",
            CharsetImpl::IsoIr144 => "ISO_IR 144",
            CharsetImpl::IsoIr192 => "ISO_IR 192",
            CharsetImpl::Gb18030 => "GB18030",
        })
    }

    fn decode(&self, text: &[u8]) -> DecodeResult<String> {
        match self {
            CharsetImpl::Default => DefaultCharacterSetCodec.decode(text),
            CharsetImpl::IsoIr100 => IsoIr100CharacterSetCodec.decode(text),
            CharsetImpl::IsoIr101 => IsoIr101CharacterSetCodec.decode(text),
            CharsetImpl::IsoIr109 => IsoIr109CharacterSetCodec.decode(text),
            CharsetImpl::IsoIr110 => IsoIr110CharacterSetCodec.decode(text),
            CharsetImpl::IsoIr144 => IsoIr144CharacterSetCodec.decode(text),
            CharsetImpl::IsoIr192 => Utf8CharacterSetCodec.decode(text),
            CharsetImpl::Gb18030 => Gb18030CharacterSetCodec.decode(text),
        }
    }

    fn encode(&self, text: &str) -> EncodeResult<Vec<u8>> {
        match self {
            CharsetImpl::Default => DefaultCharacterSetCodec.encode(text),
            CharsetImpl::IsoIr100 => IsoIr100CharacterSetCodec.encode(text),
            CharsetImpl::IsoIr101 => IsoIr101CharacterSetCodec.encode(text),
            CharsetImpl::IsoIr109 => IsoIr109CharacterSetCodec.encode(text),
            CharsetImpl::IsoIr110 => IsoIr110CharacterSetCodec.encode(text),
            CharsetImpl::IsoIr144 => IsoIr144CharacterSetCodec.encode(text),
            CharsetImpl::IsoIr192 => Utf8CharacterSetCodec.encode(text),
            CharsetImpl::Gb18030 => Gb18030CharacterSetCodec.encode(text),
        }
    }
}

fn decode_text_trap(
    _decoder: &mut dyn RawDecoder,
    input: &[u8],
    output: &mut dyn StringWriter,
) -> bool {
    let c = input[0];
    let o0 = c & 7;
    let o1 = (c & 56) >> 3;
    let o2 = (c & 192) >> 6;
    output.write_char('\\');
    output.write_char((o2 + b'0') as char);
    output.write_char((o1 + b'0') as char);
    output.write_char((o0 + b'0') as char);
    true
}

/// Create and implement a character set type using the `encoding` crate.
macro_rules! decl_character_set {
    ($typ: ident, $term: literal, $val: expr) => {
        #[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
        #[doc = "Data type for the "]
        #[doc = $term]
        #[doc = "character set encoding."]
        pub struct $typ;

        impl TextCodec for $typ {
            fn name(&self) -> Cow<'static, str> {
                Cow::Borrowed($term)
            }

            fn decode(&self, text: &[u8]) -> DecodeResult<String> {
                $val.decode(text, DecoderTrap::Call(decode_text_trap))
                    .map_err(|message| DecodeCustomSnafu { message }.build())
            }

            fn encode(&self, text: &str) -> EncodeResult<Vec<u8>> {
                $val.encode(text, EncoderTrap::Strict)
                    .map_err(|message| EncodeCustomSnafu { message }.build())
            }
        }
    };
}

/// Data type representing the default character set.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
pub struct DefaultCharacterSetCodec;

impl TextCodec for DefaultCharacterSetCodec {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ISO_IR 6")
    }

    fn decode(&self, text: &[u8]) -> DecodeResult<String> {
        // Using 8859-1 because it is a superset. Reiterations of this impl
        // should check for invalid character codes (#40).
        ISO_8859_1
            .decode(text, DecoderTrap::Call(decode_text_trap))
            .map_err(|message| DecodeCustomSnafu { message }.build())
    }

    fn encode(&self, text: &str) -> EncodeResult<Vec<u8>> {
        ISO_8859_1
            .encode(text, EncoderTrap::Strict)
            .map_err(|message| EncodeCustomSnafu { message }.build())
    }
}

decl_character_set!(IsoIr100CharacterSetCodec, "ISO_IR 100", ISO_8859_1);
decl_character_set!(IsoIr101CharacterSetCodec, "ISO_IR 101", ISO_8859_2);
decl_character_set!(IsoIr109CharacterSetCodec, "ISO_IR 109", ISO_8859_3);
decl_character_set!(IsoIr110CharacterSetCodec, "ISO_IR 110", ISO_8859_4);
decl_character_set!(IsoIr144CharacterSetCodec, "ISO_IR 144", ISO_8859_5);
decl_character_set!(Utf8CharacterSetCodec, "ISO_IR 192", UTF_8);
decl_character_set!(Gb18030CharacterSetCodec, "GB18030", GB18030);

/// The result of a text validation procedure (please see [`validate_iso_8859`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextValidationOutcome {
    /// The text is fully valid and can be safely decoded.
    Ok,
    /// Some characters may have to be replaced, other than that the text can be safely decoded.
    BadCharacters,
    /// The text cannot be decoded.
    NotOk,
}

/// Check whether the given byte slice contains valid text from the default character repertoire.
pub fn validate_iso_8859(text: &[u8]) -> TextValidationOutcome {
    if ISO_8859_1.decode(text, DecoderTrap::Strict).is_err() {
        match ISO_8859_1.decode(text, DecoderTrap::Call(decode_text_trap)) {
            Ok(_) => TextValidationOutcome::BadCharacters,
            Err(_) => TextValidationOutcome::NotOk,
        }
    } else {
        TextValidationOutcome::Ok
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Date value representation.
pub fn validate_da(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| c.is_ascii_digit()) {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Time value representation.
pub fn validate_tm(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| match c {
        b'\\' | b'.' | b'-' | b' ' => true,
        c => c.is_ascii_digit(),
    }) {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Date Time value representation.
pub fn validate_dt(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| match c {
        b'.' | b'-' | b'+' | b' ' | b'\\' => true,
        c => c.is_ascii_digit(),
    }) {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Code String value representation.
pub fn validate_cs(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| match c {
        b' ' | b'_' => true,
        c => c.is_ascii_digit() || c.is_ascii_uppercase(),
    }) {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_codec<T>(codec: T, string: &str, bytes: &[u8])
    where
        T: TextCodec,
    {
        assert_eq!(codec.encode(string).expect("encoding"), bytes);
        assert_eq!(codec.decode(bytes).expect("decoding"), string);
    }

    #[test]
    fn iso_ir_6_baseline() {
        let codec = SpecificCharacterSet::default();
        test_codec(codec, "Smith^John", b"Smith^John");
    }

    #[test]
    fn iso_ir_192_baseline() {
        let codec = SpecificCharacterSet::ISO_IR_192;
        test_codec(&codec, "Simões^John", "Simões^John".as_bytes());
        test_codec(codec, "Иванков^Андрей", "Иванков^Андрей".as_bytes());
    }

    #[test]
    fn iso_ir_100_baseline() {
        let codec = SpecificCharacterSet(CharsetImpl::IsoIr100);
        test_codec(&codec, "Simões^João", b"Sim\xF5es^Jo\xE3o");
        test_codec(codec, "Günther^Hans", b"G\xfcnther^Hans");
    }

    #[test]
    fn iso_ir_101_baseline() {
        let codec = SpecificCharacterSet(CharsetImpl::IsoIr101);
        test_codec(codec, "Günther^Hans", b"G\xfcnther^Hans");
    }

    #[test]
    fn iso_ir_144_baseline() {
        let codec = SpecificCharacterSet(CharsetImpl::IsoIr144);
        test_codec(
            codec,
            "Иванков^Андрей",
            b"\xb8\xd2\xd0\xdd\xda\xde\xd2^\xb0\xdd\xd4\xe0\xd5\xd9",
        );
    }
}
