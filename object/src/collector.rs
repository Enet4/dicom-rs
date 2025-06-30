//! DICOM object reader API
//!
//! The DICOM collector API in ([`DicomCollector`])
//! can be used for reading DICOM objects in cohesive chunks,
//! thus obtaining some meta-data earlier for asynchronous processing
//! and potentially saving memory.

use std::{
    fmt,
    fs::File,
    io::{BufRead, BufReader, Read, Seek},
    path::Path,
};

use dicom_core::{
    header::HasLength, value::{PixelFragmentSequence, C}, DataDictionary, DataElement, DicomValue, Length, Tag, VR
};
use dicom_dictionary_std::{tags, StandardDataDictionary};
use dicom_encoding::{decode::DecodeFrom, TransferSyntax, TransferSyntaxIndex};
use dicom_parser::{
    dataset::{lazy_read::LazyDataSetReader, DataToken, LazyDataToken},
    DynStatefulDecoder, StatefulDecode, StatefulDecoder,
};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use snafu::prelude::*;
use snafu::Backtrace;

use crate::{
    file::ReadPreamble,
    mem::{InMemElement, InMemFragment},
    FileMetaTable, InMemDicomObject,
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

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
    /// Illegal state for the requested operation: preamble has already been read
    IllegalStateStart,
    /// Illegal state for the requested operation: file meta group has already been read
    IllegalStateMeta,
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
    #[snafu(display("Unexpected data token {:?}", token))]
    UnexpectedDataToken {
        token: dicom_parser::dataset::DataToken,
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
    /// Could not read item
    ReadItem {
        #[snafu(backtrace)]
        source: dicom_parser::stateful::decode::Error,
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

    fn has_parser(&self) -> bool {
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
///
/// # Examples
///
/// It is possible to open a DICOM file and collect its file meta information
/// and main dataset.
///
/// ```no_run
/// # use dicom_object::InMemDicomObject;
/// # use dicom_object::collector::DicomCollector;
/// # use dicom_object::meta::FileMetaTable;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut collector = DicomCollector::open_file("file.dcm")?;
///
/// let fmi: &FileMetaTable = collector.read_file_meta()?;
/// let mut dset = InMemDicomObject::new_empty();
/// collector.read_dataset_to_end(&mut dset)?; // populate `dset` with all elements
/// # Ok(())
/// # }
/// ```
///
/// But at the moment,
/// this will be no different from using the regular file opening API.
/// To benefit from the collector,
/// read smaller portions of the dataset at a time.
/// For instance, you can first read patient/study attributes
/// and place image pixel attributes in a separate object.
///
/// ```no_run
/// # use dicom_object::InMemDicomObject;
/// # use dicom_object::collector::DicomCollector;
/// # use dicom_object::meta::FileMetaTable;
/// use dicom_core::Tag;
/// use dicom_dictionary_std::tags;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut collector = DicomCollector::open_file("file.dcm")?;
///
/// let fmi: &FileMetaTable = collector.read_file_meta()?;
/// let mut dset = InMemDicomObject::new_empty();
/// collector.read_dataset_up_to(Tag(0x0028, 0x0000), &mut dset)?; // read everything before the image pixel group
///
/// let mut pixel_image_dset = InMemDicomObject::new_empty();
/// collector.read_dataset_up_to(tags::PIXEL_DATA, &mut pixel_image_dset)?; // read from image pixel group to pixel data (excluding)
/// # Ok(())
/// # }
/// ```
///
/// Moreover, this API allows you to fetch and process
/// each pixel data fragment independently,
/// which is a significant memory saver in multi-frame scenarios.
///
/// ```no_run
/// # use dicom_object::InMemDicomObject;
/// # use dicom_object::collector::DicomCollector;
/// # use dicom_object::meta::FileMetaTable;
/// # use dicom_core::Tag;
/// # use dicom_dictionary_std::tags;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut collector = DicomCollector::open_file("file.dcm")?;
///
/// # let _fmi: &FileMetaTable = collector.read_file_meta()?;
/// # let mut dset = InMemDicomObject::new_empty();
/// # collector.read_dataset_up_to(tags::PIXEL_DATA, &mut dset)?;
/// let mut buf = Vec::new();
/// while let Some(len) = collector.read_next_fragment(&mut buf)? {
///    // should now have some data
///    assert_eq!(buf.len() as u32, len);
///    // process fragment (e.g. accumulate to a frame buffer and save to a file),
///    // and clear the buffer when done
///    buf.clear();
/// }
/// # Ok(())
/// # }
/// ```
pub struct DicomCollector<'t, D, S> {
    /// the source of byte data to read from
    source: CollectionSource<S>,
    /// data dictionary
    dictionary: D,
    /// transfer syntax suggestion
    ts_hint: Option<&'t TransferSyntax>,
    /// file meta table
    file_meta: Option<FileMetaTable>,
    options: DicomCollectorOptions,
    state: CollectorState,
}

// A state indicator of what has been collected so far
#[derive(Debug, Default, Copy, Clone, PartialEq)]
enum CollectorState {
    /// The collector is in the initial state.
    #[default]
    Start,
    /// The collector has read the preamble,
    /// or the preamble has been requested but not collected.
    Preamble,
    /// The collector has read the file meta group data set.
    ///
    /// If this state is reached,
    /// `file_meta` is guaranteed to be `Some`.
    FileMeta,
    /// The collector has read some portion the main data set.
    InDataset,
    /// The collector has read the pixel data element header.
    InPixelData,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DicomCollectorOptions {
    read_preamble: ReadPreamble,
}

impl<'t, D, S> fmt::Debug for DicomCollector<'t, D, S>
where
    D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DicomCollector")
            .field("dictionary", &self.dictionary)
            .field("ts_hint", &self.ts_hint.as_ref().map(|ts| ts.uid()))
            .field("state", &self.state)
            .finish()
    }
}

impl<'t, S> DicomCollector<'t, StandardDataDictionary, BufReader<S>>
where
    S: Read + Seek,
{
    /// Create a new DICOM dataset collector
    /// which reads from a buffered reader.
    ///
    /// The standard data dictionary is used.
    /// The transfer syntax is guessed from the file meta group data set.
    pub fn new(reader: BufReader<S>) -> Self {
        Self::new_with_dict(reader, StandardDataDictionary)
    }

    /// Create a new DICOM dataset collector
    /// which reads from a buffered reader
    /// and expects the given transfer syntax.
    ///
    /// The standard data dictionary is used.
    pub fn new_with_ts(reader: BufReader<S>, transfer_syntax: &'t TransferSyntax) -> Self {
        Self::new_with_dict_ts(reader, StandardDataDictionary, transfer_syntax)
    }
}

impl<'t> DicomCollector<'t, StandardDataDictionary, BufReader<File>> {
    /// Create a new DICOM dataset collector
    /// which reads from a standard DICOM file.
    ///
    /// The standard data dictionary is used.
    /// The transfer syntax is guessed from the file meta group data set.
    pub fn open_file(filename: impl AsRef<Path>) -> Result<Self> {
        Self::open_file_with_dict(filename, StandardDataDictionary)
    }
}

impl<'t, D> DicomCollector<'t, D, BufReader<File>>
where
    D: DataDictionary + Clone,
{
    /// Create a new DICOM dataset collector
    /// which reads from a standard DICOM file.
    ///
    /// The standard data dictionary is used.
    /// The transfer syntax is guessed from the file meta group data set.
    pub fn open_file_with_dict(filename: impl AsRef<Path>, dict: D) -> Result<Self> {
        let filename = filename.as_ref();
        let reader = BufReader::new(File::open(filename).context(OpenFileSnafu { filename })?);
        Ok(Self::new_with_dict(reader, dict))
    }
}

impl<'t, D, S> DicomCollector<'t, D, BufReader<S>>
where
    D: DataDictionary + Clone,
    S: Read + Seek,
{
    // --- constructors ---

    pub fn new_with_dict(reader: BufReader<S>, dictionary: D) -> Self {
        DicomCollector {
            source: CollectionSource::new(reader),
            dictionary,
            ts_hint: None,
            file_meta: None,
            options: Default::default(),
            state: Default::default(),
        }
    }

    pub fn new_with_dict_ts(
        reader: BufReader<S>,
        dictionary: D,
        transfer_syntax: &'t TransferSyntax,
    ) -> Self {
        DicomCollector {
            source: CollectionSource::new(reader),
            dictionary,
            ts_hint: Some(transfer_syntax),
            file_meta: None,
            options: Default::default(),
            state: Default::default(),
        }
    }

    pub fn new_with_dict_options(
        reader: BufReader<S>,
        dictionary: D,
        transfer_syntax: &'t TransferSyntax,
        options: DicomCollectorOptions,
    ) -> Self {
        DicomCollector {
            source: CollectionSource::new(reader),
            dictionary,
            ts_hint: Some(transfer_syntax),
            file_meta: None,
            options,
            state: Default::default(),
        }
    }

    // ---

    /// Read a DICOM file preamble from the given source.
    ///
    /// Returns the 128 bytes preceding the DICOM magic code,
    /// if they were found,
    /// or according to the `read_preamble` option on construction.
    pub fn read_preamble(&mut self) -> Result<Option<[u8; 128]>> {
        ensure!(self.state == CollectorState::Start, IllegalStateStartSnafu);

        if self.options.read_preamble == ReadPreamble::Never {
            self.state = CollectorState::Preamble;
            return Ok(None);
        }

        let reader = self.source.raw_reader_mut();
        let preamble = {
            if self.options.read_preamble == ReadPreamble::Always {
                // always assume that there is a preamble
                let mut buf = [0; 128];
                reader
                    .read_exact(&mut buf)
                    .context(ReadPreambleBytesSnafu)?;
                Some(buf)
            } else {
                // fill the buffer and try to identify where the magic code is
                let buf = reader.fill_buf().context(ReadPreambleBytesSnafu)?;
                if buf.len() < 4 {
                    return PrematureEndSnafu.fail();
                }

                if buf.len() >= 128 + 4 && &buf[128..132] == b"DICM" {
                    let out: [u8; 128] = std::convert::TryInto::try_into(&buf[0..128])
                        .expect("128 byte slice into array");
                    reader.consume(128);
                    Some(out)
                } else if &buf[0..4] == b"DICM" {
                    // assume that there is no preamble after all
                    None
                } else {
                    // take the risk and insist on the first 128 bytes
                    let mut out = [0; 128];
                    reader
                        .read_exact(&mut out)
                        .context(ReadPreambleBytesSnafu)?;
                    Some(out)
                }
            }
        };
        self.state = CollectorState::Preamble;
        Ok(preamble)
    }

    /// Read a file meta table from the source,
    /// retaining it in the reader for future reference.
    ///
    /// This method _must_ be called
    /// whenever the source data is known to have a file meta information group.
    /// Otherwise, it may fail to recognize the transfer syntax
    /// and fail on the first data set reading request.
    ///
    /// If the file meta information has already been collected,
    /// the previously saved file meta table is returned. 
    pub fn read_file_meta(&mut self) -> Result<&FileMetaTable> {

        // check if we are in good position to read the FMI,
        // or if we need to collect other things first

        if self.state == CollectorState::Start {
            // read preamble
            self.read_preamble()?;
        }

        if self.state == CollectorState::Preamble {

            let reader = self.source.raw_reader_mut();
            self.file_meta = Some(FileMetaTable::from_reader(reader).context(BuildMetaTableSnafu)?);

            self.state = CollectorState::FileMeta;
        }

        self.file_meta.as_ref().context(IllegalStateMetaSnafu)
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

        Self::collect_to_object(&mut self.state, parser, false, None, to, &self.dictionary)
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

        Self::collect_to_object(
            &mut self.state,
            parser,
            false,
            Some(stop_tag),
            to,
            &self.dictionary,
        )
    }

    /// Read the DICOM data set until it reaches the pixel data
    /// (if it has not done so yet)
    /// and collects the next pixel data fragment,
    /// appending the bytes into the given destination.
    ///
    /// If the data set contains native pixel data,
    /// the entire value data in the _Pixel Data_ attribute
    /// is interpreted as a single fragment.
    pub fn read_next_fragment(&mut self, to: &mut Vec<u8>) -> Result<Option<u32>> {
        if self.state == CollectorState::Start || self.state == CollectorState::Preamble {
            // read file meta information group
            self.read_file_meta()?;
        }

        // initialize parser if necessary
        if !self.source.has_parser() {
            let ts = self.guessed_ts().context(GuessTransferSyntaxSnafu)?;
            self.source.set_parser_with_ts(ts)?;
        } else {
            self.source.parser();
        }

        if self.state != CollectorState::InPixelData {
            // skip until we reach the pixel data

            self.skip_until(|token| {
                match token {
                    // catch either native pixel data
                    LazyDataToken::ElementHeader(header) if header.tag == tags::PIXEL_DATA && header.length().is_defined() => {
                        true
                    },
                    // or start of pixel data sequencce
                    LazyDataToken::PixelSequenceStart => {
                        true
                    },
                    _ => false,
                }
            })?;

            self.state = CollectorState::InPixelData;
        }

        let parser = if !self.source.has_parser() {
            let ts = self.guessed_ts().context(GuessTransferSyntaxSnafu)?;
            self.source.set_parser_with_ts(ts)?
        } else {
            self.source.parser()
        };

        // proceed with fetching tokens,
        // return the first fragment data found
        while let Some(token) = parser.advance() {
            match token.context(ReadTokenSnafu)? {
                // native pixel data
                LazyDataToken::LazyValue { header, decoder } => {
                    debug_assert!(header.length().is_defined());
                    let len = header.length().0;
                    decoder.read_to_vec(len, to).context(ReadItemSnafu)?;
                    return Ok(Some(len));
                }
                // fragment item data
                LazyDataToken::LazyItemValue { len, decoder } => {
                    decoder.read_to_vec(len, to).context(ReadItemSnafu)?;
                    return Ok(Some(len))
                }
                // empty item
                // (must be accounted for even though it yields no value token)
                LazyDataToken::ItemStart { len: Length(0) } => {
                    return Ok(Some(0))
                }
                _ => {
                    // no-op
                }
            }
        }

        Ok(None)
    }

    // --- private methods ---

    fn guessed_ts(&mut self) -> Option<&'t TransferSyntax> {
        if self.ts_hint.is_some() {
            return self.ts_hint;
        }
        if let Some(meta) = self.file_meta.as_ref() {
            self.ts_hint = TransferSyntaxRegistry.get(meta.transfer_syntax());
        }
        self.ts_hint
    }

    fn skip_until(&mut self, mut pred: impl FnMut(&LazyDataToken<&mut StatefulDecoder<Box<(dyn DecodeFrom<BufReader<S>> + 'static)>, BufReader<S>>>) -> bool) -> Result<bool> {
        let parser = self.source.parser();
        while let Some(token) = parser.advance() {
            let token = token.context(ReadTokenSnafu)?;
            if pred(&token) {
                return Ok(true);
            }
            // skip through values if necessary
            token.skip().context(ReadItemSnafu)?;
            self.state = CollectorState::InDataset;
            // continue
        }

        Ok(false)
    }

    // --- private helper functions ---

    /// Collect DICOM data elements onto an in-memory DICOM object by consuming a data set parser.
    fn collect_to_object(
        state: &mut CollectorState,
        token_src: &mut LazyDataSetReader<DynStatefulDecoder<BufReader<S>>>,
        in_item: bool,
        read_until: Option<Tag>,
        to: &mut InMemDicomObject<D>,
        dict: &D,
    ) -> Result<()> {
        let mut elements = Vec::new();
        Self::collect_elements(state, token_src, in_item, read_until, &mut elements, dict)?;
        to.extend(elements);
        Ok(())
    }

    /// Collect DICOM data elements onto a vector by consuming a data set parser.
    fn collect_elements(
        state: &mut CollectorState,
        token_src: &mut LazyDataSetReader<DynStatefulDecoder<BufReader<S>>>,
        in_item: bool,
        read_until: Option<Tag>,
        to: &mut Vec<DataElement<InMemDicomObject<D>>>,
        dict: &D,
    ) -> Result<()> {
        // perform a structured parsing of incoming tokens
        while let Some(token) = token_src.peek().context(ReadTokenSnafu)? {
            let token = token.clone();
            let elem = match token {
                DataToken::PixelSequenceStart => {
                    // stop reading if reached `read_until` tag
                    if read_until
                        .map(|t| t <= Tag(0x7fe0, 0x0010))
                        .unwrap_or(false)
                    {
                        break;
                    }
                    *state = CollectorState::InPixelData;
                    token_src.advance();
                    let value = Self::build_encapsulated_data(&mut *token_src)?;
                    DataElement::new(Tag(0x7fe0, 0x0010), VR::OB, value)
                }
                DataToken::ElementHeader(header) => {
                    // stop reading if reached `read_until` tag
                    if read_until.map(|t| t <= header.tag).unwrap_or(false) {
                        break;
                    }

                    drop(token);

                    *state = CollectorState::InDataset;
                    token_src.advance();

                    // fetch respective value, place it in the output
                    let next_token = token_src.advance().context(MissingElementValueSnafu)?;
                    match next_token.context(ReadTokenSnafu)? {
                        token @ LazyDataToken::LazyValue { .. }
                        | token @ LazyDataToken::LazyItemValue { .. } => {
                            InMemElement::new_with_len(
                                header.tag,
                                header.vr,
                                header.len,
                                token
                                    .into_value()
                                    .context(CollectDataValueSnafu { tag: header.tag })?,
                            )
                        }
                        token => {
                            return UnexpectedTokenSnafu { token }.fail();
                        }
                    }
                }
                DataToken::SequenceStart { tag, len } => {
                    // stop reading if reached `read_until` tag
                    if read_until.map(|t| t <= tag).unwrap_or(false) {
                        break;
                    }
                    *state = CollectorState::InDataset;

                    token_src.advance();

                    // delegate sequence building to another function
                    let mut items = C::new();
                    Self::collect_sequence(
                        &mut *state,
                        tag,
                        len,
                        &mut *token_src,
                        dict,
                        &mut items,
                    )?;
                    DataElement::new_with_len(
                        tag,
                        VR::SQ,
                        len,
                        DicomValue::new_sequence(items, len),
                    )
                }
                DataToken::ItemEnd if in_item => {
                    // end of item, leave now
                    token_src.advance();
                    return Ok(());
                }
                token => {
                    return UnexpectedDataTokenSnafu {
                        token: token.clone(),
                    }
                    .fail()
                }
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
                LazyDataToken::LazyItemValue { decoder, len } => {
                    if first {
                        let mut table = Vec::new();
                        decoder
                            .read_u32_to_vec(len, &mut table)
                            .context(ReadItemSnafu)?;
                        first = false;
                    } else {
                        let mut data = Vec::new();
                        decoder.read_to_vec(len, &mut data).context(ReadItemSnafu)?;
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
    fn collect_sequence(
        state: &mut CollectorState,
        _tag: Tag,
        _len: Length,
        token_src: &mut LazyDataSetReader<DynStatefulDecoder<BufReader<S>>>,
        dict: &D,
        items: &mut C<InMemDicomObject<D>>,
    ) -> Result<()> {
        while let Some(token) = token_src.advance() {
            match token.context(ReadTokenSnafu)? {
                LazyDataToken::ItemStart { len: _ } => {
                    let mut obj = InMemDicomObject::new_empty_with_dict(dict.clone());
                    Self::collect_to_object(state, token_src, true, None, &mut obj, dict)?;
                    items.push(obj);
                }
                LazyDataToken::SequenceEnd => {
                    return Ok(());
                }
                token => return UnexpectedTokenSnafu { token }.fail(),
            };
        }

        // iterator fully consumed without a sequence delimiter
        PrematureEndSnafu.fail()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Write};

    use dicom_core::{prelude::*, value::DataSetSequence, PrimitiveValue};
    use dicom_dictionary_std::{tags, uids, StandardDataDictionary};
    use dicom_encoding::TransferSyntaxIndex;
    use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

    use crate::{FileMetaTableBuilder, InMemDicomObject};

    use super::DicomCollector;

    /// read a plain data set without file meta group,
    /// by specifying the transfer syntax explicitly in the collector
    #[test]
    fn test_read_dataset_to_end_set_ts() {
        let dataset1 = InMemDicomObject::<StandardDataDictionary>::from_element_iter([
            DataElement::new(
                tags::SOP_INSTANCE_UID,
                VR::UI,
                "2.25.51008724832548260562721775118239811861\0",
            ),
            DataElement::new(
                tags::SOP_CLASS_UID,
                VR::UI,
                uids::NUCLEAR_MEDICINE_IMAGE_STORAGE,
            ),
            DataElement::new(tags::PATIENT_NAME, VR::PN, "Doe^John"),
            DataElement::new(tags::STUDY_DESCRIPTION, VR::LO, "Test study"),
            DataElement::new(tags::ROWS, VR::US, PrimitiveValue::from(64_u16)),
            DataElement::new(tags::COLUMNS, VR::US, PrimitiveValue::from(64_u16)),
            DataElement::new(tags::BITS_ALLOCATED, VR::US, PrimitiveValue::from(8_u16)),
            DataElement::new(tags::BITS_STORED, VR::US, PrimitiveValue::from(8_u16)),
            DataElement::new(tags::HIGH_BIT, VR::US, PrimitiveValue::from(7_u16)),
            DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::from(vec![0x55u8; 64 * 64]),
            ),
        ]);

        let ts_expl_vr_le = TransferSyntaxRegistry
            .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .unwrap();

        let mut encoded = Vec::new();
        dataset1
            .write_dataset_with_ts(&mut encoded, ts_expl_vr_le)
            .unwrap();

        let reader = BufReader::new(std::io::Cursor::new(&encoded));
        let mut collector = DicomCollector::new_with_ts(reader, ts_expl_vr_le);

        let mut dset = InMemDicomObject::new_empty();
        collector.read_dataset_to_end(&mut dset).unwrap();

        assert_eq!(dset, dataset1);
    }

    /// read a DICOM data set to the end,
    /// inferring the transfer syntax from the file meta group
    #[test]
    fn test_read_dataset_to_end_infer_from_meta() {
        let dataset1 = InMemDicomObject::<StandardDataDictionary>::from_element_iter([
            DataElement::new(
                tags::SOP_INSTANCE_UID,
                VR::UI,
                "2.25.245029432991021387484564600987886994494",
            ),
            DataElement::new(
                tags::SOP_CLASS_UID,
                VR::UI,
                uids::NUCLEAR_MEDICINE_IMAGE_STORAGE,
            ),
            DataElement::new(tags::PATIENT_NAME, VR::PN, "Doe^John"),
            DataElement::new(tags::STUDY_DESCRIPTION, VR::LO, "Test study"),
            DataElement::new(tags::ROWS, VR::US, PrimitiveValue::from(128_u16)),
            DataElement::new(tags::COLUMNS, VR::US, PrimitiveValue::from(128_u16)),
            DataElement::new(tags::BITS_ALLOCATED, VR::US, PrimitiveValue::from(16_u16)),
            DataElement::new(tags::BITS_STORED, VR::US, PrimitiveValue::from(16_u16)),
            DataElement::new(tags::HIGH_BIT, VR::US, PrimitiveValue::from(15_u16)),
            DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::from(vec![0x55u8; 128 * 128 * 2]),
            ),
        ]);

        let file_dataset1 = dataset1
            .clone()
            .with_meta(FileMetaTableBuilder::new().transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN))
            .unwrap();

        // write FMI and dataset to the buffer
        let mut encoded = Vec::new();
        encoded.write_all(b"DICM").unwrap();
        file_dataset1.meta().write(&mut encoded).unwrap();
        file_dataset1
            .write_dataset_with_ts(
                &mut encoded,
                TransferSyntaxRegistry
                    .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                    .unwrap(),
            )
            .unwrap();

        let reader = BufReader::new(std::io::Cursor::new(&encoded));
        let mut collector = DicomCollector::new(reader);

        let mut dset = InMemDicomObject::new_empty();
        let file_meta = collector.read_file_meta().unwrap();
        assert_eq!(file_meta.transfer_syntax(), uids::EXPLICIT_VR_LITTLE_ENDIAN,);
        collector.read_dataset_to_end(&mut dset).unwrap();

        assert_eq!(dset, dataset1);
    }

    /// read a DICOM data set with nested sequences
    #[test]
    fn test_read_dataset_nested() {
        let dataset1 = InMemDicomObject::<StandardDataDictionary>::from_element_iter([
            DataElement::new(
                tags::SOP_INSTANCE_UID,
                VR::UI,
                "2.25.245029432991021387484564600987886994494",
            ),
            DataElement::new(
                tags::SOP_CLASS_UID,
                VR::UI,
                uids::NUCLEAR_MEDICINE_IMAGE_STORAGE,
            ),
            DataElement::new(tags::PATIENT_NAME, VR::PN, "Doe^John"),
            DataElement::new(tags::STUDY_DESCRIPTION, VR::LO, "Test study"),
            DataElement::new(
                tags::ANATOMIC_REGION_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::CODE_VALUE, VR::SH, "51185008"),
                    DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "SCT"),
                    DataElement::new(tags::CODE_MEANING, VR::LO, "chest"),
                    DataElement::new(
                        tags::ANATOMIC_REGION_MODIFIER_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(tags::CODE_VALUE, VR::SH, "302551006"),
                            DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "SCT"),
                            DataElement::new(tags::CODE_MEANING, VR::LO, "entire thorax "),
                        ])]),
                    ),
                ])]),
            ),
            DataElement::new(tags::ROWS, VR::US, PrimitiveValue::from(128_u16)),
            DataElement::new(tags::COLUMNS, VR::US, PrimitiveValue::from(128_u16)),
            DataElement::new(tags::BITS_ALLOCATED, VR::US, PrimitiveValue::from(16_u16)),
            DataElement::new(tags::BITS_STORED, VR::US, PrimitiveValue::from(16_u16)),
            DataElement::new(tags::HIGH_BIT, VR::US, PrimitiveValue::from(7_u16)),
            DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::from(vec![0x55_u8; 128 * 128]),
            ),
        ]);

        let ts_expl_vr_le = TransferSyntaxRegistry
            .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .unwrap();

        let mut encoded = Vec::new();
        dataset1
            .write_dataset_with_ts(&mut encoded, ts_expl_vr_le)
            .unwrap();

        let reader = BufReader::new(std::io::Cursor::new(&encoded));

        let mut collector = DicomCollector::new_with_ts(reader, ts_expl_vr_le);

        let mut dset = InMemDicomObject::new_empty();
        collector.read_dataset_to_end(&mut dset).unwrap();

        // inspect some values using the attribute sequence API
        let v = dset
            .value_at((tags::ANATOMIC_REGION_SEQUENCE, tags::CODE_VALUE))
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(v, "51185008");

        let v = dset
            .value_at((
                tags::ANATOMIC_REGION_SEQUENCE,
                tags::ANATOMIC_REGION_MODIFIER_SEQUENCE,
                tags::CODE_MEANING,
            ))
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(v, "entire thorax");
    }

    /// read a DICOM data set in two chunks
    #[test]
    fn test_read_dataset_two_parts() {
        let dataset1 = InMemDicomObject::<StandardDataDictionary>::from_element_iter([
            DataElement::new(
                tags::SOP_INSTANCE_UID,
                VR::UI,
                "2.25.245029432991021387484564600987886994494",
            ),
            DataElement::new(
                tags::SOP_CLASS_UID,
                VR::UI,
                uids::NUCLEAR_MEDICINE_IMAGE_STORAGE,
            ),
            DataElement::new(tags::PATIENT_NAME, VR::PN, "Doe^John"),
            DataElement::new(tags::STUDY_DESCRIPTION, VR::LO, "Test study"),
            DataElement::new(tags::ROWS, VR::US, PrimitiveValue::from(128_u16)),
            DataElement::new(tags::COLUMNS, VR::US, PrimitiveValue::from(128_u16)),
            DataElement::new(tags::BITS_ALLOCATED, VR::US, PrimitiveValue::from(16_u16)),
            DataElement::new(tags::BITS_STORED, VR::US, PrimitiveValue::from(16_u16)),
            DataElement::new(tags::HIGH_BIT, VR::US, PrimitiveValue::from(7_u16)),
            DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::from(vec![0x55_u8; 128 * 128]),
            ),
        ]);

        let ts_expl_vr_le = TransferSyntaxRegistry
            .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .unwrap();

        let mut encoded = Vec::new();
        dataset1
            .write_dataset_with_ts(&mut encoded, ts_expl_vr_le)
            .unwrap();

        let reader = BufReader::new(std::io::Cursor::new(&encoded));

        let mut collector = DicomCollector::new_with_ts(reader, ts_expl_vr_le);

        // read one part of the data set
        let mut dset1 = InMemDicomObject::new_empty();

        collector
            .read_dataset_up_to(tags::ROWS, &mut dset1)
            .unwrap();
        // it has patient name and study description
        assert_eq!(
            dset1.get(tags::PATIENT_NAME).unwrap().to_str().unwrap(),
            "Doe^John"
        );
        assert_eq!(
            dset1
                .get(tags::STUDY_DESCRIPTION)
                .unwrap()
                .to_str()
                .unwrap(),
            "Test study"
        );
        // it does not have rows, or pixel data
        assert!(dset1.get(tags::ROWS).is_none());
        assert!(dset1.get(tags::PIXEL_DATA).is_none());

        // read part two of the data set
        let mut dset2 = InMemDicomObject::new_empty();

        collector.read_dataset_to_end(&mut dset2).unwrap();

        // it has rows and pixel data
        assert_eq!(dset2.get(tags::ROWS).unwrap().to_int::<u16>().unwrap(), 128);
        assert_eq!(
            dset2.get(tags::COLUMNS).unwrap().to_int::<u16>().unwrap(),
            128
        );
        assert_eq!(
            &*dset2.get(tags::PIXEL_DATA).unwrap().to_bytes().unwrap(),
            &[0x55_u8; 128 * 128]
        );

        // it does not have the other parts
        assert!(dset2.get(tags::SOP_INSTANCE_UID).is_none());
        assert!(dset2.get(tags::PATIENT_NAME).is_none());
        assert!(dset2.get(tags::STUDY_DESCRIPTION).is_none());
    }

    /// read the fragments of a DICOM file one by one
    #[test]
    fn test_read_fragments() {
        let filename = dicom_test_files::path("WG04/JPLY/SC1_JPLY").unwrap();

        let mut collector = DicomCollector::open_file(filename).unwrap();

        let fmi = collector.read_file_meta().unwrap();

        assert_eq!(fmi.transfer_syntax(), uids::JPEG_EXTENDED12_BIT);

        // collect the basic offset table
        // (currently exists as a regular fragment in this API)

        let mut bot = Vec::new();
        let len = collector
            .read_next_fragment(&mut bot)
            .expect("should read basic offset table successfully")
            .expect("should have basic offset table fragment");
        assert_eq!(len, 0);
        assert!(bot.is_empty());

        // collect the other fragments

        let mut fragment = Vec::with_capacity(131_072);

        let len = collector
            .read_next_fragment(&mut fragment)
            .expect("should read fragment successfully")
            .expect("should have fragment #0");
        assert_eq!(len, 65_536);

        // inspect a few bytes just to be sure
        assert_eq!(&fragment[0..4], &[0xFF, 0xD8, 0xFF, 0xC1]);

        // read one more

        let len = collector
            .read_next_fragment(&mut fragment)
            .expect("should read fragment successfully")
            .expect("should have fragment #1");
        assert_eq!(len, 65_536);

        // accumulates
        assert_eq!(fragment.len(), 131_072);

        // inspect a few bytes
        assert_eq!(&fragment[0..4], &[0xFF, 0xD8, 0xFF, 0xC1]);
        assert_eq!(&fragment[65_536..65_540], &[0x04, 0x6C, 0x3B, 0x60]);

        // check that it can fetch the remaining fragments
        let mut remaining: i32 = 10; // 12 fragments

        fragment.clear();

        while let Some(_len) = collector
            .read_next_fragment(&mut fragment)
            .expect("should have read fragment successfully")
        {
            remaining -= 1;
            assert!(!fragment.is_empty());
            fragment.clear();
        }

        assert_eq!(remaining, 0);
    }
}
