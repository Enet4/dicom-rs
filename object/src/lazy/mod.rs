//! This module contains the implementation for a lazily evaluated DICOM object.
//!
//! In a lazy DICOM object, larger DICOM elements
//! may be skipped during the decoding process,
//! and thus not be immediately available in memory.
//! A pointer to the original data source is kept for future access,
//! so that the element is fetched and its value is decoded on demand.

use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use smallvec::SmallVec;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::{collections::BTreeMap, io::Seek, io::SeekFrom};

use crate::DicomObject;
use crate::lazy::record::{DataSetRecord, DataSetRecordBuilder, DataSetTableBuilder};
use crate::{meta::FileMetaTable, util::ReadSeek, FileDicomObject};
use dicom_core::header::{HasLength, Header};
use dicom_core::value::{Value, C};
use dicom_core::{
    dictionary::{DataDictionary, DictionaryEntry},
    DataElementHeader, DicomValue,
};
use dicom_core::{DataElement, Length, Tag, VR};
use dicom_encoding::text::{SpecificCharacterSet, TextCodec};
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_parser::{
    dataset::lazy_read::LazyDataSetReader, stateful::decode::Error as StatefulDecodeError,
};
use dicom_parser::{dataset::read::Error as ParserError, StatefulDecode};
use dicom_parser::{
    dataset::{DataToken, LazyDataToken},
    DynStatefulDecoder,
};
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};

pub use self::element::{LazyElement, LazyNestedObject, MaybeValue};
use self::record::{DataSetTable, RecordBuildingDataSetReader};

pub(crate) mod element;
pub mod record;

/// The type of a pixel data fragment.
pub type InMemFragment = Vec<u8>;

type ParserResult<T> = std::result::Result<T, ParserError>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not open file '{}'", filename.display()))]
    OpenFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not read from file '{}'", filename.display()))]
    ReadFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    /// Could not parse meta group data set
    ParseMetaDataSet {
        #[snafu(backtrace)]
        source: crate::meta::Error,
    },
    /// Could not create data set parser
    CreateParser {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::lazy_read::Error,
    },
    /// Could not read data set token
    ReadToken {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::lazy_read::Error,
    },
    #[snafu(display("Could not write to file '{}'", filename.display()))]
    WriteFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    /// Could not write object preamble
    WritePreamble {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Unknown data attribute named `{}`", name))]
    NoSuchAttributeName { name: String, backtrace: Backtrace },
    #[snafu(display("Missing element value"))]
    MissingElementValue { backtrace: Backtrace },
    #[snafu(display("Unsupported transfer syntax `{}`", uid))]
    UnsupportedTransferSyntax { uid: String, backtrace: Backtrace },
    /// Could not position data source to value
    PositionToValue { source: StatefulDecodeError },
    /// Could not read value from data source
    ReadValue { source: StatefulDecodeError },
    /// Could not read pixel data offset table
    ReadOffsetTable { source: StatefulDecodeError },
    #[snafu(display("Unexpected token {:?}", token))]
    UnexpectedToken {
        token: dicom_parser::dataset::LazyDataTokenRepr,
        backtrace: Backtrace,
    },
    /// Premature data set end
    PrematureEnd { backtrace: Backtrace },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct OpenFileOptions<D, T> {
    pub dictionary: D,
    pub ts_index: T,
}

/// A DICOM object which fetches elements from a data source on demand.
#[derive(Debug, Clone)]
pub struct LazyDicomObject<S, D> {
    /// the binary source to fetch DICOM data from
    source: S,
    /// the element dictionary at this level
    entries: BTreeMap<Tag, LazyElement<D>>,
    /// the full record table
    records: DataSetTable,
    /// the data element dictionary
    dict: D,
    /// The length of the DICOM object in bytes.
    /// It is usually undefined, unless it is part of an item
    /// in a sequence with a specified length in its item header.
    len: Length,
}

pub type LazyFileDicomObject<S, D> = FileDicomObject<LazyDicomObject<DynStatefulDecoder<S>, D>>;

/*
impl<S, D> LazyFileDicomObject<S, D> {
    /// Load a new lazy DICOM object from a file
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
        D: DataDictionary,
        D: Clone,
        D: Default,
    {
        Self::from_file_with(
            path,
            OpenFileOptions::<_, TransferSyntaxRegistry>::default(),
        )
    }

    /// Load a new lazy DICOM object from a file,
    /// using the given options.
    pub fn from_file_with<P, T>(path: P, options: OpenFileOptions<D, T>) -> Result<Self>
    where
        P: AsRef<Path>,
        T: TransferSyntaxIndex,
        D: DataDictionary,
        D: Clone,
    {
        let OpenFileOptions {
            dictionary,
            ts_index,
        } = options;

        let path = path.as_ref();
        let mut file = File::open(path).with_context(|| OpenFile { filename: path })?;

        // skip preamble
        {
            let mut buf = [0u8; 128];
            // skip the preamble
            file.read_exact(&mut buf)
                .with_context(|| ReadFile { filename: path })?;
        }

        // read metadata header
        let meta = FileMetaTable::from_reader(&mut file).context(ParseMetaDataSet)?;

        // read rest of data according to metadata, feed it to object
        if let Some(ts) = options.ts_index.get(&meta.transfer_syntax) {
            let cs = SpecificCharacterSet::Default;
            let mut dataset =
                LazyDataSetReader::new_with_dictionary(file, dictionary.clone(), ts, cs)
                    .context(CreateParser)?;

            let mut builder = DataSetTableBuilder::new();
            let mut entries = BTreeMap::new();

            let mut dataset = RecordBuildingDataSetReader::new(dataset, &mut builder);

            LazyDicomObject::build_object(
                &mut dataset,
                &mut entries,
                dictionary,
                false,
                Length::UNDEFINED,
            )?;

            Ok(FileDicomObject {
                meta,
                obj: LazyDicomObject {
                    source: dataset.into_inner().into_decoder(),
                    entries,
                    records: builder.build(),
                    dict: dictionary,
                    len: Length::UNDEFINED,
                },
            })
        } else {
            UnsupportedTransferSyntax {
                uid: meta.transfer_syntax,
            }
            .fail()
        }
    }
}

impl<S, D> HasLength for LazyDicomObject<S, D>
where
    S: StatefulDecode,
    <S as StatefulDecode>::Reader: ReadSeek,
    D: DataDictionary,
{
    fn length(&self) -> Length {
        Length::UNDEFINED
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<S, D> LazyDicomObject<S, D>
where
    S: StatefulDecode,
    <S as StatefulDecode>::Reader: ReadSeek,
    D: DataDictionary,
{

    /// Build an object by consuming a data set parser.
    fn build_object(
        dataset: &mut RecordBuildingDataSetReader<S, D>,
        entries: &mut BTreeMap<Tag, LazyElement<D>>,
        dict: D,
        in_item: bool,
        len: Length,
    ) -> Result<()> {
        let mut pixel_sequence_record = None;

        // perform a structured parsing of incoming tokens
        while let Some(token) = dataset.advance() {
            let token = token.context(ReadToken)?;

            let elem = match token {
                LazyDataToken::PixelSequenceStart => {
                    pixel_sequence_record = Some(LazyDicomObject::build_encapsulated_data(&mut *dataset)?);
                    continue;
                }
                LazyDataToken::ElementHeader(header) => {
                    // fetch respective value, place it in the entries
                    let next_token = dataset.advance().context(MissingElementValue)?;
                    match next_token.context(ReadToken)? {
                        t @ LazyDataToken::LazyValue { header, decoder } => LazyElement::new_unloaded(header, decoder.position()),
                        token => {
                            return UnexpectedToken { token }.fail();
                        }
                    }
                }
                LazyDataToken::SequenceStart { tag, len } => {
                    // delegate sequence building to another function
                    let items = Self::build_sequence(tag, len, &mut *dataset, &dict)?;
                    let position = 0;
                    LazyElement::new_loaded(DataElementHeader::new(tag, VR::SQ, len), 0, Value::Sequence { items, size: len })
                }
                LazyDataToken::ItemEnd if in_item => {
                    // end of item, leave now
                    return Ok(());
                }
                token => return UnexpectedToken { token }.fail(),
            };
            entries.insert(elem.header.tag(), elem);
        }

        Ok(())
    }

    /// Construct a lazy record of pixel data fragment positions
    /// and its offset table.
    fn build_encapsulated_data(
        dataset: &mut RecordBuildingDataSetReader<S, D>,
    ) -> Result<PixelSequenceRecord> {
        // continue fetching tokens to retrieve:
        // - the offset table
        // - the positions of the various compressed fragments
        let mut offset_table = None;

        let mut fragment_positions = C::new();

        while let Some(token) = dataset.advance() {
            match token.context(ReadToken)? {
                LazyDataToken::LazyItemValue { len, decoder } => {
                    if offset_table.is_none() {
                        // retrieve the data into the offset table
                        let mut data = Vec::new();
                        decoder.read_to_vec(len, &mut data).context(ReadOffsetTable)?;
                        offset_table = Some(data.into());
                    } else {
                        fragment_positions.push(decoder.position());
                    }
                }
                LazyDataToken::ItemEnd => {
                    // at the end of the first item ensure the presence of
                    // an empty offset_table here, so that the next items
                    // are seen as compressed fragments
                    if offset_table.is_none() {
                        offset_table = Some(C::new())
                    }
                }
                LazyDataToken::ItemStart { len: _ } => { /* no-op */ }
                LazyDataToken::SequenceEnd => {
                    // end of pixel data
                    break;
                }
                // the following variants are unexpected
                token @ LazyDataToken::ElementHeader(_)
                | token @ LazyDataToken::PixelSequenceStart
                | token @ LazyDataToken::SequenceStart { .. }
                | token @ LazyDataToken::LazyValue { .. } => {
                    return UnexpectedToken { token }.fail();
                }
            }
        }

        Ok(PixelSequenceRecord {
            offset_table: offset_table.unwrap_or_default(),
            fragment_positions,
        })
    }

    /// Build a DICOM sequence by consuming a data set parser.
    fn build_sequence<I: ?Sized>(
        _tag: Tag,
        _len: Length,
        dataset: &mut I,
        dict: &D,
    ) -> Result<C<LazyDicomObject<S, D>>>
    where
        I: Iterator<Item = ParserResult<DataToken>>,
    {
        let mut items: C<_> = SmallVec::new();
        while let Some(token) = dataset.next() {
            match token.context(ReadToken)? {
                DataToken::ItemStart { len } => {
                    items.push(Self::build_nested_object(
                        &mut *dataset,
                        *dict.clone(),
                        true,
                        len,
                    )?);
                }
                DataToken::SequenceEnd => {
                    return Ok(items);
                }
                token => return UnexpectedToken { token }.fail(),
            };
        }

        // iterator fully consumed without a sequence delimiter
        PrematureEnd.fail()
    }

    /// Build a nested object by consuming a data set parser.
    fn build_nested_object(
        dataset: &mut LazyDataSetReader<S, D>,
        dict: D,
        in_item: bool,
        len: Length,
    ) -> Result<LazyNestedObject> {
        let mut entries: BTreeMap<Tag, LazyElement<D>> = BTreeMap::new();
        // perform a structured parsing of incoming tokens
        while let Some(token) = dataset.advance() {
            let elem = match token.context(ReadToken)? {
                LazyDataToken::PixelSequenceStart => {
                    let value = LazyDicomObject::build_encapsulated_data(&mut *dataset)?;
                    LazyElement::new_loaded(
                        DataElementHeader::new(Tag(0x7fe0, 0x0010), VR::OB, todo!()),
                        todo!(),
                        value,
                    )
                }
                LazyDataToken::ElementHeader(header) => {
                    // fetch respective value, place it in the entries
                    let next_token = dataset.advance().context(MissingElementValue)?;
                    match next_token.context(ReadToken)? {
                        t @ LazyDataToken::LazyValue { header, decoder } => {
                            // TODO choose whether to eagerly fetch the elemet or keep it unloaded
                            LazyElement {
                                header,
                                position: decoder.position(),
                                value: MaybeValue::Unloaded,
                            }
                        },
                        token => {
                            return UnexpectedToken { token }.fail();
                        }
                    }
                }
                LazyDataToken::SequenceStart { tag, len } => {
                    // delegate sequence building to another function
                    let items = Self::build_sequence(tag, len, dataset, &dict)?;
                    
                    // !!! Lazy Element does not fit the sequence system
                    todo!()
                    //LazyElement::new(tag, VR::SQ, Value::Sequence { items, size: len })
                }
                LazyDataToken::ItemEnd if in_item => {
                    // end of item, leave now
                    return Ok(LazyNestedObject { entries, dict, len });
                }
                token => return UnexpectedToken { token }.fail(),
            };
            entries.insert(elem.header.tag(), elem);
        }

        Ok(LazyNestedObject { entries, dict, len })
    }
}

*/

#[cfg(test)]
mod tests {

    use super::*;
    use crate::InMemDicomObject;
    use crate::{meta::FileMetaTableBuilder, open_file, Error};
    use byteordered::Endianness;
    use dicom_core::value::PrimitiveValue;
    use dicom_core::{
        dicom_value,
        header::{DataElementHeader, Length, VR},
    };
    use dicom_encoding::{
        decode::{basic::BasicDecoder, implicit_le::ImplicitVRLittleEndianDecoder},
        encode::EncoderFor,
        text::DefaultCharacterSetCodec,
        transfer_syntax::implicit_le::ImplicitVRLittleEndianEncoder,
    };
    use dicom_parser::{dataset::IntoTokens, StatefulDecoder};
    use tempfile;

    fn assert_obj_eq<D>(obj1: &InMemDicomObject<D>, obj2: &InMemDicomObject<D>)
    where
        D: std::fmt::Debug,
    {
        // debug representation because it makes a stricter comparison and
        // assumes that Undefined lengths are equal.
        assert_eq!(format!("{:?}", obj1), format!("{:?}", obj2))
    }

    #[test]
    #[ignore]
    fn inmem_object_read_dataset() {
        let data_in = [
            0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
            0x08, 0x00, 0x00, 0x00, // Length: 8
            b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
        ];

        let decoder = ImplicitVRLittleEndianDecoder::default();
        let text = Box::new(DefaultCharacterSetCodec) as Box<_>;
        let mut cursor = &data_in[..];
        let parser = StatefulDecoder::new(
            &mut cursor,
            decoder,
            BasicDecoder::new(Endianness::Little),
            text,
        );

        let obj = todo!(); // LazyDicomObject::read_dataset(parser).unwrap();

        let mut gt = InMemDicomObject::create_empty();

        let patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            dicom_value!(Strs, ["Doe^John"]),
        );
        gt.put(patient_name);

        //assert_eq!(obj, gt);
    }

}
