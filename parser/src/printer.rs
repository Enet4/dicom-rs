//! For the lack of a better name, this module is to hold the analogous to
//! the "parser" module, but for encoding in a way which supports text
//! encoding.
//!

use crate::error::{Error, Result};
use dicom_encoding::encode::EncodeTo;
use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
use dicom_encoding::TransferSyntax;
use std::io::Write;

/// A mid-level abstraction for writing DICOM content. Unlike `Encode`,
/// the printer also knows how to write text values.
/// `W` is the write target, `E` is the encoder, and `T` is the text formatter.
#[derive(Debug)]
pub struct Printer<W, E, T> {
    to: W,
    encoder: E,
    text: T,
    bytes_written: u64,
}

pub type DynamicDicomPrinter =
    Printer<Box<dyn Write>, Box<dyn EncodeTo<dyn Write>>, Box<dyn TextCodec>>;

impl<W, E, T> Printer<W, E, T> {
    pub fn new(to: W, encoder: E, text: T) -> Self {
        Printer {
            to,
            encoder,
            text,
            bytes_written: 0,
        }
    }

    pub fn with_text<U>(self, text: U) -> Printer<W, E, U> {
        Printer {
            to: self.to,
            encoder: self.encoder,
            text,
            bytes_written: 0,
        }
    }
}

impl DynamicDicomPrinter {
    pub fn from_transfer_syntax(
        to: Box<dyn Write>,
        ts: TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<Self> {
        let encoder = ts
            .encoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax)?;
        let text = cs.codec().ok_or_else(|| Error::UnsupportedCharacterSet)?;

        Ok(Printer::new(to, encoder, text))
    }
}
