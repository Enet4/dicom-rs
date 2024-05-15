//! Module for the data set writer
//!
//! This module contains a mid-level abstraction for printing DICOM data sets
//! sequentially.
//! The [`DataSetWriter`] receieves data tokens to be encoded and written
//! to a writer.
//! In this process, the writer will also adapt values
//! to the necessary DICOM encoding rules.
use crate::dataset::{DataToken, SeqTokenType};
use crate::stateful::encode::StatefulEncoder;
use dicom_core::{DataElementHeader, Length, Tag, VR};
use dicom_encoding::encode::EncodeTo;
use dicom_encoding::text::SpecificCharacterSet;
use dicom_encoding::transfer_syntax::DynEncoder;
use dicom_encoding::TransferSyntax;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::io::Write;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// Unsupported transfer syntax for encoding
    #[snafu(display("Unsupported transfer syntax {} ({})", ts_uid, ts_alias))]
    UnsupportedTransferSyntax {
        ts_uid: &'static str,
        ts_alias: &'static str,
        backtrace: Backtrace,
    },
    /// Character set known, but not supported
    #[snafu(display("Unsupported character set {:?}", charset))]
    UnsupportedCharacterSet {
        charset: SpecificCharacterSet,
        backtrace: Backtrace,
    },
    /// An element value token appeared without an introducing element header
    #[snafu(display("Unexpected token {:?} without element header", token))]
    UnexpectedToken {
        token: DataToken,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not write element header tagged {}", tag))]
    WriteHeader {
        tag: Tag,
        #[snafu(backtrace)]
        source: crate::stateful::encode::Error,
    },
    #[snafu(display("Could not write item header"))]
    WriteItemHeader {
        #[snafu(backtrace)]
        source: crate::stateful::encode::Error,
    },

    #[snafu(display("Could not write sequence delimiter"))]
    WriteSequenceDelimiter {
        #[snafu(backtrace)]
        source: crate::stateful::encode::Error,
    },

    #[snafu(display("Could not write item delimiter"))]
    WriteItemDelimiter {
        #[snafu(backtrace)]
        source: crate::stateful::encode::Error,
    },

    #[snafu(display("Could not write element value"))]
    WriteValue {
        #[snafu(backtrace)]
        source: crate::stateful::encode::Error,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

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
pub struct DataSetWriter<W, E, T = SpecificCharacterSet> {
    printer: StatefulEncoder<W, E, T>,
    seq_tokens: Vec<SeqToken>,
    last_de: Option<DataElementHeader>,
}

impl<'w, W: 'w> DataSetWriter<W, DynEncoder<'w, W>>
where
    W: Write,
{
    /// Create a new data set writer
    /// with the given transfer syntax specifier.
    pub fn with_ts(to: W, ts: &TransferSyntax) -> Result<Self> {
        let encoder = ts.encoder_for().context(UnsupportedTransferSyntaxSnafu {
            ts_uid: ts.uid(),
            ts_alias: ts.name(),
        })?;
        Ok(DataSetWriter::new_with_codec(
            to,
            encoder,
            SpecificCharacterSet::default(),
        ))
    }

    /// Create a new data set writer
    /// with the given transfer syntax specifier
    /// and the specific character set to assume by default.
    ///
    /// Note that the data set being written
    /// can override the character set with the presence of a
    /// _Specific Character Set_ data element.
    pub fn with_ts_cs(to: W, ts: &TransferSyntax, charset: SpecificCharacterSet) -> Result<Self> {
        let encoder = ts.encoder_for().context(UnsupportedTransferSyntaxSnafu {
            ts_uid: ts.uid(),
            ts_alias: ts.name(),
        })?;
        Ok(DataSetWriter::new_with_codec(to, encoder, charset))
    }
}

impl<W, E> DataSetWriter<W, E> {
    pub fn new(to: W, encoder: E) -> Self {
        DataSetWriter {
            printer: StatefulEncoder::new(to, encoder, SpecificCharacterSet::default()),
            seq_tokens: Vec::new(),
            last_de: None,
        }
    }
}

impl<W, E, T> DataSetWriter<W, E, T> {
    pub fn new_with_codec(to: W, encoder: E, text: T) -> Self {
        DataSetWriter {
            printer: StatefulEncoder::new(to, encoder, text),
            seq_tokens: Vec::new(),
            last_de: None,
        }
    }
}

impl<W, E> DataSetWriter<W, E>
where
    W: Write,
    E: EncodeTo<W>,
{
    /// Feed the given sequence of tokens which are part of the same data set.
    #[inline]
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
                self.write_impl(&token)?;
                Ok(())
            }
            DataToken::ItemStart { len } => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Item,
                    len,
                });
                self.write_impl(&token)?;
                Ok(())
            }
            DataToken::ItemEnd => {
                // only write if it's an unknown length item
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Item && seq_start.len.is_undefined() {
                        self.write_impl(&token)?;
                    }
                }
                Ok(())
            }
            DataToken::SequenceEnd => {
                // only write if it's an unknown length sequence
                if let Some(seq_start) = self.seq_tokens.pop() {
                    if seq_start.typ == SeqTokenType::Sequence && seq_start.len.is_undefined() {
                        self.write_impl(&token)?;
                    }
                }
                Ok(())
            }
            DataToken::ElementHeader(de) => {
                // save the header for later
                self.last_de = Some(de);

                // postpone writing the header until the value token is given
                Ok(())
            }
            token @ DataToken::PixelSequenceStart => {
                self.seq_tokens.push(SeqToken {
                    typ: SeqTokenType::Sequence,
                    len: Length::UNDEFINED,
                });
                self.write_impl(&token)
            }
            token @ DataToken::ItemValue(_)
            | token @ DataToken::PrimitiveValue(_)
            | token @ DataToken::OffsetTable(_) => self.write_impl(&token),
        }
    }

    fn write_impl(&mut self, token: &DataToken) -> Result<()> {
        match token {
            DataToken::ElementHeader(header) => {
                self.printer
                    .encode_element_header(*header)
                    .context(WriteHeaderSnafu { tag: header.tag })?;
            }
            DataToken::SequenceStart { tag, len } => {
                self.printer
                    .encode_element_header(DataElementHeader::new(*tag, VR::SQ, *len))
                    .context(WriteHeaderSnafu { tag: *tag })?;
            }
            DataToken::PixelSequenceStart => {
                let tag = Tag(0x7fe0, 0x0010);
                self.printer
                    .encode_element_header(DataElementHeader::new(tag, VR::OB, Length::UNDEFINED))
                    .context(WriteHeaderSnafu { tag })?;
            }
            DataToken::SequenceEnd => {
                self.printer
                    .encode_sequence_delimiter()
                    .context(WriteSequenceDelimiterSnafu)?;
            }
            DataToken::ItemStart { len } => {
                self.printer
                    .encode_item_header(len.0)
                    .context(WriteItemHeaderSnafu)?;
            }
            DataToken::ItemEnd => {
                self.printer
                    .encode_item_delimiter()
                    .context(WriteItemDelimiterSnafu)?;
            }
            DataToken::PrimitiveValue(ref value) => {
                let last_de = self.last_de.take().with_context(|| UnexpectedTokenSnafu {
                    token: token.clone(),
                })?;

                self.printer
                    .encode_primitive_element(&last_de, value)
                    .context(WriteValueSnafu)?;
                self.last_de = None;
            }
            DataToken::OffsetTable(table) => {
                self.printer
                    .encode_offset_table(table)
                    .context(WriteValueSnafu)?;
            }
            DataToken::ItemValue(data) => {
                self.printer.write_bytes(data).context(WriteValueSnafu)?;
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
    use dicom_encoding::encode::{explicit_le::ExplicitVRLittleEndianEncoder, EncoderFor};

    fn validate_dataset_writer<I>(tokens: I, ground_truth: &[u8])
    where
        I: IntoIterator<Item = DataToken>,
    {
        let mut raw_out: Vec<u8> = vec![];
        let encoder = EncoderFor::new(ExplicitVRLittleEndianEncoder::default());
        let mut dset_writer = DataSetWriter::new(&mut raw_out, encoder);

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
    fn write_element_overrides_len() {
        let tokens = vec![
            DataToken::ElementHeader(DataElementHeader {
                // Specific Character Set (0008,0005)
                tag: Tag(0x0008, 0x0005),
                vr: VR::CS,
                len: Length(10),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::from("ISO_IR 100")),
            DataToken::ElementHeader(DataElementHeader {
                // Referring Physician's Name (0008,0090)
                tag: Tag(0x0008, 0x0090),
                vr: VR::PN,
                // deliberately incorrect length
                len: Length("Simões^João".len() as u32),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::from("Simões^João")),
        ];

        #[rustfmt::skip]
        static GROUND_TRUTH: &[u8] = &[
            // Specific Character Set (0008,0005)
            0x08, 0x00, 0x05, 0x00, //
            b'C', b'S', // VR
            0x0a, 0x00, // length: 10
            b'I', b'S', b'O', b'_', b'I', b'R', b' ', b'1', b'0', b'0', // value = "ISO_IR 100"
            // Referring Physician's Name (0008,0090)
            0x08, 0x00, 0x90, 0x00, //
            b'P', b'N', // VR
            0x0c, 0x00, // length: 12
            // value = "Simões^João "
            b'S', b'i', b'm', 0xF5, b'e', b's', b'^', b'J', b'o', 0xE3, b'o', b' '
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

    #[test]
    fn write_sequence_explicit_with_implicit_item_len() {
        let tokens = vec![
            DataToken::SequenceStart {
                tag: Tag(0x0018, 0x6011),
                len: Length(60),
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
            0x3c, 0x00, 0x00, 0x00, // length: 60
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
            0x20, 0x00, 0x00, 0x40, b'L', b'T', 0x04, 0x00, // (0020,4000) ImageComments, len = 4  
            b'T', b'E', b'S', b'T', // value = "TEST"
        ];

        validate_dataset_writer(tokens, GROUND_TRUTH);
    }

    #[test]
    fn write_encapsulated_pixeldata() {
        let tokens = vec![
            DataToken::PixelSequenceStart,
            DataToken::ItemStart { len: Length(0) },
            DataToken::ItemEnd,
            DataToken::ItemStart { len: Length(32) },
            DataToken::ItemValue(vec![0x99; 32]),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
            DataToken::ElementHeader(DataElementHeader::new(
                Tag(0xfffc, 0xfffc),
                VR::OB,
                Length(8),
            )),
            DataToken::PrimitiveValue(PrimitiveValue::U8([0x00; 8].as_ref().into())),
        ];

        #[rustfmt::skip]
        static GROUND_TRUTH: &[u8] = &[
            0xe0, 0x7f, 0x10, 0x00, // (7FE0, 0010) PixelData
            b'O', b'B', // VR 
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 -- Basic offset table
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x00, 0x00, 0x00, 0x00, // item length: 0
            // -- 20 -- First fragment of pixel data
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x20, 0x00, 0x00, 0x00, // item length: 32
            // -- 28 -- Compressed Fragment
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            // -- 60 -- End of pixel data
            0xfe, 0xff, 0xdd, 0xe0, // sequence end tag
            0x00, 0x00, 0x00, 0x00,
            // -- 68 -- padding
            0xfc, 0xff, 0xfc, 0xff, // (fffc,fffc) DataSetTrailingPadding
            b'O', b'B', // VR
            0x00, 0x00, // reserved
            0x08, 0x00, 0x00, 0x00, // length: 8
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        validate_dataset_writer(tokens, GROUND_TRUTH);
    }
}
