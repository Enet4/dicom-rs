//! DICOM object reader API
//!
//! This API can be used for reading DICOM objects in cohesive chunks,
//! thus obtaining some meta-data earlier for asynchronous processing
//! and potentially saving memory.

use std::{
    fmt,
    io::{BufReader, Read, Seek},
};

use dicom_core::{
    value::{PixelFragmentSequence, C},
    DataDictionary, DataElement, DicomValue, Length, Tag, VR,
};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_encoding::{TransferSyntax, TransferSyntaxIndex};
use dicom_parser::{
    dataset::{lazy_read::LazyDataSetReader, read::Error as ParserError, LazyDataToken},
    DynStatefulDecoder, StatefulDecode,
};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use smallvec::SmallVec;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};

use crate::{
    file::ReadPreamble,
    mem::{InMemElement, InMemFragment},
    FileMetaTable, InMemDicomObject,
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

type ParserResult<T> = std::result::Result<T, ParserError>;

#[derive(Debug, Snafu)]
#[non_exhaustive]
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
    /// Could not read preamble bytes
    ReadPreambleBytes {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not parse meta group data set"))]
    ParseMetaDataSet {
        #[snafu(backtrace)]
        source: crate::meta::Error,
    },
    #[snafu(display("Could not create data set parser"))]
    CreateParser {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::lazy_read::Error,
    },
    #[snafu(display("Could not read data set token"))]
    ReadToken {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::lazy_read::Error,
    },
    /// DICOM value not found after non-empty element header
    MissingElementValue,
    /// Could not guess source transfer syntax
    GuessTransferSyntax { backtrace: Backtrace },
    #[snafu(display("Unsupported transfer syntax `{}`", uid))]
    UnsupportedTransferSyntax { uid: String, backtrace: Backtrace },
    #[snafu(display("Unexpected token {:?}", token))]
    UnexpectedToken {
        token: dicom_parser::dataset::LazyDataTokenRepr,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not collect data in {}", tag))]
    CollectDataValue {
        tag: Tag,
        #[snafu(backtrace)]
        source: dicom_parser::dataset::Error,
    },
    #[snafu(display("Premature data set end"))]
    PrematureEnd { backtrace: Backtrace },
    /// Could not build file meta table
    BuildMetaTable {
        #[snafu(backtrace)]
        source: crate::meta::Error,
    },
    /// Could not prepare file meta table
    PrepareMetaTable {
        source: dicom_core::value::CastValueError,
        backtrace: Backtrace,
    },
}

enum CollectionSource<T> {
    Raw(Option<T>),
    Parser(LazyDataSetReader<DynStatefulDecoder<T>>),
}

impl<S> CollectionSource<S>
where
    S: Read + Seek,
{
    fn new(raw_source: S) -> Self {
        CollectionSource::Raw(Some(raw_source))
    }

    pub fn has_parser(&self) -> bool {
        matches!(self, CollectionSource::Parser(_))
    }

    fn raw_reader_mut(&mut self) -> &mut S {
        match self {
            CollectionSource::Raw(reader) => reader.as_mut().unwrap(),
            CollectionSource::Parser(_) => {
                panic!("cannot retrieve raw reader after setting parser")
            }
        }
    }

    fn set_parser_with_ts(
        &mut self,
        ts: &TransferSyntax,
    ) -> Result<&mut LazyDataSetReader<DynStatefulDecoder<S>>> {
        match self {
            CollectionSource::Raw(src) => {
                let src = src.take().unwrap();
                *self = CollectionSource::Parser(
                    LazyDataSetReader::new_with_ts(src, ts).context(CreateParserSnafu)?,
                );
                let CollectionSource::Parser(parser) = self else {
                    unreachable!();
                };
                Ok(parser)
            }
            CollectionSource::Parser(decoder) => Ok(decoder),
        }
    }

    fn parser(&mut self) -> &mut LazyDataSetReader<DynStatefulDecoder<S>> {
        match self {
            CollectionSource::Raw(_) => panic!("parser transfer syntax not set"),
            CollectionSource::Parser(parser) => parser,
        }
    }
}

/// A high-level construct for reading DICOM data sets in controlled chunks.
///
/// Unlike [`open_file`](crate::open_file),
/// this API makes it possible to read and process
/// multiple data set partitions from the same source in sequence,
/// making it appealing when working with data sets which are known to be large,
/// such as multi-frame images.
pub struct DicomObjectCollector<'t, D, S> {
    /// the source of byte data to read from
    source: CollectionSource<S>,
    /// data dictionary
    dictionary: D,
    /// transfer syntax suggestion
    ts_hint: Option<&'t TransferSyntax>,
    /// file meta table
    file_meta: Option<FileMetaTable>,
    options: DicomCollectorOptions,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DicomCollectorOptions {
    read_preamble: ReadPreamble,
}

impl<'t, D, S> fmt::Debug for DicomObjectCollector<'t, D, S>
where
    D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DicomObjectCollector")
            .field("dictionary", &self.dictionary)
            .field("ts_hint", &self.ts_hint.as_ref().map(|ts| ts.uid()))
            .finish()
    }
}

impl<'t, S> DicomObjectCollector<'t, StandardDataDictionary, BufReader<S>>
where
    S: Read + Seek,
{
    pub fn new(reader: BufReader<S>) -> Self {
        Self::new_with_dict(reader, StandardDataDictionary::default())
    }

    pub fn new_with_ts(reader: BufReader<S>, transfer_syntax: &'t TransferSyntax) -> Self {
        Self::new_with_dict_ts(reader, StandardDataDictionary, transfer_syntax)
    }
}

impl<'t, D, S> DicomObjectCollector<'t, D, BufReader<S>>
where
    D: DataDictionary + Clone,
    S: Read + Seek,
{
    // --- constructors ---

    pub fn new_with_dict(reader: BufReader<S>, dictionary: D) -> Self {
        DicomObjectCollector {
            source: CollectionSource::new(reader),
            dictionary,
            ts_hint: None,
            file_meta: None,
            options: Default::default(),
        }
    }

    pub fn new_with_dict_ts(
        reader: BufReader<S>,
        dictionary: D,
        transfer_syntax: &'t TransferSyntax,
    ) -> Self {
        DicomObjectCollector {
            source: CollectionSource::new(reader),
            dictionary,
            ts_hint: Some(transfer_syntax),
            file_meta: None,
            options: Default::default(),
        }
    }

    pub fn new_with_dict_options(
        reader: BufReader<S>,
        dictionary: D,
        transfer_syntax: &'t TransferSyntax,
        options: DicomCollectorOptions,
    ) -> Self {
        DicomObjectCollector {
            source: CollectionSource::new(reader),
            dictionary,
            ts_hint: Some(transfer_syntax),
            file_meta: None,
            options,
        }
    }

    // ---

    /// Read a file meta table from the source,
    /// retaining it in the reader for future reference.
    ///
    /// This method must be called
    /// whenever the source data is known to have a file meta group data set.
    pub fn read_file_meta(&mut self) -> Result<&FileMetaTable> {
        let reader = self.source.raw_reader_mut();
        self.file_meta = Some(FileMetaTable::from_reader(reader).context(BuildMetaTableSnafu)?);

        Ok(self.file_meta.as_ref().unwrap())
    }

    /// Read a DICOM data set until it finds its end,
    /// accumulating the elements into an in-memory object.
    pub fn read_dataset_to_end(&mut self, to: &mut InMemDicomObject<D>) -> Result<()> {
        let parser = if !self.source.has_parser() {
            let ts = self.guessed_ts().context(GuessTransferSyntaxSnafu)?;
            self.source.set_parser_with_ts(ts)?
        } else {
            self.source.parser()
        };

        Self::collect_to_object(parser, false, Length::UNDEFINED, None, to)
    }

    /// Read a DICOM data set until it reaches the given stop tag
    /// (excluding it) or finds the end of the data set,
    /// accumulating the elements into an in-memory object.
    pub fn read_dataset_up_to(
        &mut self,
        stop_tag: Tag,
        to: &mut InMemDicomObject<D>,
    ) -> Result<()> {
        let parser = if !self.source.has_parser() {
            let ts = self.guessed_ts().context(GuessTransferSyntaxSnafu)?;
            self.source.set_parser_with_ts(ts)?
        } else {
            self.source.parser()
        };

        Self::collect_to_object(parser, false, Length::UNDEFINED, Some(stop_tag), to)
    }

    fn guessed_ts(&mut self) -> Option<&'t TransferSyntax> {
        if self.ts_hint.is_some() {
            return self.ts_hint.clone();
        }
        if let Some(meta) = self.file_meta.as_ref() {
            self.ts_hint = TransferSyntaxRegistry.get(meta.transfer_syntax());
        }
        self.ts_hint.clone()
    }

    /// Collect DICOM data elements onto an in-memory DICOM object by consuming a data set parser.
    fn collect_to_object(
        token_src: &mut LazyDataSetReader<DynStatefulDecoder<BufReader<S>>>,
        in_item: bool,
        len: Length,
        read_until: Option<Tag>,
        to: &mut InMemDicomObject<D>,
    ) -> Result<()> {
        let mut elements = Vec::new();
        Self::collect_elements(token_src, in_item, len, read_until, &mut elements)?;
        to.extend(elements);
        Ok(())
    }

    /// Collect DICOM data elements onto a vector by consuming a data set parser.
    fn collect_elements(
        token_src: &mut LazyDataSetReader<DynStatefulDecoder<BufReader<S>>>,
        in_item: bool,
        len: Length,
        read_until: Option<Tag>,
        to: &mut Vec<DataElement<InMemDicomObject<D>>>,
    ) -> Result<()> {
        // perform a structured parsing of incoming tokens
        while let Some(token) = token_src.advance() {
            let elem = match token.context(ReadTokenSnafu)? {
                LazyDataToken::PixelSequenceStart => {
                    // stop reading if reached `read_until` tag
                    if read_until
                        .map(|t| t <= Tag(0x7fe0, 0x0010))
                        .unwrap_or(false)
                    {
                        break;
                    }
                    let value = Self::build_encapsulated_data(&mut *token_src)?;
                    DataElement::new(Tag(0x7fe0, 0x0010), VR::OB, value)
                }
                LazyDataToken::ElementHeader(header) => {
                    // stop reading if reached `read_until` tag
                    if read_until.map(|t| t <= header.tag).unwrap_or(false) {
                        break;
                    }

                    // fetch respective value, place it in the entries
                    let next_token = token_src.advance().context(MissingElementValueSnafu)?;
                    match next_token.context(ReadTokenSnafu)? {
                        token @ LazyDataToken::LazyItemValue { .. } => InMemElement::new_with_len(
                            header.tag,
                            header.vr,
                            header.len,
                            token
                                .into_value()
                                .context(CollectDataValueSnafu { tag: header.tag })?,
                        ),
                        token => {
                            return UnexpectedTokenSnafu { token }.fail();
                        }
                    }
                }
                LazyDataToken::SequenceStart { tag, len } => {
                    // stop reading if reached `read_until` tag
                    if read_until.map(|t| t <= tag).unwrap_or(false) {
                        break;
                    }

                    // delegate sequence building to another function
                    let mut items = Vec::new();
                    Self::collect_sequence(tag, len, &mut *token_src, &dict, &mut items)?;
                    DataElement::new_with_len(
                        tag,
                        VR::SQ,
                        len,
                        DicomValue::new_sequence(items, len),
                    )
                }
                LazyDataToken::ItemEnd if in_item => {
                    // end of item, leave now
                    return Ok(());
                }
                token => return UnexpectedTokenSnafu { token }.fail(),
            };
            to.push(elem);
        }

        Ok(())
    }

    /// Build an encapsulated pixel data by collecting all fragments into an
    /// in-memory DICOM value.
    fn build_encapsulated_data(
        dataset: &mut LazyDataSetReader<DynStatefulDecoder<BufReader<S>>>,
    ) -> Result<DicomValue<InMemDicomObject<D>, InMemFragment>> {
        // continue fetching tokens to retrieve:
        // - the offset table
        // - the various compressed fragments

        let mut offset_table = None;

        let mut fragments = C::new();

        // whether to read the fragment as the basic offset table (true)
        // or as a pixel data fragment (false)
        let mut first = true;

        while let Some(token) = dataset.advance() {
            let token = token.context(ReadTokenSnafu)?;
            match token {
                LazyDataToken::LazyItemValue { mut decoder, len } => {
                    if first {
                        let mut table = Vec::new();
                        decoder.read_u32_to_vec(len, &mut table);
                        first = false;
                    } else {
                        let mut data = Vec::new();
                        decoder.read_to_vec(len, &mut data);
                        fragments.push(data);
                    }
                }
                LazyDataToken::ItemEnd => {
                    // at the end of the first item ensure the presence of
                    // an empty offset_table here, so that the next items
                    // are seen as compressed fragments
                    if offset_table.is_none() {
                        offset_table = Some(Vec::new())
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
                | token @ LazyDataToken::LazyValue { .. }
                | token => {
                    return UnexpectedTokenSnafu { token }.fail();
                }
            }
        }

        Ok(DicomValue::from(PixelFragmentSequence::new(
            offset_table.unwrap_or_default(),
            fragments,
        )))
    }

    /// Build a DICOM sequence by consuming a data set parser.
    fn build_sequence(
        _tag: Tag,
        _len: Length,
        token_src: &mut LazyDataSetReader<DynStatefulDecoder<S>>,
        dict: &D,
    ) -> Result<C<InMemDicomObject<D>>>
    where
        D: Default,
    {
        let mut items: C<_> = SmallVec::new();
        while let Some(token) = token_src.advance() {
            match token.context(ReadTokenSnafu)? {
                LazyDataToken::ItemStart { len } => {
                    let mut obj = InMemDicomObject::new_empty_with_dict(D::default());
                    Self::collect_to_object(token_src, true, len, None, &mut obj)?;
                    items.push(obj);
                }
                LazyDataToken::SequenceEnd => {
                    return Ok(items);
                }
                token => return UnexpectedTokenSnafu { token }.fail(),
            };
        }

        // iterator fully consumed without a sequence delimiter
        PrematureEndSnafu.fail()
    }
}

#[cfg(test)]
mod tests {}
