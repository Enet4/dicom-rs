//! Data structures and algorithms for DICOM data set record tables.
//!
//! A complete table of element records
//! (with some meta-information and byte positions)
//! can be obtained from a parser
//! by creating a [`DataSetTableBuilder`]
//! and invoking [`update`] on each token.
//! 
//! [`update`]: DataSetTableBuilder::update
//!

use std::{collections::BTreeMap, iter::FromIterator};

use dicom_core::{value::C, DataDictionary, DataElementHeader, Length, Tag};
use dicom_parser::{
    dataset::{lazy_read::LazyDataSetReader, LazyDataToken},
    StatefulDecode,
};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DataSetTable {
    table: BTreeMap<Tag, DataSetRecord>,
}

impl FromIterator<DataSetRecord> for DataSetTable {
    fn from_iter<T: IntoIterator<Item = DataSetRecord>>(iter: T) -> Self {
        DataSetTable {
            table: iter
                .into_iter()
                .map(|record| (record.tag(), record))
                .collect(),
        }
    }
}

impl DataSetTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn by_tag(&self, tag: Tag) -> Option<&DataSetRecord> {
        self.table.get(&tag)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DataSetTableBuilder {
    records: Vec<DataSetRecordBuilder>,
    /// current amount of data set nesting.
    /// 0 means push new elements to `table`,
    /// 1 or more means push them to last record at the given depth
    depth: u32,
    last_header: Option<DataElementHeader>,
}

impl DataSetTableBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update<D>(&mut self, token: &LazyDataToken<D>)
    where
        D: StatefulDecode,
    {
        match token {
            LazyDataToken::ElementHeader(..) => {
                // no-op
            }
            LazyDataToken::LazyValue { header, decoder } => {
                // record element header and position into table
                let records = self.records_at(self.depth);
                records.push(DataSetRecordBuilder::Element {
                    header: *header,
                    position: decoder.position(),
                })
            }
            LazyDataToken::SequenceStart { tag, len } => {
                // add depth, create empty sequence record
                let records = self.records_at(self.depth);
                records.push(DataSetRecordBuilder::Sequence {
                    tag: *tag,
                    length: *len,
                    items: vec![],
                });
                self.depth += 1;
            }
            LazyDataToken::ItemStart { len } => {
                // create new item at record
                match self.last_record_at(self.depth) {
                    DataSetRecordBuilder::Sequence { items, .. } => {
                        items.push(Default::default());
                    }
                    DataSetRecordBuilder::PixelSequence { fragment_positions } => {
                        // record position if length is 0
                        // (because then we have no LazyItemValue
                        // and the position must be recorded anyway)
                        if *len == Length(0) {
                            // Note: because the position cannot be identified from here,
                            // we place an arbitrary value with the assumption
                            // that the zero length will be checked beforehand
                            // and that no read is actually attempted.
                            fragment_positions.push(None);
                        }
                    }
                    _ => unreachable!("Unexpected record type"),
                }
            }
            LazyDataToken::SequenceEnd => {
                // remove depth
                self.depth -= 1;
            }
            LazyDataToken::PixelSequenceStart => {
                // create new empty pixel sequence record
                let records = self.records_at(self.depth);
                records.push(DataSetRecordBuilder::PixelSequence {
                    fragment_positions: Default::default(),
                });
                self.depth += 1;
            }
            LazyDataToken::LazyItemValue { len: _, decoder } => {
                // update pixel sequence record
                match self.last_record_at(self.depth) {
                    DataSetRecordBuilder::PixelSequence { fragment_positions } => {
                        // record and push position
                        fragment_positions.push(Some(decoder.position()));
                    }
                    _ => unreachable!("Unexpected record type"),
                }
            }
            LazyDataToken::ItemEnd => {
                // no-op
            }
            _ => unreachable!("unsupported token variant"),
        }
    }

    pub fn build(self) -> DataSetTable {
        DataSetTable::from_iter(self.records.into_iter().map(DataSetRecordBuilder::build))
    }

    fn records_at(&mut self, depth: u32) -> &mut Vec<DataSetRecordBuilder> {
        let mut records = &mut self.records;

        for i in 0..depth {
            // go in self.depth times
            if let Some(DataSetRecordBuilder::Sequence { items, .. }) = records.last_mut() {
                if let Some(item) = items.last_mut() {
                    records = &mut item.records;
                } else {
                    unreachable!("last record at depth {} does not have any items", i);
                }
            } else {
                unreachable!("last record at depth {} is not a sequence", i);
            }
        }
        records
    }

    fn last_record_at(&mut self, depth: u32) -> &mut DataSetRecordBuilder {
        let mut records = &mut self.records;

        for _ in 1..depth {
            match records.last_mut().expect("missing record") {
                DataSetRecordBuilder::Sequence { items, .. } => {
                    let item = items.last_mut().unwrap();
                    records = &mut item.records;
                }
                _ => unreachable!(),
            }
        }

        records.last_mut().expect("missing last record")
    }
}

/// A record of value positions on a persisted DICOM data set.
#[derive(Debug, Clone, PartialEq)]
pub enum DataSetRecord {
    /// Primitive data element
    Element {
        /// data element header
        header: DataElementHeader,
        /// the byte position of the value
        position: u64,
    },
    /// Data element sequence
    Sequence {
        /// sequence element tag
        tag: Tag,
        /// the length according to the persisted data set
        length: Length,
        items: Vec<DataSetTable>,
    },
    /// Encapsulated pixel sequence
    PixelSequence {
        /// the byte positions of each fragment in order
        /// (the first fragment is the offset table),
        /// `None` if the fragment is empty
        fragment_positions: C<Option<u64>>,
    },
}

impl DataSetRecord {
    pub fn tag(&self) -> Tag {
        match self {
            DataSetRecord::Element { header, .. } => header.tag,
            DataSetRecord::Sequence { tag, .. } => *tag,
            DataSetRecord::PixelSequence { .. } => Tag(0x7FE0, 0x0010),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataSetRecordBuilder {
    /// Primitive data element
    Element {
        /// data element header
        header: DataElementHeader,
        /// the byte position of the value
        position: u64,
    },
    /// Data element sequence
    Sequence {
        /// sequence element tag
        tag: Tag,
        /// the length according to the persisted data set
        length: Length,
        items: Vec<DataSetTableBuilder>,
    },
    /// Encapsulated pixel sequence
    PixelSequence {
        /// the byte positions of each fragment in order,
        /// `None` if the fragment is empty.
        fragment_positions: C<Option<u64>>,
    },
}

impl DataSetRecordBuilder {
    pub fn build(self) -> DataSetRecord {
        match self {
            DataSetRecordBuilder::Element { header, position } => {
                DataSetRecord::Element { header, position }
            }
            DataSetRecordBuilder::Sequence { tag, length, items } => DataSetRecord::Sequence {
                tag,
                length,
                items: items.into_iter().map(DataSetTableBuilder::build).collect(),
            },
            DataSetRecordBuilder::PixelSequence { fragment_positions } => {
                DataSetRecord::PixelSequence { fragment_positions }
            }
        }
    }
}

/// A lazy data set reader which updates a data set table builder
/// as it fetches new tokens.
///
/// It still uses [`LazyDataSetReader`][1] as its underlying implementation.
///
/// [1]: dicom_parser::dataset::lazy_read::LazyDataSetReader
#[derive(Debug)]
pub struct RecordBuildingDataSetReader<'a, S, D> {
    builder: &'a mut DataSetTableBuilder,
    reader: LazyDataSetReader<S, D>,
}

impl<'a, S, D> RecordBuildingDataSetReader<'a, S, D>
where
    S: StatefulDecode,
    D: DataDictionary,
{
    pub fn new(reader: LazyDataSetReader<S, D>, builder: &'a mut DataSetTableBuilder) -> Self {
        RecordBuildingDataSetReader { builder, reader }
    }

    pub fn into_inner(self) -> LazyDataSetReader<S, D> {
        self.reader
    }

    /** Advance and retrieve the next DICOM data token.
     *
     * If a token is obtained,
     * the referenced builder is automatically updated.
     *
     * **Note:** For the data set to be successfully parsed,
     * the resulting data tokens needs to be consumed
     * if they are of a value type.
     */
    pub fn next(
        &mut self,
    ) -> Option<dicom_parser::dataset::lazy_read::Result<LazyDataToken<&mut S>>> {
        match self.reader.next() {
            Some(Ok(token)) => {
                self.builder.update(&token);
                Some(Ok(token))
            }
            e @ Some(Err(_)) => e,
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use dicom_core::{DataElementHeader, Length, Tag, VR};
    use dicom_encoding::{
        decode::{basic::LittleEndianBasicDecoder, explicit_le::ExplicitVRLittleEndianDecoder},
        text::DefaultCharacterSetCodec,
    };
    use dicom_parser::{dataset::lazy_read::LazyDataSetReader, StatefulDecoder};

    use crate::lazy::record::{DataSetRecord, DataSetTable};

    use super::DataSetTableBuilder;

    fn validate_create_table_explicit_vr<R>(source: R, gt: &DataSetTable)
    where
        R: Read,
    {
        let stateful_decoder = StatefulDecoder::new(
            source,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder::default(),
            Box::new(DefaultCharacterSetCodec::default()) as Box<_>,
        );

        let mut dataset_reader = LazyDataSetReader::new(stateful_decoder);

        let mut b = DataSetTableBuilder::new();

        while let Some(token) = dataset_reader.next() {
            let token = token.unwrap();
            b.update(&token);
            token.skip().unwrap();
        }

        let table = b.build();

        assert_eq!(&table, gt);
    }

    #[test]
    fn lazy_record_from_sequence_explicit() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
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

        let sequence_record: DataSetRecord = DataSetRecord::Sequence {
            tag: Tag(0x0018, 0x6011),
            length: Length(46),
            items: vec![
                vec![
                    DataSetRecord::Element {
                        header: DataElementHeader {
                            tag: Tag(0x0018, 0x6012),
                            vr: VR::US,
                            len: Length(2),
                        },
                        position: 28,
                    },
                    DataSetRecord::Element {
                        header: DataElementHeader {
                            tag: Tag(0x0018, 0x6014),
                            vr: VR::US,
                            len: Length(2),
                        },
                        position: 38,
                    },
                ]
                .into_iter()
                .collect(),
                vec![DataSetRecord::Element {
                    header: DataElementHeader {
                        tag: Tag(0x0018, 0x6012),
                        vr: VR::US,
                        len: Length(2),
                    },
                    position: 56,
                }]
                .into_iter()
                .collect(),
            ],
        };

        let ground_truth: DataSetTable = vec![
            sequence_record,
            DataSetRecord::Element {
                header: DataElementHeader {
                    tag: Tag(0x0020, 0x4000),
                    vr: VR::LT,
                    len: Length(4),
                },
                position: 66,
            },
        ]
        .into_iter()
        .collect();

        validate_create_table_explicit_vr(DATA, &ground_truth);
    }

    #[test]
    fn lazy_record_from_encapsulated_pixel_data() {
        #[rustfmt::skip]
        static DATA: &[u8] = &[
            0xe0, 0x7f, 0x10, 0x00, // (7FE0, 0010) PixelData
            b'O', b'B', // VR 
            0x00, 0x00, // reserved
            0xff, 0xff, 0xff, 0xff, // length: undefined
            // -- 12 -- Pixel Item 0: empty offset table
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x00, 0x00, 0x00, 0x00, // item length: 0
            // -- 20 -- First fragment of pixel data
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x20, 0x00, 0x00, 0x00, // item length: 32
            // -- 28 -- Pixel Item 1: Compressed Fragment
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            // -- 60 -- Second fragment of pixel data
            0xfe, 0xff, 0x00, 0xe0, // item start tag
            0x10, 0x00, 0x00, 0x00, // item length: 16
            // -- 68 -- Pixel Item 2: Compressed Fragment
            0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB,
            0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB,
            // -- 84 -- End of pixel data
            0xfe, 0xff, 0xdd, 0xe0, // sequence end tag
            0x00, 0x00, 0x00, 0x00,
            // -- 92  -- padding
            0xfc, 0xff, 0xfc, 0xff, // (fffc,fffc) DataSetTrailingPadding
            b'O', b'B', // VR
            0x00, 0x00, // reserved
            0x08, 0x00, 0x00, 0x00, // length: 8
            // -- 104 --
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let ground_truth = vec![
            DataSetRecord::PixelSequence {
                fragment_positions: smallvec::smallvec![None, Some(28), Some(68)],
            },
            DataSetRecord::Element {
                header: DataElementHeader::new(Tag(0xFFFC, 0xFFFC), VR::OB, Length(8)),
                position: 104,
            },
        ]
        .into_iter()
        .collect();

        validate_create_table_explicit_vr(DATA, &ground_truth);
    }
}
