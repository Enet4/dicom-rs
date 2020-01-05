//! For the lack of a better name, this module is to hold the analogous to
//! the "parser" module, but for encoding in a way which supports text
//! encoding.
//!

use crate::error::{Error, Result};
use dicom_core::{value::PrimitiveValue, DataElementHeader, VR};
use dicom_encoding::{
    encode::EncodeTo,
    text::{DefaultCharacterSetCodec, SpecificCharacterSet, TextCodec},
    TransferSyntax,
};
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

impl<W, E, T> Printer<W, E, T>
where
    W: Write,
    E: EncodeTo<W>,
    T: TextCodec,
{
    /// Encode and write a data element header.
    pub fn encode_element_header(&mut self, de: DataElementHeader) -> Result<()> {
        let bytes = self.encoder.encode_element_header(&mut self.to, de)?;
        self.bytes_written += bytes as u64;
        Ok(())
    }

    /// Encode and write an item header.
    pub fn encode_item_header(&mut self, len: u32) -> Result<()> {
        self.encoder.encode_item_header(&mut self.to, len)?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Encode and write an item delimiter.
    pub fn encode_item_delimiter(&mut self) -> Result<()> {
        self.encoder.encode_item_delimiter(&mut self.to)?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Encode and write a sequence delimiter.
    pub fn encode_sequence_delimiter(&mut self) -> Result<()> {
        self.encoder.encode_sequence_delimiter(&mut self.to)?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Retrieve the number of bytes written by this printer.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Encode and write a primitive value. Where applicable, this printer
    /// will use the inner text codec for textual values.
    pub fn encode_primitive(
        &mut self,
        de: &DataElementHeader,
        value: &PrimitiveValue,
    ) -> Result<()> {
        // intercept string encoding calls to use the text codec
        match value {
            PrimitiveValue::Str(text) => {
                self.encode_text(text, de.vr())?;
                Ok(())
            }
            PrimitiveValue::Strs(texts) => {
                self.encode_texts(&texts[..], de.vr())?;
                Ok(())
            }
            _ => {
                let bytes = self.encoder.encode_primitive(&mut self.to, value)?;
                self.bytes_written += bytes as u64;
                Ok(())
            }
        }
    }

    fn encode_text(&mut self, text: &str, vr: VR) -> Result<()> {
        let bytes = self.encode_text_untrailed(text, vr)?;
        if bytes % 2 == 1 {
            self.to.write_all(b" ")?;
            self.bytes_written += 1;
        }
        Ok(())
    }

    fn encode_texts<S>(&mut self, texts: &[S], vr: VR) -> Result<()>
    where
        S: AsRef<str>,
    {
        let mut acc = 0;
        for (i, text) in texts.iter().enumerate() {
            acc += self.encode_text_untrailed(text.as_ref(), vr)?;
            if i < texts.len() - 1 {
                self.to.write_all(b"\\")?;
                acc += 1;
                self.bytes_written += 1;
            }
        }
        if acc % 2 == 1 {
            self.to.write_all(b" ")?;
            self.bytes_written += 1;
        }
        Ok(())
    }

    fn encode_text_untrailed(&mut self, text: &str, vr: VR) -> Result<usize> {
        let data = match vr {
            VR::AE | VR::AS | VR::CS | VR::DA | VR::DS | VR::DT | VR::IS | VR::TM | VR::UI => {
                // these VRs always use the default character repertoire
                DefaultCharacterSetCodec.encode(text)?
            }
            _ => self.text.encode(text)?,
        };
        self.to.write_all(&data)?;
        self.bytes_written += data.len() as u64;
        Ok(data.len())
    }
}
