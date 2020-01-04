use crate::dataset::*;
use crate::error::{Error, Result};
use dicom_core::{DataElementHeader, Length, VR};
use dicom_encoding::encode::{Encode, EncodeTo};
use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
use dicom_encoding::TransferSyntax;
use std::io::Write;

/// A token representing a sequence or item start.
#[derive(Debug)]
struct SeqToken {
    /// Whether it is the start of a sequence or the start of an item.
    typ: SeqTokenType,
    /// The length of the value, as indicated by the starting element,
    /// can be unknown.
    len: Length,
}

/// A stateful device for printing a DICOM data set in sequential order.
/// This is analogous to the `DatasetReader` type for converting data
/// set tokens to bytes.
#[derive(Debug)]
pub struct DataSetWriter<W, E, T> {
    to: W,
    encoder: E,
    text: T,
    seq_tokens: Vec<SeqToken>,
}

impl<W> DataSetWriter<W, Box<dyn EncodeTo<W>>, Box<dyn TextCodec>>
where
    W: Write,
{
    pub fn with_ts_cs(to: W, ts: TransferSyntax, cs: SpecificCharacterSet) -> Result<Self> {
        let encoder = ts
            .encoder_for()
            .ok_or_else(|| Error::UnsupportedTransferSyntax)?;
        let text = cs.codec().ok_or_else(|| Error::UnsupportedCharacterSet)?;
        Ok(DataSetWriter::new(to, encoder, text))
    }
}

impl<W, E, T> DataSetWriter<W, E, T> {
    pub fn new(to: W, encoder: E, text: T) -> Self {
        DataSetWriter {
            to,
            encoder,
            text,
            seq_tokens: Vec::new(),
        }
    }
}

impl<W, E, T> DataSetWriter<W, E, T>
where
    W: Write,
    E: Encode,
    T: TextCodec,
{
    /// Feed the given sequence of tokens which are part of the same data set.
    pub fn write_sequence<I>(&mut self, tokens: I) -> Result<()>
    where
        I: IntoIterator<Item = DataToken>,
    {
        for token in tokens {
            self.write(token)?;
        }

        Ok(())
    }

    /// Feed the given data set token for writing the data set.
    #[inline]
    pub fn write(&mut self, token: DataToken) -> Result<()> {
        // TODO adjust the logic of sequence printing:
        // explicit length sequences or items should not print
        // the respective delimiter

        match token {
            DataToken::SequenceStart { tag: _, len } => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Sequence,
                    len,
                });
                self.write_stateless(token)?;
                Ok(())
            }
            DataToken::ItemStart { len } => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Item,
                    len,
                });
                self.write_stateless(token)?;
                Ok(())
            }
            DataToken::ItemEnd => {
                // only write if it's an unknown length item
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Item && seq_start.len.is_undefined() {
                        self.write_stateless(token)?;
                    }
                }
                Ok(())
            }
            DataToken::SequenceEnd => {
                // only write if it's an unknown length sequence
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Sequence && seq_start.len.is_undefined() {
                        self.write_stateless(token)?;
                    }
                }
                Ok(())
            }
            _ => self.write_stateless(token),
        }
    }

    fn write_stateless(&mut self, token: DataToken) -> Result<()> {
        use DataToken::*;
        match token {
            ElementHeader(header) => {
                self.encoder.encode_element_header(&mut self.to, header)?;
            }
            SequenceStart { tag, len } => {
                self.encoder.encode_element_header(
                    &mut self.to,
                    DataElementHeader::new(tag, VR::SQ, len),
                )?;
            }
            SequenceEnd => {
                self.encoder.encode_sequence_delimiter(&mut self.to)?;
            }
            ItemStart { len } => {
                self.encoder.encode_item_header(&mut self.to, len.0)?;
            }
            ItemEnd => {
                self.encoder.encode_item_delimiter(&mut self.to)?;
            }
            PrimitiveValue(value) => {
                // TODO handle strings properly
                self.encoder.encode_primitive(&mut self.to, &value)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::DataToken;
    use super::DataSetWriter;
    use crate::printer::Printer;
    use dicom_core::header::{DataElementHeader, Length};
    use dicom_core::value::PrimitiveValue;
    use dicom_encoding::text::DefaultCharacterSetCodec;
    use dicom_encoding::transfer_syntax::explicit_le::ExplicitVRLittleEndianEncoder;

    fn validate_dataset_writer<I>(tokens: I, ground_truth: &[u8])
    where
        I: IntoIterator<Item = DataToken>,
    {
        let mut raw_out: Vec<u8> = vec![];
        let encoder = ExplicitVRLittleEndianEncoder::default();
        let text = DefaultCharacterSetCodec::default();
        let mut dset_writer = DataSetWriter::new(&mut raw_out, encoder, text);

        //let mut iter = Iterator::zip(dset_writer.by_ref(), ground_truth);
    }
}
