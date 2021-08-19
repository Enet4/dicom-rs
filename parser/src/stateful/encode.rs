//! Module holding a stateful DICOM data encoding abstraction,
//! in a way which supports text encoding.

use dicom_core::Tag;
use dicom_core::{value::PrimitiveValue, DataElementHeader, VR};
use dicom_encoding::transfer_syntax::DynEncoder;
use dicom_encoding::{
    encode::EncodeTo,
    text::{DefaultCharacterSetCodec, SpecificCharacterSet, TextCodec},
    TransferSyntax,
};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::io::Write;

#[derive(Debug, Snafu)]
#[non_exhaustive]
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

    #[snafu(display("Failed to encode a data piece at position {}", position))]
    EncodeData {
        position: u64,
        source: dicom_encoding::encode::Error,
    },

    #[snafu(display("Could not encode text at position {}", position))]
    EncodeText {
        position: u64,
        source: dicom_encoding::text::EncodeTextError,
    },

    #[snafu(display("Could not write value data to writer"))]
    WriteValueData {
        position: u64,
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
pub struct StatefulEncoder<W, E, T = Box<dyn TextCodec>> {
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
}

impl<'s> DynStatefulEncoder<'s> {
    pub fn from_transfer_syntax(
        to: Box<dyn Write + 's>,
        ts: TransferSyntax,
        charset: SpecificCharacterSet,
    ) -> Result<Self> {
        let encoder = ts
            .encoder()
            .context(UnsupportedTransferSyntax { ts: ts.uid() })?;
        let text = charset
            .codec()
            .context(UnsupportedCharacterSet { charset })?;

        Ok(StatefulEncoder::new(to, encoder, text))
    }
}

impl<W, E> StatefulEncoder<W, E, Box<dyn TextCodec>>
where
    W: Write,
    E: EncodeTo<W>,
{
    /// Encode and write a data element header.
    pub fn encode_element_header(&mut self, de: DataElementHeader) -> Result<()> {
        let bytes = self
            .encoder
            .encode_element_header(&mut self.to, de)
            .context(EncodeData {
                position: self.bytes_written,
            })?;
        self.bytes_written += bytes as u64;
        Ok(())
    }

    /// Encode and write an item header.
    pub fn encode_item_header(&mut self, len: u32) -> Result<()> {
        self.encoder
            .encode_item_header(&mut self.to, len)
            .context(EncodeData {
                position: self.bytes_written,
            })?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Encode and write an item delimiter.
    pub fn encode_item_delimiter(&mut self) -> Result<()> {
        self.encoder
            .encode_item_delimiter(&mut self.to)
            .context(EncodeData {
                position: self.bytes_written,
            })?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Encode and write a sequence delimiter.
    pub fn encode_sequence_delimiter(&mut self) -> Result<()> {
        self.encoder
            .encode_sequence_delimiter(&mut self.to)
            .context(EncodeData {
                position: self.bytes_written,
            })?;
        self.bytes_written += 8;
        Ok(())
    }

    /// Write all bytes directly to the inner writer.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.to.write_all(bytes).context(WriteValueData {
            position: self.bytes_written,
        })?;
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

                // if element is Specific Character Set,
                // update the text codec
                if de.tag == Tag(0x0008, 0x0005) {
                    self.try_new_codec(text);
                }

                Ok(())
            }
            PrimitiveValue::Strs(texts) => {
                self.encode_texts(&texts[..], de.vr())?;
                
                // if element is Specific Character Set,
                // update the text codec
                if de.tag == Tag(0x0008, 0x0005) {
                    if let Some(charset_name) = texts.first() {
                        self.try_new_codec(charset_name);
                    }
                }
                Ok(())
            }
            _ => {
                let bytes =
                    self.encoder
                        .encode_primitive(&mut self.to, value)
                        .context(EncodeData {
                            position: self.bytes_written,
                        })?;
                self.bytes_written += bytes as u64;
                Ok(())
            }
        }
    }

    fn try_new_codec(&mut self, name: &str) {
        if let Some(codec) = SpecificCharacterSet::from_code(name).and_then(SpecificCharacterSet::codec) {
            self.text = codec;
        } else {
            // TODO(#49) log this as a warning
            eprintln!("Unsupported character set `{}`, ignoring", name);
        }
    } 

    fn encode_text(&mut self, text: &str, vr: VR) -> Result<()> {
        let bytes = self.encode_text_untrailed(text, vr)?;
        if bytes % 2 == 1 {
            self.to.write_all(b" ").context(WriteValueData {
                position: self.bytes_written,
            })?;
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
                self.to.write_all(b"\\").context(WriteValueData {
                    position: self.bytes_written,
                })?;
                acc += 1;
                self.bytes_written += 1;
            }
        }
        if acc % 2 == 1 {
            self.to.write_all(b" ").context(WriteValueData {
                position: self.bytes_written,
            })?;
            self.bytes_written += 1;
        }
        Ok(())
    }

    fn encode_text_untrailed(&mut self, text: &str, vr: VR) -> Result<usize> {
        let data = match vr {
            VR::AE | VR::AS | VR::CS | VR::DA | VR::DS | VR::DT | VR::IS | VR::TM | VR::UI => {
                // these VRs always use the default character repertoire
                DefaultCharacterSetCodec.encode(text).context(EncodeText {
                    position: self.bytes_written,
                })?
            }
            _ => self.text.encode(text).context(EncodeText {
                position: self.bytes_written,
            })?,
        };
        self.to.write_all(&data).context(WriteValueData {
            position: self.bytes_written,
        })?;
        self.bytes_written += data.len() as u64;
        Ok(data.len())
    }
}

#[cfg(test)]
mod tests {
    use dicom_core::{DataElementHeader, Length, PrimitiveValue, Tag, VR};
    use dicom_encoding::{
        encode::EncoderFor,
        text::{DefaultCharacterSetCodec, DynamicTextCodec},
        transfer_syntax::explicit_le::ExplicitVRLittleEndianEncoder,
    };

    use super::StatefulEncoder;

    /// Test that the stateful encoder updates
    /// the active character set after writing a Specific Character Set element
    /// with a supported text encoding.
    #[test]
    fn update_character_set() {
        const GT: &'static [u8; 54] = &[
            // Tag: (0008,0005) Specific Character Set
            0x08, 0x00, 0x05, 0x00,
            // VR: CS
            b'C', b'S',
            // Length: 10
            0x0a, 0x00,
            // Value: "ISO_IR 192"
            b'I', b'S', b'O', b'_', b'I', b'R', b' ', b'1', b'9', b'2',
            // Tag: (0010,0010) Patient Name
            0x10, 0x00, 0x10, 0x00,
            // VR: PN
            b'P', b'N',
            // Length: 28
            0x1c, 0x00,
            // Value: "Иванков^Андрей "
            0xd0, 0x98, 0xd0, 0xb2, 0xd0, 0xb0, 0xd0, 0xbd, 0xd0, 0xba,
            0xd0, 0xbe, 0xd0, 0xb2, 0x5e, 0xd0, 0x90, 0xd0, 0xbd, 0xd0,
            0xb4, 0xd1, 0x80, 0xd0, 0xb5, 0xd0, 0xb9, b' ',
        ];

        let mut sink = Vec::with_capacity(GT.len());

        let mut encoder = StatefulEncoder::new(
            &mut sink,
            EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
            Box::new(DefaultCharacterSetCodec) as DynamicTextCodec,
        );

        // encode specific character set
        let scs = DataElementHeader {
            tag: Tag(0x0008, 0x0005),
            vr: VR::CS,
            len: Length(10),
        };
        let scs_value = PrimitiveValue::from("ISO_IR 192");

        encoder.encode_element_header(scs).unwrap();

        encoder.encode_primitive(&scs, &scs_value).unwrap();

        // check that the encoder has changed
        assert_eq!(encoder.text.name(), "ISO_IR 192");

        // now encode something non-ASCII
        let pn = DataElementHeader {
            tag: Tag(0x0010, 0x0010),
            vr: VR::PN,
            len: Length(28),
        };
        let pn_value = PrimitiveValue::from("Иванков^Андрей ");
        encoder.encode_element_header(pn).unwrap();
        encoder.encode_primitive(&pn, &pn_value).unwrap();

        // test all output against ground truth
        assert_eq!(
            &sink,
            GT,
        );
    }
}
