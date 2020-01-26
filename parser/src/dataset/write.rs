//! Module for the data set reader
use crate::dataset::*;
use crate::error::{DataSetSyntaxError, Error, Result};
use crate::stateful::encode::StatefulEncoder;
use dicom_core::{DataElementHeader, Length, VR};
use dicom_encoding::encode::{Encode, EncodeTo};
use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
use dicom_encoding::TransferSyntax;
use std::io::Write;

/// A writer-specific token representing a sequence or item start.
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
    printer: StatefulEncoder<W, E, T>,
    seq_tokens: Vec<SeqToken>,
    last_de: Option<DataElementHeader>,
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
            printer: StatefulEncoder::new(to, encoder, text),
            seq_tokens: Vec::new(),
            last_de: None,
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
        // adjust the logic of sequence printing:
        // explicit length sequences or items should not print
        // the respective delimiter

        match token {
            DataToken::SequenceStart { len, .. } => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Sequence,
                    len,
                });
                self.write_impl(token)?;
                Ok(())
            }
            DataToken::ItemStart { len } => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Item,
                    len,
                });
                self.write_impl(token)?;
                Ok(())
            }
            DataToken::ItemEnd => {
                // only write if it's an unknown length item
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Item && seq_start.len.is_undefined() {
                        self.write_impl(token)?;
                    }
                }
                Ok(())
            }
            DataToken::SequenceEnd => {
                // only write if it's an unknown length sequence
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Sequence && seq_start.len.is_undefined() {
                        self.write_impl(token)?;
                    }
                }
                Ok(())
            }
            DataToken::ElementHeader(de) => {
                self.last_de = Some(de.clone());
                self.write_impl(token)
            }
            _ => self.write_impl(token),
        }
    }

    fn write_impl(&mut self, token: DataToken) -> Result<()> {
        use DataToken::*;
        match token {
            ElementHeader(header) => {
                self.printer.encode_element_header(header)?;
            }
            SequenceStart { tag, len } => {
                self.printer
                    .encode_element_header(DataElementHeader::new(tag, VR::SQ, len))?;
            }
            SequenceEnd => {
                self.printer.encode_sequence_delimiter()?;
            }
            ItemStart { len } => {
                self.printer.encode_item_header(len.0)?;
            }
            ItemEnd => {
                self.printer.encode_item_delimiter()?;
            }
            PrimitiveValue(ref value) => {
                let last_de = self
                    .last_de
                    .as_ref()
                    .ok_or_else(|| DataSetSyntaxError::UnexpectedToken(token.clone()))?;
                self.printer.encode_primitive(last_de, value)?;
                self.last_de = None;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::DataToken;
    use super::DataSetWriter;
    use dicom_core::{
        header::{DataElementHeader, Length},
        value::PrimitiveValue,
        Tag, VR,
    };
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

        dset_writer.write_sequence(tokens).unwrap();

        assert_eq!(raw_out, ground_truth);
    }

    #[test]
    fn write_sequence_explicit() {
        let tokens = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0018, 0x6011),
                len: Length(46),
            },
            DataToken::ItemStart { len: Length(20) },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([1].as_ref().into())),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6014),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([2].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::ItemStart { len: Length(10) },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([4].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0020, 0x4000),
                vr: VR::LT,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("TEST".into())),
        ];

        #[rustfmt::skip]
        static GROUND_TRUTH: &[u8] = &[
            0x18, 0x00, 0x11, 0x60, // sequence tag: (0018,6011) SequenceOfUltrasoundRegions
            b'S', b'Q', // VR 
            0x00, 0x00, // reserved
            0x2e, 0x00, 0x00, 0x00, // length: 28 + 18 = 46 (#= 2)
            // -- 12 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x14, 0x00, 0x00, 0x00, // item length: 20 (#= 2)
            // -- 20 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x01, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 1
            // -- 30 --
            0x18, 0x00, 0x14, 0x60, b'U', b'S', 0x02, 0x00, 0x02, 0x00, // (0018, 6012) RegionDataType, len = 2, value = 2
            // -- 40 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x0a, 0x00, 0x00, 0x00, // item length: 10 (#= 1)
            // -- 48 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x04, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 4
            // -- 58 --
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04, 0x00, // (0020,4000) ImageComments, len = 4  
            b'T', b'E', b'S', b'T', // value = "TEST"
        ];

        validate_dataset_writer(tokens, GROUND_TRUTH);
    }

    #[test]
    fn write_sequence_implicit() {
        let tokens = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0018, 0x6011),
                len: Length::UNDEFINED,
            },
            DataToken::ItemStart {
                len: Length::UNDEFINED,
            },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([1].as_ref().into())),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6014),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([2].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::ItemStart {
                len: Length::UNDEFINED,
            },
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0018, 0x6012),
                vr: VR::US,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::U16([4].as_ref().into())),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0020, 0x4000),
                vr: VR::LT,
                len: Length(4),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("TEST".into())),
        ];

        #[rustfmt::skip]
        static GROUND_TRUTH: &[u8] = &[
            0x18, 0x00, 0x11, 0x60, // sequence tag: (0018,6011) SequenceOfUltrasoundRegions
            b'S', b'Q', // VR 
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0xff, 0xff, 0xff, 0xff, // item length: undefined
            // -- 20 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x01, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 1
            // -- 30 --
            0x18, 0x00, 0x14, 0x60, b'U', b'S', 0x02, 0x00, 0x02, 0x00, // (0018, 6012) RegionDataType, len = 2, value = 2
            // -- 40 --
            0xfe, 0xff, 0x0d, 0xe0, 0x00, 0x00, 0x00, 0x00, // item end
            // -- 48 --
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0xff, 0xff, 0xff, 0xff, // item length: undefined
            // -- 56 --
            0x18, 0x00, 0x12, 0x60, b'U', b'S', 0x02, 0x00, 0x04, 0x00, // (0018, 6012) RegionSpatialformat, len = 2, value = 4
            // -- 66 --
            0xfe, 0xff, 0x0d, 0xe0, 0x00, 0x00, 0x00, 0x00, // item end
            // -- 74 --
            0xfe, 0xff, 0xdd, 0xe0, 0x00, 0x00, 0x00, 0x00, // sequence end
            // -- 82 --
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04, 0x00, // (0020,4000) ImageComments, len = 4  
            b'T', b'E', b'S', b'T', // value = "TEST"
        ];

        validate_dataset_writer(tokens, GROUND_TRUTH);
    }
}
