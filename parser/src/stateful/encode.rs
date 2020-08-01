//! Module holding a stateful DICOM data encoding abstraction,
//! in a way which supports text encoding.
//!

use dicom_core::{value::PrimitiveValue, DataElementHeader, VR};
use dicom_encoding::transfer_syntax::DynEncoder;
use dicom_encoding::{
    encode::EncodeTo,
    text::{DefaultCharacterSetCodec, SpecificCharacterSet, TextCodec},
    TransferSyntax,
};
use std::io::Write;
use snafu::{Backtrace, GenerateBacktrace, ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Encoding in transfer syntax {} is unsupported", ts))]
    UnsupportedTransferSyntax {
        ts: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Unsupported character set {:?}", charset))]
    UnsupportedCharacterSet {
        charset: SpecificCharacterSet,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to encode a data piece"))]
    EncodeData {
        source: dicom_encoding::error::Error,
    },

    #[snafu(display("Could not encode text"))]
    EncodeText {
        source: dicom_encoding::error::TextEncodingError,
    },

    #[snafu(display("Could not write value data to writer"))]
    WriteValueData {
        source: std::io::Error,
        backtrace: Backtrace,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Also called a printer, this encoder type provides a stateful mid-level
/// abstraction for writing DICOM content. Unlike `Encode`,
/// the stateful encoder knows how to write text values and keeps track
/// of how many bytes were written.
/// `W` is the write target, `E` is the encoder, and `T` is the text codec.
#[derive(Debug)]
pub struct StatefulEncoder<W, E, T> {
    to: W,
    encoder: E,
    text: T,
    bytes_written: u64,
}

pub type DynStatefulEncoder<'w> =
    StatefulEncoder<Box<dyn Write + 'w>, DynEncoder<'w, dyn Write>, Box<dyn TextCodec>>;

impl<W, E, T> StatefulEncoder<W, E, T> {
    pub fn new(to: W, encoder: E, text: T) -> Self {
        StatefulEncoder {
            to,
            encoder,
            text,
            bytes_written: 0,
        }
    }

    pub fn with_text<U>(self, text: U) -> StatefulEncoder<W, E, U> {
        StatefulEncoder {
            to: self.to,
            encoder: self.encoder,
            text,
            bytes_written: 0,
        }
    }
}

impl<'s> DynStatefulEncoder<'s> {
    pub fn from_transfer_syntax(
        to: Box<dyn Write + 's>,
        ts: TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<Self> {
        let encoder = ts
            .encoder()
            .ok_or_else(|| Error::UnsupportedTransferSyntax {
                ts: ts.uid(),
                backtrace: Backtrace::generate(),
            })?;
        let text = cs.codec().ok_or_else(|| Error::UnsupportedCharacterSet {
            charset: cs,
            backtrace: Backtrace::generate(),
        })?;

        Ok(StatefulEncoder::new(to, encoder, text))
    }
}

impl<W, E, T> StatefulEncoder<W, E, T>
where
    W: Write,
    E: EncodeTo<W>,
    T: TextCodec,
{
    /// Encode and write a data element header.
    pub fn encode_element_header(&mut self, de: DataElementHeader) -> Result<()> {
        let bytes = self.encoder.encode_element_header(&mut self.to, de).context(EncodeData)?;
        self.bytes_written += bytes as u64;
        Ok(())
    }

    /// Encode and write an item header.
    pub fn encode_item_header(&mut self, len: u32) -> Result<()> {
        self.encoder.encode_item_header(&mut self.to, len).context(EncodeData)?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Encode and write an item delimiter.
    pub fn encode_item_delimiter(&mut self) -> Result<()> {
        self.encoder.encode_item_delimiter(&mut self.to).context(EncodeData)?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Encode and write a sequence delimiter.
    pub fn encode_sequence_delimiter(&mut self) -> Result<()> {
        self.encoder.encode_sequence_delimiter(&mut self.to).context(EncodeData)?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Write all bytes directly to the inner writer.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.to.write_all(bytes).context(WriteValueData)?;
        self.bytes_written += bytes.len() as u64;
        Ok(())
    }

    /// Retrieve the number of bytes written so far by this printer.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Encode and write a primitive value. Where applicable, this
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
                let bytes = self.encoder.encode_primitive(&mut self.to, value).context(EncodeData)?;
                self.bytes_written += bytes as u64;
                Ok(())
            }
        }
    }

    fn encode_text(&mut self, text: &str, vr: VR) -> Result<()> {
        let bytes = self.encode_text_untrailed(text, vr)?;
        if bytes % 2 == 1 {
            self.to.write_all(b" ").context(WriteValueData)?;
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
                self.to.write_all(b"\\").context(WriteValueData)?;
                acc += 1;
                self.bytes_written += 1;
            }
        }
        if acc % 2 == 1 {
            self.to.write_all(b" ").context(WriteValueData)?;
            self.bytes_written += 1;
        }
        Ok(())
    }

    fn encode_text_untrailed(&mut self, text: &str, vr: VR) -> Result<usize> {
        let data = match vr {
            VR::AE | VR::AS | VR::CS | VR::DA | VR::DS | VR::DT | VR::IS | VR::TM | VR::UI => {
                // these VRs always use the default character repertoire
                DefaultCharacterSetCodec.encode(text).context(EncodeText)?
            }
            _ => self.text.encode(text).context(EncodeText)?,
        };
        self.to.write_all(&data).context(WriteValueData)?;
        self.bytes_written += data.len() as u64;
        Ok(data.len())
    }
}
