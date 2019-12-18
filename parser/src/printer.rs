use crate::dataset::*;
use dicom_core::{DataElementHeader, Length, VR};
use dicom_encoding::encode::Encode;
use dicom_encoding::error::Result;
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
pub struct DatasetWriter<E> {
    encoder: E,
    seq_tokens: Vec<SeqToken>,
}

impl<E> DatasetWriter<E>
where
    E: Encode,
{
    /// Feed the given sequence of tokens which are part of the same data set.
    pub fn write_sequence<W, I>(&mut self, mut to: W, tokens: I) -> Result<()>
    where
        I: IntoIterator<Item = DataToken>,
        W: Write,
    {
        for token in tokens {
            self.write(&mut to, token)?;
        }

        Ok(())
    }

    /// Feed the given data set token for writing the data set.
    #[inline]
    pub fn write<W>(&mut self, to: W, token: DataToken) -> Result<()>
    where
        W: Write,
    {
        // TODO adjust the logic of sequence printing:
        // explicit length sequences or items should not print
        // the respective delimiter

        match token {
            DataToken::SequenceStart { tag: _, len } => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Sequence,
                    len,
                });
                self.write_stateless(to, token)?;
                Ok(())
            }
            DataToken::ItemStart { len } => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Item,
                    len,
                });
                self.write_stateless(to, token)?;
                Ok(())
            }
            DataToken::ItemEnd => {
                // only write if it's an unknown length item
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Item && seq_start.len.is_undefined() {
                        self.write_stateless(to, token)?;
                    }
                }
                Ok(())
            }
            DataToken::SequenceEnd => {
                // only write if it's an unknown length sequence
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Sequence && seq_start.len.is_undefined() {
                        self.write_stateless(to, token)?;
                    }
                }
                Ok(())
            }
            _ => self.write_stateless(to, token),
        }
    }

    fn write_stateless<W>(&self, mut to: W, token: DataToken) -> Result<()>
    where
        W: Write,
    {
        use DataToken::*;
        match token {
            ElementHeader(header) => {
                self.encoder.encode_element_header(&mut to, header)?;
            }
            SequenceStart { tag, len } => {
                self.encoder
                    .encode_element_header(&mut to, DataElementHeader::new(tag, VR::SQ, len))?;
            }
            SequenceEnd => {
                self.encoder.encode_sequence_delimiter(&mut to)?;
            }
            ItemStart { len } => {
                self.encoder.encode_item_header(&mut to, len.0)?;
            }
            ItemEnd => {
                self.encoder.encode_item_delimiter(&mut to)?;
            }
            PrimitiveValue(value) => {
                self.encoder.encode_primitive(&mut to, &value)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    
}