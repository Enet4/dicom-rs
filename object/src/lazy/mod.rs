//! This module contains the implementation for a lazily evaluated DICOM object.
//!
//! In a lazy DICOM object, larger DICOM elements
//! may be skipped during the decoding process,
//! and thus not be immediately available in memory.
//! A pointer to the original data source is kept for future access,
//! so that the element is fetched and its value is decoded on demand.

use dicom_dictionary_std::StandardDataDictionary;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use smallvec::SmallVec;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::cell::RefCell;
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

use self::element::LoadedValue;
pub use self::element::{MaybeElement, LazyNestedObject, MaybeValue};
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
    /// Could not read fragment from data source
    ReadFragment { source: StatefulDecodeError },
    /// Could not read pixel data offset table
    ReadOffsetTable { source: StatefulDecodeError },
    #[snafu(display("Unexpected token {:?}", token))]
    UnexpectedToken {
        token: dicom_parser::dataset::LazyDataTokenRepr,
        backtrace: Backtrace,
    },
    #[snafu(display("Pixel data fragment #{} was expected to be loaded, but was not", index))]
    UnloadedFragment {
        index: u32,
        backtrace: Backtrace,
    },
    /// Premature data set end
    PrematureEnd { backtrace: Backtrace },
    #[snafu(display("No such data element with tag {}", tag))]
    NoSuchDataElementTag { tag: Tag, backtrace: Backtrace },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The options for opening a DICOM file
/// as a lazily evaluated object.
#[derive(Debug, Default, Clone, PartialEq)]
#[non_exhaustive]
pub struct OpenFileOptions<D = StandardDataDictionary, T = TransferSyntaxRegistry> {
    /// the data dictionary to use
    pub dictionary: D,
    /// the transfer syntax registry to use
    pub ts_index: T,
}

/// A DICOM object which fetches elements from a data source on demand.
#[derive(Debug, Clone)]
pub struct LazyDicomObject<S, D> {
    /// the binary source to fetch DICOM data from
    source: S,
    /// the element dictionary at this level
    entries: BTreeMap<Tag, MaybeElement<D>>,
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

/// A temporary reference to a DICOM element which fetches its value on demand.
#[derive(Debug)]
pub struct LazyElement<'a, S: 'a, D> {
    source: &'a mut S,
    elem: &'a mut MaybeElement<D>,
}

impl<'a, S, D> LazyElement<'a, S, D>
where
    S: StatefulDecode,
    <S as StatefulDecode>::Reader: ReadSeek,
    D: Clone + DataDictionary,
{
    
    pub fn to_value(self) -> Result<LoadedValue<D>> {
        self.elem.load(self.source)?;

        todo!()
    }
}

impl LazyFileDicomObject<File, StandardDataDictionary> {
    /// Load a new lazy DICOM object from a file
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Self::from_file_with(
            path,
            OpenFileOptions::<_, TransferSyntaxRegistry>::default(),
        )
    }
}

impl<D> LazyFileDicomObject<File, D> {
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
        if let Some(ts) = ts_index.get(&meta.transfer_syntax) {
            let cs = SpecificCharacterSet::Default;
            let dataset =
                LazyDataSetReader::new_with_dictionary(file, dictionary.clone(), ts, cs)
                    .context(CreateParser)?;

            let mut builder = DataSetTableBuilder::new();
            let mut entries = BTreeMap::new();

            let mut dataset = RecordBuildingDataSetReader::new(dataset, &mut builder);

            LazyDicomObject::build_object(
                &mut dataset,
                &mut entries,
                dictionary.clone(),
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

impl<S> LazyDicomObject<S, StandardDataDictionary>
where
    S: StatefulDecode,
    <S as StatefulDecode>::Reader: ReadSeek,
{

    pub fn read_dataset(reader: LazyDataSetReader<S>) -> Result<Self> {
        Self::read_dataset_with(reader, StandardDataDictionary)
    }
}


impl<S, D> LazyDicomObject<S, D>
where
    S: StatefulDecode,
    <S as StatefulDecode>::Reader: ReadSeek,
    D: DataDictionary,
{

    pub fn read_dataset_with(reader: LazyDataSetReader<S, D>, dict: D) -> Result<Self> {
        todo!()
    }

    pub fn element<'a>(&'a mut self, tag: Tag) -> Result<LazyElement<'a, S, D>> {
        let source = &mut self.source;
        self.entries
            .get_mut(&tag)
            .ok_or_else(|| NoSuchDataElementTag { tag }.build())
            .map(move |elem| LazyElement {
                source,
                elem,
            })
    }

    pub fn element_mut<'a>(&'a mut self, tag: Tag) -> Result<LazyElement<'a, S, D>> {
        let source = &mut self.source;
        self.entries
            .get_mut(&tag)
            .ok_or_else(|| NoSuchDataElementTag { tag }.build())
            .map(move |elem| LazyElement {
                source,
                elem,
            })
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
        entries: &mut BTreeMap<Tag, MaybeElement<D>>,
        dict: D,
        in_item: bool,
        len: Length,
    ) -> Result<()> {
        todo!()
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

#[cfg(test)]
mod tests {

    use std::io::Cursor;

    use super::*;
    use byteordered::Endianness;
    use dicom_core::{
        dicom_value,
        header::{DataElementHeader, Length, VR},
    };
    use dicom_encoding::{
        decode::{basic::BasicDecoder, implicit_le::ImplicitVRLittleEndianDecoder},
        text::DefaultCharacterSetCodec,
    };
    use dicom_parser::StatefulDecoder;

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
        let mut cursor = Cursor::new(&data_in[..]);
        let parser = StatefulDecoder::new(
            &mut cursor,
            decoder,
            BasicDecoder::new(Endianness::Little),
            text,
        );
        let dataset = LazyDataSetReader::new(parser);

        let mut obj: LazyDicomObject<_, _> = LazyDicomObject::read_dataset(dataset).unwrap();

        let patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            DicomValue::new(dicom_value!(Strs, ["Doe^John"])),
        );

        let lazy_patient_name = obj.element(Tag(0x0010, 0x0010)).expect("Failed to retrieve element");


    }

}
