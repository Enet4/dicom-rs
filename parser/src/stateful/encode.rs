//! Module holding a stateful DICOM data encoding abstraction.
//!
//! The [`StatefulEncoder`] supports encoding of binary data and text
//! while applying the necessary padding to conform to DICOM encoding rules.

use dicom_core::{value::PrimitiveValue, DataElementHeader, Length, Tag, VR};
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

    #[snafu(display("Could not write value data at position {}", position))]
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
pub struct StatefulEncoder<W, E, T = SpecificCharacterSet> {
    to: W,
    encoder: E,
    text: T,
    bytes_written: u64,
    buffer: Vec<u8>,
}

pub type DynStatefulEncoder<'w> = StatefulEncoder<Box<dyn Write + 'w>, DynEncoder<'w, dyn Write>>;

impl<W, E, T> StatefulEncoder<W, E, T> {
    pub fn new(to: W, encoder: E, text: T) -> Self {
        StatefulEncoder {
            to,
            encoder,
            text,
            bytes_written: 0,
            buffer: Vec::with_capacity(128),
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
        Ok(StatefulEncoder::new(to, encoder, charset))
    }
}

impl<W, E> StatefulEncoder<W, E>
where
    W: Write,
    E: EncodeTo<W>,
{
    /// Encode and write a data element header.
    pub fn encode_element_header(&mut self, mut de: DataElementHeader) -> Result<()> {
        if let Some(len) = de.len.get() {
            de.len = Length(even_len(len))
        }
        let bytes = self
            .encoder
            .encode_element_header(&mut self.to, de)
            .context(EncodeData {
                position: self.bytes_written,
            })?;
        self.bytes_written += bytes as u64;
        Ok(())
    }

    /// Encode and write an item header,
    /// where `len` is the specified length of the item
    /// (can be `0xFFFF_FFFF` for undefined length).
    pub fn encode_item_header(&mut self, len: u32) -> Result<()> {
        let len = if len == 0xFFFF_FFFF {
            len
        } else {
            even_len(len)
        };
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

    /// Write the given bytes directly to the inner writer.
    ///
    /// Note that this method
    /// (unlike [`write_bytes`](StatefulEncoder::write_bytes))
    /// does not perform any additional padding.
    pub fn write_raw_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.to.write_all(bytes).context(WriteValueData {
            position: self.bytes_written,
        })?;
        self.bytes_written += bytes.len() as u64;
        Ok(())
    }

    /// Write a primitive DICOM value as a bunch of bytes
    /// directly to the inner writer.
    ///
    /// This method will perform the necessary padding
    /// (always with zeros)
    /// to ensure that the encoded value has an even number of bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        debug_assert!(bytes.len() < u32::max_value() as usize);
        self.to.write_all(bytes).context(WriteValueData {
            position: self.bytes_written,
        })?;
        self.bytes_written += bytes.len() as u64;
        if bytes.len() % 2 != 0 {
            self.to.write_all(&[0]).context(WriteValueData {
                position: self.bytes_written,
            })?;
            self.bytes_written += 1;
        }
        Ok(())
    }

    /// Retrieve the number of bytes written so far by this printer.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Encode and write the values of a pixel data offset table.
    pub fn encode_offset_table(&mut self, table: &[u32]) -> Result<()> {
        self.encoder
            .encode_offset_table(&mut self.to, table)
            .context(EncodeData {
                position: self.bytes_written,
            })?;

        self.bytes_written += table.len() as u64 * 4;
        Ok(())
    }

    /// Encode and write a data element with a primitive value.
    ///
    /// This method will perform the necessary padding to ensure that the
    /// encoded value is an even number of bytes.
    /// Where applicable,
    /// this will use the inner text codec for textual values.
    /// The length property of the header is ignored,
    /// the true byte length of the value in its encoded form is used instead.
    pub fn encode_primitive_element(
        &mut self,
        de: &DataElementHeader,
        value: &PrimitiveValue,
    ) -> Result<()> {
        // intercept string encoding calls to use the text codec
        match value {
            PrimitiveValue::Str(text) => {
                self.encode_text_element(text, *de)?;
                Ok(())
            }
            PrimitiveValue::Strs(texts) => {
                self.encode_texts_element(&texts[..], *de)?;
                Ok(())
            }
            _ => {
                let byte_len = value.calculate_byte_len();
                self.encode_element_header(DataElementHeader {
                    tag: de.tag,
                    vr: de.vr,
                    len: Length(byte_len as u32),
                })?;
                let bytes =
                    self.encoder
                        .encode_primitive(&mut self.to, value)
                        .context(EncodeData {
                            position: self.bytes_written,
                        })?;

                self.bytes_written += bytes as u64;
                if bytes % 2 != 0 {
                    self.to.write_all(&[0]).context(WriteValueData {
                        position: self.bytes_written,
                    })?;
                    self.bytes_written += 1;
                }
                Ok(())
            }
        }
    }

    /// Encode and write a primitive value.
    ///
    /// Its use is not recommended
    /// because the encoded value's real length
    /// might not match the header's length,
    /// leading to an inconsistent data set.
    #[deprecated(since = "0.5.0", note = "use `encode_primitive_element` instead")]
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
                if bytes % 2 != 0 {
                    self.to.write_all(&[0]).context(WriteValueData {
                        position: self.bytes_written,
                    })?;
                    self.bytes_written += 1;
                }
                Ok(())
            }
        }
    }

    fn try_new_codec(&mut self, name: &str) {
        if let Some(codec) = SpecificCharacterSet::from_code(name) {
            self.text = codec;
        } else {
            // TODO(#49) log this as a warning
            eprintln!("Unsupported character set `{}`, ignoring", name);
        }
    }

    fn encode_text(&mut self, text: &str, vr: VR) -> Result<()> {
        let bytes = self.encode_text_untrailed(text, vr)?;
        // pad to even length
        if bytes % 2 == 1 {
            let pad = if vr == VR::UI { b"\0" } else { b" " };
            self.to.write_all(pad).context(WriteValueData {
                position: self.bytes_written,
            })?;
            self.bytes_written += 1;
        }
        Ok(())
    }

    fn encode_text_element(&mut self, text: &str, de: DataElementHeader) -> Result<()> {
        // encode it in memory first so that we know the real length
        let mut encoded_value = self.convert_text_untrailed(text, de.vr)?;
        // pad to even length
        if encoded_value.len() % 2 == 1 {
            let pad = if de.vr == VR::UI { b'\0' } else { b' ' };
            encoded_value.push(pad);
        }

        // now we can write the header with the correct length
        self.encode_element_header(DataElementHeader {
            tag: de.tag,
            vr: de.vr,
            len: Length(encoded_value.len() as u32),
        })?;
        self.to.write_all(&encoded_value).context(WriteValueData {
            position: self.bytes_written,
        })?;
        self.bytes_written += encoded_value.len() as u64;

        // if element is Specific Character Set,
        // update the text codec
        if de.tag == Tag(0x0008, 0x0005) {
            self.try_new_codec(text);
        }

        Ok(())
    }

    fn encode_texts_element<S>(&mut self, texts: &[S], de: DataElementHeader) -> Result<()>
    where
        S: AsRef<str>,
    {
        self.buffer.clear();
        for (i, t) in texts.iter().enumerate() {
            self.buffer
                .extend_from_slice(&self.convert_text_untrailed(t.as_ref(), de.vr)?);
            if i < texts.len() - 1 {
                self.buffer.push(b'\\');
            }
        }
        // pad to even length
        if self.buffer.len() % 2 == 1 {
            let pad = if de.vr == VR::UI { b'\0' } else { b' ' };
            self.buffer.push(pad);
        }

        // now we can write the header with the correct length
        self.encode_element_header(DataElementHeader {
            tag: de.tag,
            vr: de.vr,
            len: Length(self.buffer.len() as u32),
        })?;

        self.to.write_all(&self.buffer).context(WriteValueData {
            position: self.bytes_written,
        })?;
        self.bytes_written += self.buffer.len() as u64;

        // if element is Specific Character Set,
        // update the text codec
        if de.tag == Tag(0x0008, 0x0005) {
            if let Some(charset_name) = texts.first() {
                self.try_new_codec(charset_name.as_ref());
            }
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
        // pad to even length
        if acc % 2 == 1 {
            let pad = if vr == VR::UI { b"\0" } else { b" " };
            self.to.write_all(pad).context(WriteValueData {
                position: self.bytes_written,
            })?;
            self.bytes_written += 1;
        }
        Ok(())
    }

    fn encode_text_untrailed(&mut self, text: &str, vr: VR) -> Result<usize> {
        let data = self.convert_text_untrailed(text, vr)?;
        self.to.write_all(&data).context(WriteValueData {
            position: self.bytes_written,
        })?;
        self.bytes_written += data.len() as u64;
        Ok(data.len())
    }

    fn convert_text_untrailed(&self, text: &str, vr: VR) -> Result<Vec<u8>> {
        match vr {
            VR::AE | VR::AS | VR::CS | VR::DA | VR::DS | VR::DT | VR::IS | VR::TM | VR::UI => {
                // these VRs always use the default character repertoire
                DefaultCharacterSetCodec.encode(text).context(EncodeText {
                    position: self.bytes_written,
                })
            }
            _ => self.text.encode(text).context(EncodeText {
                position: self.bytes_written,
            }),
        }
    }
}

#[inline]
fn even_len(l: u32) -> u32 {
    ((l + 1) & !1) as u32
}

#[cfg(test)]
mod tests {
    use dicom_core::{
        dicom_value, DataElement, DataElementHeader, DicomValue, Length, PrimitiveValue, Tag, VR,
    };
    use dicom_encoding::{
        encode::{explicit_le::ExplicitVRLittleEndianEncoder, EncoderFor},
        text::{SpecificCharacterSet, TextCodec},
    };

    use super::StatefulEncoder;

    /// Odd lengthed values convert to tokens with even padding (PN)
    #[test]
    fn encode_odd_length_element_pn() {
        let element = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            DicomValue::new(dicom_value!(Strs, ["Dall^John"])),
        );

        let mut out: Vec<_> = Vec::new();

        {
            let mut encoder = StatefulEncoder::new(
                &mut out,
                EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
                SpecificCharacterSet::Default,
            );

            encoder
                .encode_primitive_element(element.header(), element.value().primitive().unwrap())
                .unwrap();
        }

        assert_eq!(
            &out,
            &[
                0x10, 0x00, 0x10, 0x00, // tag
                b'P', b'N', // VR
                0x0A, 0x00, // length
                // ---------- value ----------
                b'D', b'a', b'l', b'l', b'^', b'J', b'o', b'h', b'n', b' ',
            ],
        )
    }

    /// Odd lengthed values are encoded with even padding (bytes)
    #[test]
    fn encode_odd_length_element_bytes() {
        let element = DataElement::new(
            Tag(0x7FE0, 0x0010),
            VR::OB,
            DicomValue::new(vec![1; 9].into()),
        );

        let mut out: Vec<_> = Vec::new();

        {
            let mut encoder = StatefulEncoder::new(
                &mut out,
                EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
                SpecificCharacterSet::Default,
            );

            encoder
                .encode_primitive_element(element.header(), element.value().primitive().unwrap())
                .unwrap();
        }

        assert_eq!(
            &out,
            &[
                0xE0, 0x7F, 0x10, 0x00, // tag
                b'O', b'B', // VR
                0x00, 0x00, // reserved
                0x0A, 0x00, 0x00, 0x00, // length
                // ---------- value ----------
                1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
            ],
        )
    }

    /// Odd lengthed values are encoded with even padding (UIDs)
    #[test]
    fn encode_odd_length_element_uid() {
        let element = DataElement::new(
            Tag(0x0000, 0x0002),
            VR::UI,
            DicomValue::new("1.2.840.10008.1.1".into()),
        );

        let mut out: Vec<_> = Vec::new();

        {
            let mut encoder = StatefulEncoder::new(
                &mut out,
                EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
                SpecificCharacterSet::Default,
            );

            encoder
                .encode_primitive_element(element.header(), element.value().primitive().unwrap())
                .unwrap();
        }

        assert_eq!(
            &out,
            &[
                // tag
                0x00, 0x00, 0x02, 0x00, // VR
                b'U', b'I', // length
                0x12, 0x00, // length
                // ---------- value ----------
                b'1', b'.', b'2', b'.', b'8', b'4', b'0', b'.', b'1', b'0', b'0', b'0', b'8', b'.',
                b'1', b'.', b'1', b'\0',
            ],
        )
    }

    /// Odd lengthed item values are encoded with even padding
    #[test]
    fn encode_odd_length_item_bytes() {
        let mut out: Vec<_> = Vec::new();

        {
            let mut encoder = StatefulEncoder::new(
                &mut out,
                EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
                SpecificCharacterSet::Default,
            );

            encoder.encode_item_header(9).unwrap();
            encoder.write_bytes(&[5; 9]).unwrap();
        }

        assert_eq!(
            &out,
            &[
                0xFE, 0xFF, 0x00, 0xE0, // tag (0xFFFE, 0xE000)
                0x0A, 0x00, 0x00, 0x00, // length
                // ---------- value ----------
                5, 5, 5, 5, 5, 5, 5, 5, 5, 0,
            ],
        )
    }

    #[test]
    fn test_even_len() {
        use super::even_len;

        assert_eq!(even_len(0), 0);
        assert_eq!(even_len(1), 2);
        assert_eq!(even_len(2), 2);
        assert_eq!(even_len(3), 4);
        assert_eq!(even_len(4), 4);
        assert_eq!(even_len(5), 6);
        assert_eq!(even_len(6), 6);
        assert_eq!(even_len(0xFFFF_FFFD), 0xFFFF_FFFE);
    }

    /// Test that the stateful encoder updates
    /// the active character set after writing a Specific Character Set element
    /// with a supported text encoding.
    #[test]
    fn update_character_set() {
        const GT: &'static [u8; 54] = &[
            // Tag: (0008,0005) Specific Character Set
            0x08, 0x00, 0x05, 0x00, // VR: CS
            b'C', b'S', // Length: 10
            0x0a, 0x00, // Value: "ISO_IR 192"
            b'I', b'S', b'O', b'_', b'I', b'R', b' ', b'1', b'9', b'2',
            // Tag: (0010,0010) Patient Name
            0x10, 0x00, 0x10, 0x00, // VR: PN
            b'P', b'N', // Length: 28
            0x1c, 0x00, // Value: "Иванков^Андрей "
            0xd0, 0x98, 0xd0, 0xb2, 0xd0, 0xb0, 0xd0, 0xbd, 0xd0, 0xba, 0xd0, 0xbe, 0xd0, 0xb2,
            0x5e, 0xd0, 0x90, 0xd0, 0xbd, 0xd0, 0xb4, 0xd1, 0x80, 0xd0, 0xb5, 0xd0, 0xb9, b' ',
        ];

        let mut sink = Vec::with_capacity(GT.len());

        let mut encoder = StatefulEncoder::new(
            &mut sink,
            EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
            SpecificCharacterSet::Default,
        );

        // encode specific character set
        let scs = DataElementHeader {
            tag: Tag(0x0008, 0x0005),
            vr: VR::CS,
            len: Length(10),
        };
        let scs_value = PrimitiveValue::from("ISO_IR 192");

        encoder.encode_primitive_element(&scs, &scs_value).unwrap();

        // check that the encoder has changed
        assert_eq!(encoder.text.name(), "ISO_IR 192");

        // now encode something non-ASCII
        let pn = DataElementHeader {
            tag: Tag(0x0010, 0x0010),
            vr: VR::PN,
            len: Length(28),
        };
        let pn_value = PrimitiveValue::from("Иванков^Андрей ");
        encoder.encode_primitive_element(&pn, &pn_value).unwrap();

        // test all output against ground truth
        assert_eq!(&sink, GT);
    }
}
