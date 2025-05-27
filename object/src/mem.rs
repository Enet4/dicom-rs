//! This module contains the implementation for an in-memory DICOM object.
//!
//! Use [`InMemDicomObject`] for your DICOM data set construction needs.
//! Values of this type support infallible insertion, removal, and retrieval
//! of elements by DICOM tag,
//! or name (keyword) with a data element dictionary look-up.
//!
//! If you wish to build a complete DICOM file,
//! you can start from an `InMemDicomObject`
//! and complement it with a [file meta group table](crate::meta)
//! (see [`with_meta`](InMemDicomObject::with_meta)
//! and [`with_exact_meta`](InMemDicomObject::with_exact_meta)).
//!
//! # Example
//!
//! A new DICOM data set can be built by providing a sequence of data elements.
//! Insertion and removal methods are also available.
//!
//! ```
//! # use dicom_core::{DataElement, VR, dicom_value};
//! # use dicom_dictionary_std::tags;
//! # use dicom_dictionary_std::uids;
//! # use dicom_object::InMemDicomObject;
//! let mut obj = InMemDicomObject::from_element_iter([
//!     DataElement::new(tags::SOP_CLASS_UID, VR::UI, uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE),
//!     DataElement::new(tags::SOP_INSTANCE_UID, VR::UI, "2.25.60156688944589400766024286894543900794"),
//!     // ...
//! ]);
//!
//! // continue adding elements
//! obj.put(DataElement::new(tags::MODALITY, VR::CS, "CR"));
//! ```
//!
//! In-memory DICOM objects may have a byte length recorded,
//! if it was part of a data set sequence with explicit length.
//! If necessary, this number can be obtained via the [`HasLength`] trait.
//! However, any modifications made to the object will reset this length
//! to [_undefined_](dicom_core::Length::UNDEFINED).
use dicom_core::ops::{
    ApplyOp, AttributeAction, AttributeOp, AttributeSelector, AttributeSelectorStep,
};
use dicom_encoding::Codec;
use dicom_parser::dataset::read::{DataSetReaderOptions, OddLengthStrategy};
use itertools::Itertools;
use smallvec::SmallVec;
use snafu::{ensure, OptionExt, ResultExt};
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::{collections::BTreeMap, io::Write};

use crate::file::ReadPreamble;
use crate::ops::{
    ApplyError, ApplyResult, IncompatibleTypesSnafu, ModifySnafu, UnsupportedActionSnafu,
};
use crate::{meta::FileMetaTable, FileMetaTableBuilder};
use crate::{
    AccessByNameError, AccessError, AtAccessError, BuildMetaTableSnafu, CreateParserSnafu,
    CreatePrinterSnafu, DicomObject, ElementNotFoundSnafu, FileDicomObject, InvalidGroupSnafu,
    MissingElementValueSnafu, MissingLeafElementSnafu, NoSpaceSnafu, NoSuchAttributeNameSnafu,
    NoSuchDataElementAliasSnafu, NoSuchDataElementTagSnafu, NotASequenceSnafu, OpenFileSnafu,
    ParseMetaDataSetSnafu, ParseSopAttributeSnafu, PrematureEndSnafu, PrepareMetaTableSnafu,
    PrintDataSetSnafu, PrivateCreatorNotFoundSnafu, PrivateElementError, ReadError, ReadFileSnafu,
    ReadPreambleBytesSnafu, ReadTokenSnafu, ReadUnsupportedTransferSyntaxSnafu,
    UnexpectedTokenSnafu, WithMetaError, WriteError,
};
use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry};
use dicom_core::header::{GroupNumber, HasLength, Header};
use dicom_core::value::{DataSetSequence, PixelFragmentSequence, Value, ValueType, C};
use dicom_core::{DataElement, Length, PrimitiveValue, Tag, VR};
use dicom_dictionary_std::{tags, StandardDataDictionary};
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_encoding::{encode::EncodeTo, text::SpecificCharacterSet, TransferSyntax};
use dicom_parser::dataset::{DataSetReader, DataToken, IntoTokensOptions};
use dicom_parser::{
    dataset::{read::Error as ParserError, DataSetWriter, IntoTokens},
    StatefulDecode,
};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

/// A full in-memory DICOM data element.
pub type InMemElement<D = StandardDataDictionary> = DataElement<InMemDicomObject<D>, InMemFragment>;

/// The type of a pixel data fragment.
pub type InMemFragment = dicom_core::value::InMemFragment;

type Result<T, E = AccessError> = std::result::Result<T, E>;

type ParserResult<T> = std::result::Result<T, ParserError>;

/// A DICOM object that is fully contained in memory.
///
/// See the [module-level documentation](self)
/// for more details.
#[derive(Debug, Clone)]
pub struct InMemDicomObject<D = StandardDataDictionary> {
    /// the element map
    entries: BTreeMap<Tag, InMemElement<D>>,
    /// the data dictionary
    dict: D,
    /// The length of the DICOM object in bytes.
    /// It is usually undefined, unless it is part of an item
    /// in a sequence with a specified length in its item header.
    len: Length,
    /// In case the SpecificCharSet changes we need to mark the object as dirty,
    /// because changing the character set may change the length in bytes of
    /// stored text. It has to be public for now because we need
    pub(crate) charset_changed: bool,
}

impl<D> PartialEq for InMemDicomObject<D> {
    // This implementation ignores the data dictionary.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<D> HasLength for InMemDicomObject<D> {
    fn length(&self) -> Length {
        self.len
    }
}

impl<'s, D: 's> DicomObject for &'s InMemDicomObject<D>
where
    D: DataDictionary,
    D: Clone,
{
    type Element = &'s InMemElement<D>;

    fn element(&self, tag: Tag) -> Result<Self::Element> {
        self.entries
            .get(&tag)
            .context(NoSuchDataElementTagSnafu { tag })
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element, AccessByNameError> {
        let tag = self.lookup_name(name)?;
        self.element(tag).map_err(|e| e.into_access_by_name(name))
    }
}

impl FileDicomObject<InMemDicomObject<StandardDataDictionary>> {
    /// Create a DICOM object by reading from a file.
    ///
    /// This function assumes the standard file encoding structure:
    /// first it automatically detects whether the 128-byte preamble is present,
    /// skipping it if found.
    /// Then it reads the file meta group,
    /// followed by the rest of the data set.
    pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Self, ReadError> {
        Self::open_file_with_dict(path, StandardDataDictionary)
    }

    /// Create a DICOM object by reading from a byte source.
    ///
    /// This function assumes the standard file encoding structure:
    /// first it automatically detects whether the 128-byte preamble is present,
    /// skipping it if found.
    /// Then it reads the file meta group,
    /// followed by the rest of the data set.
    pub fn from_reader<S>(src: S) -> Result<Self, ReadError>
    where
        S: Read + 'static,
    {
        Self::from_reader_with_dict(src, StandardDataDictionary)
    }
}

impl InMemDicomObject<StandardDataDictionary> {
    /// Create a new empty DICOM object.
    pub fn new_empty() -> Self {
        InMemDicomObject {
            entries: BTreeMap::new(),
            dict: StandardDataDictionary,
            len: Length::UNDEFINED,
            charset_changed: false,
        }
    }

    /// Construct a DICOM object from a fallible source of structured elements.
    #[inline]
    pub fn from_element_source<I>(iter: I) -> Result<Self>
    where
        I: IntoIterator<Item = Result<InMemElement<StandardDataDictionary>>>,
    {
        Self::from_element_source_with_dict(iter, StandardDataDictionary)
    }

    /// Construct a DICOM object from a non-fallible source of structured elements.
    #[inline]
    pub fn from_element_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = InMemElement<StandardDataDictionary>>,
    {
        Self::from_iter_with_dict(iter, StandardDataDictionary)
    }

    /// Construct a DICOM object representing a command set,
    /// from a non-fallible iterator of structured elements.
    ///
    /// This method will automatically insert
    /// a _Command Group Length_ (0000,0000) element
    /// based on the command elements found in the sequence.
    #[inline]
    pub fn command_from_element_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = InMemElement<StandardDataDictionary>>,
    {
        Self::command_from_iter_with_dict(iter, StandardDataDictionary)
    }

    /// Read an object from a source using the given decoder.
    ///
    /// Note: [`read_dataset_with_ts`] and [`read_dataset_with_ts_cs`]
    /// may be easier to use.
    ///
    /// [`read_dataset_with_ts`]: InMemDicomObject::read_dataset_with_ts
    /// [`read_dataset_with_ts_cs`]: InMemDicomObject::read_dataset_with_ts_cs
    #[inline]
    pub fn read_dataset<S>(decoder: S) -> Result<Self, ReadError>
    where
        S: StatefulDecode,
    {
        Self::read_dataset_with_dict(decoder, StandardDataDictionary)
    }

    /// Read an object from a source,
    /// using the given transfer syntax and default character set.
    ///
    /// If the attribute _Specific Character Set_ is found in the encoded data,
    /// this will override the given character set.
    #[inline]
    pub fn read_dataset_with_ts_cs<S>(
        from: S,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<Self, ReadError>
    where
        S: Read + 'static,
    {
        Self::read_dataset_with_dict_ts_cs(from, StandardDataDictionary, ts, cs)
    }

    /// Read an object from a source,
    /// using the given transfer syntax.
    ///
    /// The default character set is assumed
    /// until _Specific Character Set_ is found in the encoded data,
    /// after which the text decoder will be overridden accordingly.
    #[inline]
    pub fn read_dataset_with_ts<S>(from: S, ts: &TransferSyntax) -> Result<Self, ReadError>
    where
        S: Read,
    {
        Self::read_dataset_with_dict_ts_cs(
            from,
            StandardDataDictionary,
            ts,
            SpecificCharacterSet::default(),
        )
    }
}

impl<D> FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary,
    D: Clone,
{
    /// Create a new empty object, using the given dictionary and
    /// file meta table.
    pub fn new_empty_with_dict_and_meta(dict: D, meta: FileMetaTable) -> Self {
        FileDicomObject {
            meta,
            obj: InMemDicomObject {
                entries: BTreeMap::new(),
                dict,
                len: Length::UNDEFINED,
                charset_changed: false,
            },
        }
    }

    /// Create a DICOM object by reading from a file.
    ///
    /// This function assumes the standard file encoding structure:
    /// first it automatically detects whether the 128-byte preamble is present,
    /// skipping it when found.
    /// Then it reads the file meta group,
    /// followed by the rest of the data set.
    pub fn open_file_with_dict<P: AsRef<Path>>(path: P, dict: D) -> Result<Self, ReadError> {
        Self::open_file_with(path, dict, TransferSyntaxRegistry)
    }

    /// Create a DICOM object by reading from a file.
    ///
    /// This function assumes the standard file encoding structure:
    /// first it automatically detects whether the 128-byte preamble is present,
    /// skipping it when found.
    /// Then it reads the file meta group,
    /// followed by the rest of the data set.
    ///
    /// This function allows you to choose a different transfer syntax index,
    /// but its use is only advised when the built-in transfer syntax registry
    /// is insufficient. Otherwise, please use [`open_file_with_dict`] instead.
    ///
    /// [`open_file_with_dict`]: #method.open_file_with_dict
    pub fn open_file_with<P, R>(path: P, dict: D, ts_index: R) -> Result<Self, ReadError>
    where
        P: AsRef<Path>,
        R: TransferSyntaxIndex,
    {
        Self::open_file_with_all_options(
            path,
            dict,
            ts_index,
            None,
            ReadPreamble::Auto,
            Default::default(),
        )
    }

    // detect the presence of a preamble
    // and provide a better `ReadPreamble` option accordingly
    fn detect_preamble<S>(reader: &mut BufReader<S>) -> std::io::Result<ReadPreamble>
    where
        S: Read,
    {
        let buf = reader.fill_buf()?;
        let buflen = buf.len();

        if buflen < 4 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }

        if buflen >= 132 && &buf[128..132] == b"DICM" {
            return Ok(ReadPreamble::Always);
        }

        if &buf[0..4] == b"DICM" {
            return Ok(ReadPreamble::Never);
        }

        // could not detect
        Ok(ReadPreamble::Auto)
    }

    pub(crate) fn open_file_with_all_options<P, R>(
        path: P,
        dict: D,
        ts_index: R,
        read_until: Option<Tag>,
        mut read_preamble: ReadPreamble,
        odd_length: OddLengthStrategy,
    ) -> Result<Self, ReadError>
    where
        P: AsRef<Path>,
        R: TransferSyntaxIndex,
    {
        let path = path.as_ref();
        let mut file =
            BufReader::new(File::open(path).with_context(|_| OpenFileSnafu { filename: path })?);

        if read_preamble == ReadPreamble::Auto {
            read_preamble = Self::detect_preamble(&mut file)
                .with_context(|_| ReadFileSnafu { filename: path })?;
        }

        if read_preamble == ReadPreamble::Auto || read_preamble == ReadPreamble::Always {
            let mut buf = [0u8; 128];
            // skip the preamble
            file.read_exact(&mut buf)
                .with_context(|_| ReadFileSnafu { filename: path })?;
        }

        // read metadata header
        let mut meta = FileMetaTable::from_reader(&mut file).context(ParseMetaDataSetSnafu)?;

        // read rest of data according to metadata, feed it to object
        if let Some(ts) = ts_index.get(&meta.transfer_syntax) {
            let mut options = DataSetReaderOptions::default();
            options.odd_length = odd_length;

            let obj = if let Codec::Dataset(Some(adapter)) = ts.codec() {
                let adapter = adapter.adapt_reader(Box::new(file));
                let mut dataset =
                    DataSetReader::new_with_ts(adapter, ts).context(CreateParserSnafu)?;

                InMemDicomObject::build_object(
                    &mut dataset,
                    dict,
                    false,
                    Length::UNDEFINED,
                    read_until,
                )?
            } else {
                let mut dataset =
                    DataSetReader::new_with_ts(file, ts).context(CreateParserSnafu)?;

                InMemDicomObject::build_object(
                    &mut dataset,
                    dict,
                    false,
                    Length::UNDEFINED,
                    read_until,
                )?
            };

            // if Media Storage SOP Class UID is empty attempt to infer from SOP Class UID
            if meta.media_storage_sop_class_uid().is_empty() {
                if let Some(elem) = obj.get(tags::SOP_CLASS_UID) {
                    meta.media_storage_sop_class_uid = elem
                        .value()
                        .to_str()
                        .context(ParseSopAttributeSnafu)?
                        .to_string();
                }
            }

            // if Media Storage SOP Instance UID is empty attempt to infer from SOP Instance UID
            if meta.media_storage_sop_instance_uid().is_empty() {
                if let Some(elem) = obj.get(tags::SOP_INSTANCE_UID) {
                    meta.media_storage_sop_instance_uid = elem
                        .value()
                        .to_str()
                        .context(ParseSopAttributeSnafu)?
                        .to_string();
                }
            }

            Ok(FileDicomObject { meta, obj })
        } else {
            ReadUnsupportedTransferSyntaxSnafu {
                uid: meta.transfer_syntax,
            }
            .fail()
        }
    }

    /// Create a DICOM object by reading from a byte source.
    ///
    /// This function assumes the standard file encoding structure:
    /// first it automatically detects whether the 128-byte preamble is present,
    /// skipping it when found.
    /// Then it reads the file meta group,
    /// followed by the rest of the data set.
    pub fn from_reader_with_dict<S>(src: S, dict: D) -> Result<Self, ReadError>
    where
        S: Read,
    {
        Self::from_reader_with(src, dict, TransferSyntaxRegistry)
    }

    /// Create a DICOM object by reading from a byte source.
    ///
    /// This function assumes the standard file encoding structure:
    /// first it automatically detects whether the preamble is present,
    /// skipping it when found.
    /// Then it reads the file meta group,
    /// followed by the rest of the data set.
    ///
    /// This function allows you to choose a different transfer syntax index,
    /// but its use is only advised when the built-in transfer syntax registry
    /// is insufficient. Otherwise, please use [`from_reader_with_dict`] instead.
    ///
    /// [`from_reader_with_dict`]: #method.from_reader_with_dict
    pub fn from_reader_with<S, R>(src: S, dict: D, ts_index: R) -> Result<Self, ReadError>
    where
        S: Read,
        R: TransferSyntaxIndex,
    {
        Self::from_reader_with_all_options(
            src,
            dict,
            ts_index,
            None,
            ReadPreamble::Auto,
            Default::default(),
        )
    }

    pub(crate) fn from_reader_with_all_options<S, R>(
        src: S,
        dict: D,
        ts_index: R,
        read_until: Option<Tag>,
        mut read_preamble: ReadPreamble,
        odd_length: OddLengthStrategy,
    ) -> Result<Self, ReadError>
    where
        S: Read,
        R: TransferSyntaxIndex,
    {
        let mut file = BufReader::new(src);

        if read_preamble == ReadPreamble::Auto {
            read_preamble = Self::detect_preamble(&mut file).context(ReadPreambleBytesSnafu)?;
        }

        if read_preamble == ReadPreamble::Always {
            // skip preamble
            let mut buf = [0u8; 128];
            // skip the preamble
            file.read_exact(&mut buf).context(ReadPreambleBytesSnafu)?;
        }

        // read metadata header
        let meta = FileMetaTable::from_reader(&mut file).context(ParseMetaDataSetSnafu)?;

        // read rest of data according to metadata, feed it to object
        if let Some(ts) = ts_index.get(&meta.transfer_syntax) {
            let mut options = DataSetReaderOptions::default();
            options.odd_length = odd_length;

            if let Codec::Dataset(Some(adapter)) = ts.codec() {
                let adapter = adapter.adapt_reader(Box::new(file));
                let mut dataset =
                    DataSetReader::new_with_ts_options(adapter, ts, options).context(CreateParserSnafu)?;
                let obj = InMemDicomObject::build_object(
                    &mut dataset,
                    dict,
                    false,
                    Length::UNDEFINED,
                    read_until,
                )?;
                Ok(FileDicomObject { meta, obj })
            } else {
                let mut dataset =
                    DataSetReader::new_with_ts_options(file, ts, options).context(CreateParserSnafu)?;
                let obj = InMemDicomObject::build_object(
                    &mut dataset,
                    dict,
                    false,
                    Length::UNDEFINED,
                    read_until,
                )?;
                Ok(FileDicomObject { meta, obj })
            }
        } else {
            ReadUnsupportedTransferSyntaxSnafu {
                uid: meta.transfer_syntax,
            }
            .fail()
        }
    }
}

impl FileDicomObject<InMemDicomObject<StandardDataDictionary>> {
    /// Create a new empty object, using the given file meta table.
    pub fn new_empty_with_meta(meta: FileMetaTable) -> Self {
        FileDicomObject {
            meta,
            obj: InMemDicomObject {
                entries: BTreeMap::new(),
                dict: StandardDataDictionary,
                len: Length::UNDEFINED,
                charset_changed: false,
            },
        }
    }
}

impl<D> InMemDicomObject<D>
where
    D: DataDictionary,
    D: Clone,
{
    /// Create a new empty object, using the given dictionary for name lookup.
    pub fn new_empty_with_dict(dict: D) -> Self {
        InMemDicomObject {
            entries: BTreeMap::new(),
            dict,
            len: Length::UNDEFINED,
            charset_changed: false,
        }
    }

    /// Construct a DICOM object from an iterator of structured elements.
    pub fn from_element_source_with_dict<I>(iter: I, dict: D) -> Result<Self>
    where
        I: IntoIterator<Item = Result<InMemElement<D>>>,
    {
        let entries: Result<_> = iter.into_iter().map_ok(|e| (e.tag(), e)).collect();
        Ok(InMemDicomObject {
            entries: entries?,
            dict,
            len: Length::UNDEFINED,
            charset_changed: false,
        })
    }

    /// Construct a DICOM object from a non-fallible iterator of structured elements.
    pub fn from_iter_with_dict<I>(iter: I, dict: D) -> Self
    where
        I: IntoIterator<Item = InMemElement<D>>,
    {
        let entries = iter.into_iter().map(|e| (e.tag(), e)).collect();
        InMemDicomObject {
            entries,
            dict,
            len: Length::UNDEFINED,
            charset_changed: false,
        }
    }

    /// Construct a DICOM object representing a command set,
    /// from a non-fallible iterator of structured elements.
    ///
    /// This method will automatically insert
    /// a _Command Group Length_ (0000,0000) element
    /// based on the command elements found in the sequence.
    pub fn command_from_iter_with_dict<I>(iter: I, dict: D) -> Self
    where
        I: IntoIterator<Item = InMemElement<D>>,
    {
        let mut calculated_length: u32 = 0;
        let mut entries: BTreeMap<_, _> = iter
            .into_iter()
            .map(|e| {
                // count the length of command set elements
                if e.tag().0 == 0x0000 && e.tag().1 != 0x0000 {
                    let l = e.value().length();
                    calculated_length += if l.is_defined() { even_len(l.0) } else { 0 } + 8;
                }

                (e.tag(), e)
            })
            .collect();

        entries.insert(
            Tag(0, 0),
            InMemElement::new(Tag(0, 0), VR::UL, PrimitiveValue::from(calculated_length)),
        );

        InMemDicomObject {
            entries,
            dict,
            len: Length::UNDEFINED,
            charset_changed: false,
        }
    }

    /// Read an object from a source,
    /// using the given decoder
    /// and the given dictionary for name lookup.
    pub fn read_dataset_with_dict<S>(decoder: S, dict: D) -> Result<Self, ReadError>
    where
        S: StatefulDecode,
        D: DataDictionary,
    {
        let mut dataset = DataSetReader::new(decoder, Default::default());
        InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED, None)
    }

    /// Read an object from a source,
    /// using the given data dictionary and transfer syntax.
    #[inline]
    pub fn read_dataset_with_dict_ts<S>(
        from: S,
        dict: D,
        ts: &TransferSyntax,
    ) -> Result<Self, ReadError>
    where
        S: Read,
        D: DataDictionary,
    {
        Self::read_dataset_with_dict_ts_cs(from, dict, ts, SpecificCharacterSet::default())
    }

    /// Read an object from a source,
    /// using the given data dictionary,
    /// transfer syntax,
    /// and the given character set to assume by default.
    ///
    /// If the attribute _Specific Character Set_ is found in the encoded data,
    /// this will override the given character set.
    pub fn read_dataset_with_dict_ts_cs<S>(
        from: S,
        dict: D,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<Self, ReadError>
    where
        S: Read,
        D: DataDictionary,
    {
        let from = BufReader::new(from);
        if let Codec::Dataset(Some(adapter)) = ts.codec() {
            let adapter = adapter.adapt_reader(Box::new(from));
            let mut dataset =
                DataSetReader::new_with_ts_cs(adapter, ts, cs).context(CreateParserSnafu)?;
            InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED, None)
        } else {
            let mut dataset = DataSetReader::new_with_ts_cs(from, ts, cs).context(CreateParserSnafu)?;
            InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED, None)
        }
    }

    // Standard methods follow. They are not placed as a trait implementation
    // because they may require outputs to reference the lifetime of self,
    // which is not possible without GATs.

    /// Retrieve a particular DICOM element by its tag.
    ///
    /// An error is returned if the element does not exist.
    /// For an alternative to this behavior,
    /// see [`element_opt`](InMemDicomObject::element_opt).
    pub fn element(&self, tag: Tag) -> Result<&InMemElement<D>> {
        self.entries
            .get(&tag)
            .context(NoSuchDataElementTagSnafu { tag })
    }

    /// Retrieve a particular DICOM element by its name.
    ///
    /// This method translates the given attribute name into its tag
    /// before retrieving the element.
    /// If the attribute is known in advance,
    /// using [`element`](InMemDicomObject::element)
    /// with a tag constant is preferred.
    ///
    /// An error is returned if the element does not exist.
    /// For an alternative to this behavior,
    /// see [`element_by_name_opt`](InMemDicomObject::element_by_name_opt).
    pub fn element_by_name(&self, name: &str) -> Result<&InMemElement<D>, AccessByNameError> {
        let tag = self.lookup_name(name)?;
        self.entries
            .get(&tag)
            .with_context(|| NoSuchDataElementAliasSnafu {
                tag,
                alias: name.to_string(),
            })
    }

    /// Retrieve a particular DICOM element that might not exist by its tag.
    ///
    /// If the element does not exist,
    /// `None` is returned.
    pub fn element_opt(&self, tag: Tag) -> Result<Option<&InMemElement<D>>, AccessError> {
        match self.element(tag) {
            Ok(e) => Ok(Some(e)),
            Err(super::AccessError::NoSuchDataElementTag { .. }) => Ok(None),
        }
    }

    /// Get a particular DICOM attribute from this object by tag.
    ///
    /// If the element does not exist,
    /// `None` is returned.
    pub fn get(&self, tag: Tag) -> Option<&InMemElement<D>> {
        self.entries.get(&tag)
    }

    // Get a mutable reference to a particular DICOM attribute from this object by tag.
    //
    // Should be private as it would allow a user to change the tag of an
    // element and diverge from the dictionary
    fn get_mut(&mut self, tag: Tag) -> Option<&mut InMemElement<D>> {
        self.entries.get_mut(&tag)
    }

    /// Retrieve a particular DICOM element that might not exist by its name.
    ///
    /// If the element does not exist,
    /// `None` is returned.
    ///
    /// This method translates the given attribute name into its tag
    /// before retrieving the element.
    /// If the attribute is known in advance,
    /// using [`element_opt`](InMemDicomObject::element_opt)
    /// with a tag constant is preferred.
    pub fn element_by_name_opt(
        &self,
        name: &str,
    ) -> Result<Option<&InMemElement<D>>, AccessByNameError> {
        match self.element_by_name(name) {
            Ok(e) => Ok(Some(e)),
            Err(AccessByNameError::NoSuchDataElementAlias { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn find_private_creator(&self, group: GroupNumber, creator: &str) -> Option<&Tag> {
        let range = Tag(group, 0)..Tag(group, 0xFF);
        for (tag, elem) in self.entries.range(range) {
            // Private Creators are always LO
            // https://dicom.nema.org/medical/dicom/2024a/output/chtml/part05/sect_7.8.html
            if elem.header().vr() == VR::LO && elem.to_str().unwrap_or_default() == creator {
                return Some(tag);
            }
        }
        None
    }

    /// Get a private element from the dataset using the group number, creator and element number.
    ///
    /// An error is raised when the group number is not odd,
    /// the private creator is not found in the group,
    /// or the private element is not found.
    ///
    /// For more info, see the [DICOM standard section on private elements][1].
    ///
    /// [1]: https://dicom.nema.org/medical/dicom/2024a/output/chtml/part05/sect_7.8.html
    ///
    /// ## Example
    ///
    /// ```
    /// # use dicom_core::{VR, PrimitiveValue, Tag, DataElement};
    /// # use dicom_object::{InMemDicomObject, PrivateElementError};
    /// # use std::error::Error;
    /// let mut ds = InMemDicomObject::from_element_iter([
    ///     DataElement::new(
    ///         Tag(0x0009, 0x0010),
    ///         VR::LO,
    ///         PrimitiveValue::from("CREATOR 1"),
    ///     ),
    ///     DataElement::new(Tag(0x0009, 0x01001), VR::DS, "1.0"),
    /// ]);
    /// assert_eq!(
    ///     ds.private_element(0x0009, "CREATOR 1", 0x01)?
    ///         .value()
    ///         .to_str()?,
    ///     "1.0"
    /// );
    /// # Ok::<(), Box<dyn Error>>(())
    /// ```
    pub fn private_element(
        &self,
        group: GroupNumber,
        creator: &str,
        element: u8,
    ) -> Result<&InMemElement<D>, PrivateElementError> {
        let tag = self.find_private_creator(group, creator).ok_or_else(|| {
            PrivateCreatorNotFoundSnafu {
                group,
                creator: creator.to_string(),
            }
            .build()
        })?;

        let element_num = (tag.element() << 8) | (element as u16);
        self.get(Tag(group, element_num)).ok_or_else(|| {
            ElementNotFoundSnafu {
                group,
                creator: creator.to_string(),
                elem: element,
            }
            .build()
        })
    }

    /// Insert a data element to the object, replacing (and returning) any
    /// previous element of the same attribute.
    /// This might invalidate all sequence and item lengths if the charset of the
    /// element changes.
    pub fn put(&mut self, elt: InMemElement<D>) -> Option<InMemElement<D>> {
        self.put_element(elt)
    }

    /// Insert a data element to the object, replacing (and returning) any
    /// previous element of the same attribute.
    /// This might invalidate all sequence and item lengths if the charset of the
    /// element changes.
    pub fn put_element(&mut self, elt: InMemElement<D>) -> Option<InMemElement<D>> {
        self.len = Length::UNDEFINED;
        self.invalidate_if_charset_changed(elt.tag());
        self.entries.insert(elt.tag(), elt)
    }

    /// Insert a private element into the dataset, replacing (and returning) any
    /// previous element of the same attribute.
    ///
    /// This function will find the next available private element block in the given
    /// group. If the creator already exists, the element will be added to the block
    /// already reserved for that creator. If it does not exist, then a new block
    /// will be reserved for the creator in the specified group.
    /// An error is returned if there is no space left in the group.
    ///
    /// For more info, see the [DICOM standard section on private elements][1].
    ///
    /// [1]: https://dicom.nema.org/medical/dicom/2024a/output/chtml/part05/sect_7.8.html
    ///
    /// ## Example
    /// ```
    /// # use dicom_core::{VR, PrimitiveValue, Tag, DataElement, header::Header};
    /// # use dicom_object::InMemDicomObject;
    /// # use std::error::Error;
    /// let mut ds = InMemDicomObject::new_empty();
    /// ds.put_private_element(
    ///     0x0009,
    ///     "CREATOR 1",
    ///     0x02,
    ///     VR::DS,
    ///     PrimitiveValue::from("1.0"),
    /// )?;
    /// assert_eq!(
    ///     ds.private_element(0x0009, "CREATOR 1", 0x02)?
    ///         .value()
    ///         .to_str()?,
    ///     "1.0"
    /// );
    /// assert_eq!(
    ///     ds.private_element(0x0009, "CREATOR 1", 0x02)?
    ///         .header()
    ///         .tag(),
    ///     Tag(0x0009, 0x0102)
    /// );
    /// # Ok::<(), Box<dyn Error>>(())
    /// ```
    pub fn put_private_element(
        &mut self,
        group: GroupNumber,
        creator: &str,
        element: u8,
        vr: VR,
        value: PrimitiveValue,
    ) -> Result<Option<InMemElement<D>>, PrivateElementError> {
        ensure!(group % 2 == 1, InvalidGroupSnafu { group });
        let private_creator = self.find_private_creator(group, creator);
        if let Some(tag) = private_creator {
            // Private creator already exists
            let tag = Tag(group, (tag.element() << 8) | element as u16);
            Ok(self.put_element(DataElement::new(tag, vr, value)))
        } else {
            // Find last reserved block of tags.
            let range = Tag(group, 0)..Tag(group, 0xFF);
            let last_entry = self.entries.range(range).next_back();
            let next_available = match last_entry {
                Some((tag, _)) => tag.element() + 1,
                None => 0x01,
            };
            if next_available < 0xFF {
                // Put private creator
                let tag = Tag(group, next_available);
                self.put_str(tag, VR::LO, creator);

                // Put private element
                let tag = Tag(group, (next_available << 8) | element as u16);
                Ok(self.put_element(DataElement::new(tag, vr, value)))
            } else {
                NoSpaceSnafu { group }.fail()
            }
        }
    }

    /// Insert a new element with a string value to the object,
    /// replacing (and returning) any previous element of the same attribute.
    pub fn put_str(
        &mut self,
        tag: Tag,
        vr: VR,
        string: impl Into<String>,
    ) -> Option<InMemElement<D>> {
        self.put_element(DataElement::new(tag, vr, string.into()))
    }

    /// Remove a DICOM element by its tag,
    /// reporting whether it was present.
    pub fn remove_element(&mut self, tag: Tag) -> bool {
        if self.entries.remove(&tag).is_some() {
            self.len = Length::UNDEFINED;
            true
        } else {
            false
        }
    }

    /// Remove a DICOM element by its keyword,
    /// reporting whether it was present.
    pub fn remove_element_by_name(&mut self, name: &str) -> Result<bool, AccessByNameError> {
        let tag = self.lookup_name(name)?;
        Ok(self.entries.remove(&tag).is_some()).map(|removed| {
            if removed {
                self.len = Length::UNDEFINED;
            }
            removed
        })
    }

    /// Remove and return a particular DICOM element by its tag.
    pub fn take_element(&mut self, tag: Tag) -> Result<InMemElement<D>> {
        self.entries
            .remove(&tag)
            .map(|e| {
                self.len = Length::UNDEFINED;
                e
            })
            .context(NoSuchDataElementTagSnafu { tag })
    }

    /// Remove and return a particular DICOM element by its tag,
    /// if it is present,
    /// returns `None` otherwise.
    pub fn take(&mut self, tag: Tag) -> Option<InMemElement<D>> {
        self.entries.remove(&tag).map(|e| {
            self.len = Length::UNDEFINED;
            e
        })
    }

    /// Remove and return a particular DICOM element by its name.
    pub fn take_element_by_name(
        &mut self,
        name: &str,
    ) -> Result<InMemElement<D>, AccessByNameError> {
        let tag = self.lookup_name(name)?;
        self.entries
            .remove(&tag)
            .map(|e| {
                self.len = Length::UNDEFINED;
                e
            })
            .with_context(|| NoSuchDataElementAliasSnafu {
                tag,
                alias: name.to_string(),
            })
    }

    /// Modify the object by
    /// retaining only the DICOM data elements specified by the predicate.
    ///
    /// The elements are visited in ascending tag order,
    /// and those for which `f(&element)` returns `false` are removed.
    pub fn retain(&mut self, mut f: impl FnMut(&InMemElement<D>) -> bool) {
        self.entries.retain(|_, elem| f(elem));
        self.len = Length::UNDEFINED;
    }

    /// Obtain a temporary mutable reference to a DICOM value by tag,
    /// so that mutations can be applied within.
    ///
    /// If found, this method resets all related lengths recorded
    /// and returns `true`.
    /// Returns `false` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::{DataElement, VR, dicom_value};
    /// # use dicom_dictionary_std::tags;
    /// # use dicom_object::InMemDicomObject;
    /// let mut obj = InMemDicomObject::from_element_iter([
    ///     DataElement::new(tags::LOSSY_IMAGE_COMPRESSION_RATIO, VR::DS, dicom_value!(Strs, ["25"])),
    /// ]);
    ///
    /// // update lossy image compression ratio
    /// obj.update_value(tags::LOSSY_IMAGE_COMPRESSION_RATIO, |e| {
    ///     e.primitive_mut().unwrap().extend_str(["2.56"]);
    /// });
    ///
    /// assert_eq!(
    ///     obj.get(tags::LOSSY_IMAGE_COMPRESSION_RATIO).unwrap().value().to_str().unwrap(),
    ///     "25\\2.56"
    /// );
    /// ```
    pub fn update_value(
        &mut self,
        tag: Tag,
        f: impl FnMut(&mut Value<InMemDicomObject<D>, InMemFragment>),
    ) -> bool {
        self.invalidate_if_charset_changed(tag);
        if let Some(e) = self.entries.get_mut(&tag) {
            e.update_value(f);
            self.len = Length::UNDEFINED;
            true
        } else {
            false
        }
    }

    /// Obtain a temporary mutable reference to a DICOM value by AttributeSelector,
    /// so that mutations can be applied within.
    ///
    /// If found, this method resets all related lengths recorded
    /// and returns `true`.
    /// Returns `false` otherwise.
    ///
    /// See the documentation of [`AttributeSelector`] for more information
    /// on how to write attribute selectors.
    ///
    /// Note: Consider using [`apply`](ApplyOp::apply) when possible.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::{DataElement, VR, dicom_value, value::DataSetSequence};
    /// # use dicom_dictionary_std::tags;
    /// # use dicom_object::InMemDicomObject;
    /// # use dicom_core::ops::{AttributeAction, AttributeOp, ApplyOp};
    /// let mut dcm = InMemDicomObject::from_element_iter([
    ///     DataElement::new(
    ///         tags::OTHER_PATIENT_I_DS_SEQUENCE,
    ///         VR::SQ,
    ///         DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
    ///             DataElement::new(
    ///                 tags::PATIENT_ID,
    ///                 VR::LO,
    ///                 dicom_value!(Str, "1234")
    ///             )])
    ///         ])
    ///     ),
    /// ]);
    /// let selector = (
    ///     tags::OTHER_PATIENT_I_DS_SEQUENCE,
    ///     0,
    ///     tags::PATIENT_ID
    /// );
    ///
    /// // update referenced SOP instance UID for deidentification potentially
    /// dcm.update_value_at(*&selector, |e| {
    ///     let mut v = e.primitive_mut().unwrap();
    ///     *v = dicom_value!(Str, "abcd");
    /// });
    ///
    /// assert_eq!(
    ///     dcm.entry_at(*&selector).unwrap().value().to_str().unwrap(),
    ///     "abcd"
    /// );
    /// ```
    pub fn update_value_at(
        &mut self,
        selector: impl Into<AttributeSelector>,
        f: impl FnMut(&mut Value<InMemDicomObject<D>, InMemFragment>),
    ) -> Result<(), AtAccessError> {
        self.entry_at_mut(selector)
            .map(|e| e.update_value(f))
            .map(|_| {
                self.len = Length::UNDEFINED;
            })
    }

    /// Obtain the DICOM value by finding the element
    /// that matches the given selector.
    ///
    /// Returns an error if the respective element or any of its parents
    /// cannot be found.
    ///
    /// See the documentation of [`AttributeSelector`] for more information
    /// on how to write attribute selectors.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dicom_core::prelude::*;
    /// # use dicom_core::ops::AttributeSelector;
    /// # use dicom_dictionary_std::tags;
    /// # use dicom_object::InMemDicomObject;
    /// # let obj: InMemDicomObject = unimplemented!();
    /// let referenced_sop_instance_iod = obj.value_at(
    ///     (
    ///         tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
    ///         tags::REFERENCED_IMAGE_SEQUENCE,
    ///         tags::REFERENCED_SOP_INSTANCE_UID,
    ///     ))?
    ///     .to_str()?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn value_at(
        &self,
        selector: impl Into<AttributeSelector>,
    ) -> Result<&Value<InMemDicomObject<D>, InMemFragment>, AtAccessError> {
        let selector: AttributeSelector = selector.into();

        let mut obj = self;
        for (i, step) in selector.iter().enumerate() {
            match step {
                // reached the leaf
                AttributeSelectorStep::Tag(tag) => {
                    return obj.get(*tag).map(|e| e.value()).with_context(|| {
                        MissingLeafElementSnafu {
                            selector: selector.clone(),
                        }
                    });
                }
                // navigate further down
                AttributeSelectorStep::Nested { tag, item } => {
                    let e = obj
                        .entries
                        .get(tag)
                        .with_context(|| crate::MissingSequenceSnafu {
                            selector: selector.clone(),
                            step_index: i as u32,
                        })?;

                    // get items
                    let items = e.items().with_context(|| NotASequenceSnafu {
                        selector: selector.clone(),
                        step_index: i as u32,
                    })?;

                    // if item.length == i and action is a constructive action, append new item
                    obj =
                        items
                            .get(*item as usize)
                            .with_context(|| crate::MissingSequenceSnafu {
                                selector: selector.clone(),
                                step_index: i as u32,
                            })?;
                }
            }
        }

        unreachable!()
    }

    /// Change the 'specific_character_set' tag to ISO_IR 192, marking the dataset as UTF-8
    pub fn convert_to_utf8(&mut self) {
        self.put(DataElement::new(
            tags::SPECIFIC_CHARACTER_SET,
            VR::CS,
            "ISO_IR 192",
        ));
    }

    /// Get a DataElement by AttributeSelector
    ///
    /// If the element or other intermediate elements do not exist, the method will return an error.
    ///
    /// See the documentation of [`AttributeSelector`] for more information
    /// on how to write attribute selectors.
    ///
    /// If you only need the value, use [`value_at`](Self::value_at).
    pub fn entry_at(
        &self,
        selector: impl Into<AttributeSelector>,
    ) -> Result<&InMemElement<D>, AtAccessError> {
        let selector: AttributeSelector = selector.into();

        let mut obj = self;
        for (i, step) in selector.iter().enumerate() {
            match step {
                // reached the leaf
                AttributeSelectorStep::Tag(tag) => {
                    return obj.get(*tag).with_context(|| MissingLeafElementSnafu {
                        selector: selector.clone(),
                    })
                }
                // navigate further down
                AttributeSelectorStep::Nested { tag, item } => {
                    let e = obj
                        .entries
                        .get(tag)
                        .with_context(|| crate::MissingSequenceSnafu {
                            selector: selector.clone(),
                            step_index: i as u32,
                        })?;

                    // get items
                    let items = e.items().with_context(|| NotASequenceSnafu {
                        selector: selector.clone(),
                        step_index: i as u32,
                    })?;

                    // if item.length == i and action is a constructive action, append new item
                    obj =
                        items
                            .get(*item as usize)
                            .with_context(|| crate::MissingSequenceSnafu {
                                selector: selector.clone(),
                                step_index: i as u32,
                            })?;
                }
            }
        }

        unreachable!()
    }

    // Get a mutable reference to a particular entry by AttributeSelector
    //
    // Should be private for the same reason as `self.get_mut`
    fn entry_at_mut(
        &mut self,
        selector: impl Into<AttributeSelector>,
    ) -> Result<&mut InMemElement<D>, AtAccessError> {
        let selector: AttributeSelector = selector.into();

        let mut obj = self;
        for (i, step) in selector.iter().enumerate() {
            match step {
                // reached the leaf
                AttributeSelectorStep::Tag(tag) => {
                    return obj.get_mut(*tag).with_context(|| MissingLeafElementSnafu {
                        selector: selector.clone(),
                    })
                }
                // navigate further down
                AttributeSelectorStep::Nested { tag, item } => {
                    let e =
                        obj.entries
                            .get_mut(tag)
                            .with_context(|| crate::MissingSequenceSnafu {
                                selector: selector.clone(),
                                step_index: i as u32,
                            })?;

                    // get items
                    let items = e.items_mut().with_context(|| NotASequenceSnafu {
                        selector: selector.clone(),
                        step_index: i as u32,
                    })?;

                    // if item.length == i and action is a constructive action, append new item
                    obj = items.get_mut(*item as usize).with_context(|| {
                        crate::MissingSequenceSnafu {
                            selector: selector.clone(),
                            step_index: i as u32,
                        }
                    })?;
                }
            }
        }

        unreachable!()
    }

    /// Apply the given attribute operation on this object.
    ///
    /// For more complex updates, see [`update_value_at`].
    ///
    /// See the [`dicom_core::ops`] module
    /// for more information.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dicom_core::header::{DataElement, VR};
    /// # use dicom_core::value::PrimitiveValue;
    /// # use dicom_dictionary_std::tags;
    /// # use dicom_object::mem::*;
    /// # use dicom_object::ops::ApplyResult;
    /// use dicom_core::ops::{ApplyOp, AttributeAction, AttributeOp};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // given an in-memory DICOM object
    /// let mut obj = InMemDicomObject::from_element_iter([
    ///     DataElement::new(
    ///         tags::PATIENT_NAME,
    ///         VR::PN,
    ///         PrimitiveValue::from("Rosling^Hans")
    ///     ),
    /// ]);
    ///
    /// // apply patient name change
    /// obj.apply(AttributeOp::new(
    ///   tags::PATIENT_NAME,
    ///   AttributeAction::SetStr("Patient^Anonymous".into()),
    /// ))?;
    ///
    /// assert_eq!(
    ///     obj.element(tags::PATIENT_NAME)?.to_str()?,
    ///     "Patient^Anonymous",
    /// );
    /// # Ok(())
    /// # }
    /// ```
    fn apply(&mut self, op: AttributeOp) -> ApplyResult {
        let AttributeOp { selector, action } = op;
        let dict = self.dict.clone();

        let mut obj = self;
        for (i, step) in selector.iter().enumerate() {
            match step {
                // reached the leaf
                AttributeSelectorStep::Tag(tag) => return obj.apply_leaf(*tag, action),
                // navigate further down
                AttributeSelectorStep::Nested { tag, item } => {
                    if !obj.entries.contains_key(tag) {
                        // missing sequence, create it if action is constructive
                        if action.is_constructive() {
                            let vr = dict
                                .by_tag(*tag)
                                .and_then(|entry| entry.vr().exact())
                                .unwrap_or(VR::UN);

                            if vr != VR::SQ && vr != VR::UN {
                                return Err(ApplyError::NotASequence {
                                    selector: selector.clone(),
                                    step_index: i as u32,
                                });
                            }

                            obj.put(DataElement::new(*tag, vr, DataSetSequence::empty()));
                        } else {
                            return Err(ApplyError::MissingSequence {
                                selector: selector.clone(),
                                step_index: i as u32,
                            });
                        }
                    };

                    // get items
                    let items = obj
                        .entries
                        .get_mut(tag)
                        .expect("sequence element should exist at this point")
                        .items_mut()
                        .ok_or_else(|| ApplyError::NotASequence {
                            selector: selector.clone(),
                            step_index: i as u32,
                        })?;

                    // if item.length == i and action is a constructive action, append new item
                    obj = if items.len() == *item as usize && action.is_constructive() {
                        items.push(InMemDicomObject::new_empty_with_dict(dict.clone()));
                        items.last_mut().unwrap()
                    } else {
                        items.get_mut(*item as usize).ok_or_else(|| {
                            ApplyError::MissingSequence {
                                selector: selector.clone(),
                                step_index: i as u32,
                            }
                        })?
                    };
                }
            }
        }
        unreachable!()
    }

    fn apply_leaf(&mut self, tag: Tag, action: AttributeAction) -> ApplyResult {
        self.invalidate_if_charset_changed(tag);
        match action {
            AttributeAction::Remove => {
                self.remove_element(tag);
                Ok(())
            }
            AttributeAction::Empty => {
                if let Some(e) = self.entries.get_mut(&tag) {
                    let vr = e.vr();
                    // replace element
                    *e = DataElement::empty(tag, vr);
                    self.len = Length::UNDEFINED;
                }
                Ok(())
            }
            AttributeAction::SetVr(new_vr) => {
                if let Some(e) = self.entries.remove(&tag) {
                    let (header, value) = e.into_parts();
                    let e = DataElement::new(header.tag, new_vr, value);
                    self.put(e);
                } else {
                    self.put(DataElement::empty(tag, new_vr));
                }
                Ok(())
            }
            AttributeAction::Set(new_value) => {
                self.apply_change_value_impl(tag, new_value);
                Ok(())
            }
            AttributeAction::SetStr(string) => {
                let new_value = PrimitiveValue::from(&*string);
                self.apply_change_value_impl(tag, new_value);
                Ok(())
            }
            AttributeAction::SetIfMissing(new_value) => {
                if self.get(tag).is_none() {
                    self.apply_change_value_impl(tag, new_value);
                }
                Ok(())
            }
            AttributeAction::SetStrIfMissing(string) => {
                if self.get(tag).is_none() {
                    let new_value = PrimitiveValue::from(&*string);
                    self.apply_change_value_impl(tag, new_value);
                }
                Ok(())
            }
            AttributeAction::Replace(new_value) => {
                if self.get(tag).is_some() {
                    self.apply_change_value_impl(tag, new_value);
                }
                Ok(())
            }
            AttributeAction::ReplaceStr(string) => {
                if self.get(tag).is_some() {
                    let new_value = PrimitiveValue::from(&*string);
                    self.apply_change_value_impl(tag, new_value);
                }
                Ok(())
            }
            AttributeAction::PushStr(string) => self.apply_push_str_impl(tag, string),
            AttributeAction::PushI32(integer) => self.apply_push_i32_impl(tag, integer),
            AttributeAction::PushU32(integer) => self.apply_push_u32_impl(tag, integer),
            AttributeAction::PushI16(integer) => self.apply_push_i16_impl(tag, integer),
            AttributeAction::PushU16(integer) => self.apply_push_u16_impl(tag, integer),
            AttributeAction::PushF32(number) => self.apply_push_f32_impl(tag, number),
            AttributeAction::PushF64(number) => self.apply_push_f64_impl(tag, number),
            AttributeAction::Truncate(limit) => {
                self.update_value(tag, |value| value.truncate(limit));
                Ok(())
            }
            _ => UnsupportedActionSnafu.fail(),
        }
    }

    fn apply_change_value_impl(&mut self, tag: Tag, new_value: PrimitiveValue) {
        self.invalidate_if_charset_changed(tag);

        if let Some(e) = self.entries.get_mut(&tag) {
            let vr = e.vr();
            // handle edge case: if VR is SQ and suggested value is empty,
            // then create an empty data set sequence
            let new_value = if vr == VR::SQ && new_value.is_empty() {
                DataSetSequence::empty().into()
            } else {
                Value::from(new_value)
            };
            *e = DataElement::new(tag, vr, new_value);
            self.len = Length::UNDEFINED;
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::UN);
            // insert element

            // handle edge case: if VR is SQ and suggested value is empty,
            // then create an empty data set sequence
            let new_value = if vr == VR::SQ && new_value.is_empty() {
                DataSetSequence::empty().into()
            } else {
                Value::from(new_value)
            };

            self.put(DataElement::new(tag, vr, new_value));
        }
    }

    fn invalidate_if_charset_changed(&mut self, tag: Tag) {
        if tag == tags::SPECIFIC_CHARACTER_SET {
            self.charset_changed = true;
        }
    }

    fn apply_push_str_impl(&mut self, tag: Tag, string: Cow<'static, str>) -> ApplyResult {
        if let Some(e) = self.entries.remove(&tag) {
            let (header, value) = e.into_parts();
            match value {
                Value::Primitive(mut v) => {
                    self.invalidate_if_charset_changed(tag);
                    // extend value
                    v.extend_str([string]).context(ModifySnafu)?;
                    // reinsert element
                    self.put(DataElement::new(tag, header.vr, v));
                    Ok(())
                }

                Value::PixelSequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::PixelSequence,
                }
                .fail(),
                Value::Sequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::DataSetSequence,
                }
                .fail(),
            }
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::UN);
            // insert element
            self.put(DataElement::new(tag, vr, PrimitiveValue::from(&*string)));
            Ok(())
        }
    }

    fn apply_push_i32_impl(&mut self, tag: Tag, integer: i32) -> ApplyResult {
        if let Some(e) = self.entries.remove(&tag) {
            let (header, value) = e.into_parts();
            match value {
                Value::Primitive(mut v) => {
                    // extend value
                    v.extend_i32([integer]).context(ModifySnafu)?;
                    // reinsert element
                    self.put(DataElement::new(tag, header.vr, v));
                    Ok(())
                }

                Value::PixelSequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::PixelSequence,
                }
                .fail(),
                Value::Sequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::DataSetSequence,
                }
                .fail(),
            }
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::SL);
            // insert element
            self.put(DataElement::new(tag, vr, PrimitiveValue::from(integer)));
            Ok(())
        }
    }

    fn apply_push_u32_impl(&mut self, tag: Tag, integer: u32) -> ApplyResult {
        if let Some(e) = self.entries.remove(&tag) {
            let (header, value) = e.into_parts();
            match value {
                Value::Primitive(mut v) => {
                    // extend value
                    v.extend_u32([integer]).context(ModifySnafu)?;
                    // reinsert element
                    self.put(DataElement::new(tag, header.vr, v));
                    Ok(())
                }

                Value::PixelSequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::PixelSequence,
                }
                .fail(),
                Value::Sequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::DataSetSequence,
                }
                .fail(),
            }
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::UL);
            // insert element
            self.put(DataElement::new(tag, vr, PrimitiveValue::from(integer)));
            Ok(())
        }
    }

    fn apply_push_i16_impl(&mut self, tag: Tag, integer: i16) -> ApplyResult {
        if let Some(e) = self.entries.remove(&tag) {
            let (header, value) = e.into_parts();
            match value {
                Value::Primitive(mut v) => {
                    // extend value
                    v.extend_i16([integer]).context(ModifySnafu)?;
                    // reinsert element
                    self.put(DataElement::new(tag, header.vr, v));
                    Ok(())
                }

                Value::PixelSequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::PixelSequence,
                }
                .fail(),
                Value::Sequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::DataSetSequence,
                }
                .fail(),
            }
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::SS);
            // insert element
            self.put(DataElement::new(tag, vr, PrimitiveValue::from(integer)));
            Ok(())
        }
    }

    fn apply_push_u16_impl(&mut self, tag: Tag, integer: u16) -> ApplyResult {
        if let Some(e) = self.entries.remove(&tag) {
            let (header, value) = e.into_parts();
            match value {
                Value::Primitive(mut v) => {
                    // extend value
                    v.extend_u16([integer]).context(ModifySnafu)?;
                    // reinsert element
                    self.put(DataElement::new(tag, header.vr, v));
                    Ok(())
                }

                Value::PixelSequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::PixelSequence,
                }
                .fail(),
                Value::Sequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::DataSetSequence,
                }
                .fail(),
            }
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::US);
            // insert element
            self.put(DataElement::new(tag, vr, PrimitiveValue::from(integer)));
            Ok(())
        }
    }

    fn apply_push_f32_impl(&mut self, tag: Tag, number: f32) -> ApplyResult {
        if let Some(e) = self.entries.remove(&tag) {
            let (header, value) = e.into_parts();
            match value {
                Value::Primitive(mut v) => {
                    // extend value
                    v.extend_f32([number]).context(ModifySnafu)?;
                    // reinsert element
                    self.put(DataElement::new(tag, header.vr, v));
                    Ok(())
                }

                Value::PixelSequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::PixelSequence,
                }
                .fail(),
                Value::Sequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::DataSetSequence,
                }
                .fail(),
            }
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::FL);
            // insert element
            self.put(DataElement::new(tag, vr, PrimitiveValue::from(number)));
            Ok(())
        }
    }

    fn apply_push_f64_impl(&mut self, tag: Tag, number: f64) -> ApplyResult {
        if let Some(e) = self.entries.remove(&tag) {
            let (header, value) = e.into_parts();
            match value {
                Value::Primitive(mut v) => {
                    // extend value
                    v.extend_f64([number]).context(ModifySnafu)?;
                    // reinsert element
                    self.put(DataElement::new(tag, header.vr, v));
                    Ok(())
                }

                Value::PixelSequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::PixelSequence,
                }
                .fail(),
                Value::Sequence(..) => IncompatibleTypesSnafu {
                    kind: ValueType::DataSetSequence,
                }
                .fail(),
            }
        } else {
            // infer VR from tag
            let vr = dicom_dictionary_std::StandardDataDictionary
                .by_tag(tag)
                .and_then(|entry| entry.vr().exact())
                .unwrap_or(VR::FD);
            // insert element
            self.put(DataElement::new(tag, vr, PrimitiveValue::from(number)));
            Ok(())
        }
    }

    /// Write this object's data set into the given writer,
    /// with the given encoder specifications,
    /// without preamble, magic code, nor file meta group.
    ///
    /// The text encoding to use will be the default character set
    /// until _Specific Character Set_ is found in the data set,
    /// in which then that character set will be used.
    ///
    /// Note: [`write_dataset_with_ts`] and [`write_dataset_with_ts_cs`]
    /// may be easier to use and _will_ apply a dataset adapter (such as
    /// DeflatedExplicitVRLittleEndian (1.2.840.10008.1.2.99)) whereas this
    /// method will _not_
    ///
    /// [`write_dataset_with_ts`]: #method.write_dataset_with_ts
    /// [`write_dataset_with_ts_cs`]: #method.write_dataset_with_ts_cs
    pub fn write_dataset<W, E>(&self, to: W, encoder: E) -> Result<(), WriteError>
    where
        W: Write,
        E: EncodeTo<W>,
    {
        // prepare data set writer
        let mut dset_writer = DataSetWriter::new(to, encoder);
        let required_options = IntoTokensOptions::new(self.charset_changed);
        // write object
        dset_writer
            .write_sequence(self.into_tokens_with_options(required_options))
            .context(PrintDataSetSnafu)?;

        Ok(())
    }

    /// Write this object's data set into the given printer,
    /// with the specified transfer syntax and character set,
    /// without preamble, magic code, nor file meta group.
    ///
    /// If the attribute _Specific Character Set_ is found in the data set,
    /// the last parameter is overridden accordingly.
    /// See also [`write_dataset_with_ts`](Self::write_dataset_with_ts).
    pub fn write_dataset_with_ts_cs<W>(
        &self,
        to: W,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<(), WriteError>
    where
        W: Write,
    {
        if let Codec::Dataset(Some(adapter)) = ts.codec() {
            let adapter = adapter.adapt_writer(Box::new(to));
            // prepare data set writer
            let mut dset_writer = DataSetWriter::with_ts(adapter, ts).context(CreatePrinterSnafu)?;

            // write object
            dset_writer
                .write_sequence(self.into_tokens())
                .context(PrintDataSetSnafu)?;

            Ok(())
        } else {
            // prepare data set writer
            let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinterSnafu)?;

            // write object
            dset_writer
                .write_sequence(self.into_tokens())
                .context(PrintDataSetSnafu)?;

            Ok(())
        }
    }

    /// Write this object's data set into the given writer,
    /// with the specified transfer syntax,
    /// without preamble, magic code, nor file meta group.
    ///
    /// The default character set is assumed
    /// until the _Specific Character Set_ is found in the data set,
    /// after which the text encoder is overridden accordingly.
    pub fn write_dataset_with_ts<W>(&self, to: W, ts: &TransferSyntax) -> Result<(), WriteError>
    where
        W: Write,
    {
        self.write_dataset_with_ts_cs(to, ts, SpecificCharacterSet::default())
    }

    /// Encapsulate this object to contain a file meta group
    /// as described exactly by the given table.
    ///
    /// **Note:** this method will not adjust the file meta group
    /// to be semantically valid for the object.
    /// Namely, the _Media Storage SOP Instance UID_
    /// and _Media Storage SOP Class UID_
    /// are not updated based on the receiving data set.
    pub fn with_exact_meta(self, meta: FileMetaTable) -> FileDicomObject<Self> {
        FileDicomObject { meta, obj: self }
    }

    /// Encapsulate this object to contain a file meta group,
    /// created through the given file meta table builder.
    ///
    /// A complete file meta group should provide
    /// the _Transfer Syntax UID_,
    /// the _Media Storage SOP Instance UID_,
    /// and the _Media Storage SOP Class UID_.
    /// The last two will be filled with the values of
    /// _SOP Instance UID_ and _SOP Class UID_
    /// if they are present in this object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dicom_core::{DataElement, VR};
    /// # use dicom_dictionary_std::tags;
    /// # use dicom_dictionary_std::uids;
    /// use dicom_object::{InMemDicomObject, meta::FileMetaTableBuilder};
    ///
    /// let obj = InMemDicomObject::from_element_iter([
    ///     DataElement::new(tags::SOP_CLASS_UID, VR::UI, uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE),
    ///     DataElement::new(tags::SOP_INSTANCE_UID, VR::UI, "2.25.60156688944589400766024286894543900794"),
    ///     // ...
    /// ]);
    ///
    /// let obj = obj.with_meta(FileMetaTableBuilder::new()
    ///     .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN))?;
    ///
    /// // can now save everything to a file
    /// let meta = obj.write_to_file("out.dcm")?;
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn with_meta(
        self,
        mut meta: FileMetaTableBuilder,
    ) -> Result<FileDicomObject<Self>, WithMetaError> {
        if let Some(elem) = self.get(tags::SOP_INSTANCE_UID) {
            meta = meta.media_storage_sop_instance_uid(
                elem.value().to_str().context(PrepareMetaTableSnafu)?,
            );
        }
        if let Some(elem) = self.get(tags::SOP_CLASS_UID) {
            meta = meta
                .media_storage_sop_class_uid(elem.value().to_str().context(PrepareMetaTableSnafu)?);
        }
        Ok(FileDicomObject {
            meta: meta.build().context(BuildMetaTableSnafu)?,
            obj: self,
        })
    }

    /// Obtain an iterator over the elements of this object.
    pub fn iter(&self) -> impl Iterator<Item = &InMemElement<D>> + '_ {
        self.into_iter()
    }

    /// Obtain an iterator over the tags of the object's elements.
    pub fn tags(&self) -> impl Iterator<Item = Tag> + '_ {
        self.entries.keys().copied()
    }

    // private methods

    /// Build an object by consuming a data set parser.
    fn build_object<I>(
        dataset: &mut I,
        dict: D,
        in_item: bool,
        len: Length,
        read_until: Option<Tag>,
    ) -> Result<Self, ReadError>
    where
        I: ?Sized + Iterator<Item = ParserResult<DataToken>>,
    {
        let mut entries: BTreeMap<Tag, InMemElement<D>> = BTreeMap::new();
        // perform a structured parsing of incoming tokens
        while let Some(token) = dataset.next() {
            let elem = match token.context(ReadTokenSnafu)? {
                DataToken::PixelSequenceStart => {
                    // stop reading if reached `read_until` tag
                    if read_until
                        .map(|t| t <= Tag(0x7fe0, 0x0010))
                        .unwrap_or(false)
                    {
                        break;
                    }
                    let value = InMemDicomObject::build_encapsulated_data(&mut *dataset)?;
                    DataElement::new(Tag(0x7fe0, 0x0010), VR::OB, value)
                }
                DataToken::ElementHeader(header) => {
                    // stop reading if reached `read_until` tag
                    if read_until.map(|t| t <= header.tag).unwrap_or(false) {
                        break;
                    }

                    // fetch respective value, place it in the entries
                    let next_token = dataset.next().context(MissingElementValueSnafu)?;
                    match next_token.context(ReadTokenSnafu)? {
                        DataToken::PrimitiveValue(v) => InMemElement::new_with_len(
                            header.tag,
                            header.vr,
                            header.len,
                            Value::Primitive(v),
                        ),
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

                    // delegate sequence building to another function
                    let items = Self::build_sequence(tag, len, &mut *dataset, &dict)?;
                    DataElement::new_with_len(
                        tag,
                        VR::SQ,
                        len,
                        Value::Sequence(DataSetSequence::new(items, len)),
                    )
                }
                DataToken::ItemEnd if in_item => {
                    // end of item, leave now
                    return Ok(InMemDicomObject {
                        entries,
                        dict,
                        len,
                        charset_changed: false,
                    });
                }
                token => return UnexpectedTokenSnafu { token }.fail(),
            };
            entries.insert(elem.tag(), elem);
        }

        Ok(InMemDicomObject {
            entries,
            dict,
            len,
            charset_changed: false,
        })
    }

    /// Build an encapsulated pixel data by collecting all fragments into an
    /// in-memory DICOM value.
    fn build_encapsulated_data<I>(
        dataset: I,
    ) -> Result<Value<InMemDicomObject<D>, InMemFragment>, ReadError>
    where
        I: Iterator<Item = ParserResult<DataToken>>,
    {
        // continue fetching tokens to retrieve:
        // - the offset table
        // - the various compressed fragments
        //
        // Note: as there is still no standard way to represent this in memory,
        // this code will currently flatten all compressed fragments into a
        // single vector.

        let mut offset_table = None;

        let mut fragments = C::new();

        for token in dataset {
            match token.context(ReadTokenSnafu)? {
                DataToken::OffsetTable(table) => {
                    offset_table = Some(table);
                }
                DataToken::ItemValue(data) => {
                    fragments.push(data);
                }
                DataToken::ItemEnd => {
                    // at the end of the first item ensure the presence of
                    // an empty offset_table here, so that the next items
                    // are seen as compressed fragments
                    if offset_table.is_none() {
                        offset_table = Some(Vec::new())
                    }
                }
                DataToken::ItemStart { len: _ } => { /* no-op */ }
                DataToken::SequenceEnd => {
                    // end of pixel data
                    break;
                }
                // the following variants are unexpected
                token @ DataToken::ElementHeader(_)
                | token @ DataToken::PixelSequenceStart
                | token @ DataToken::SequenceStart { .. }
                | token @ DataToken::PrimitiveValue(_) => {
                    return UnexpectedTokenSnafu { token }.fail();
                }
            }
        }

        Ok(Value::PixelSequence(PixelFragmentSequence::new(
            offset_table.unwrap_or_default(),
            fragments,
        )))
    }

    /// Build a DICOM sequence by consuming a data set parser.
    fn build_sequence<I>(
        _tag: Tag,
        _len: Length,
        dataset: &mut I,
        dict: &D,
    ) -> Result<C<InMemDicomObject<D>>, ReadError>
    where
        I: ?Sized + Iterator<Item = ParserResult<DataToken>>,
    {
        let mut items: C<_> = SmallVec::new();
        while let Some(token) = dataset.next() {
            match token.context(ReadTokenSnafu)? {
                DataToken::ItemStart { len } => {
                    items.push(Self::build_object(
                        &mut *dataset,
                        dict.clone(),
                        true,
                        len,
                        None,
                    )?);
                }
                DataToken::SequenceEnd => {
                    return Ok(items);
                }
                token => return UnexpectedTokenSnafu { token }.fail(),
            };
        }

        // iterator fully consumed without a sequence delimiter
        PrematureEndSnafu.fail()
    }

    fn lookup_name(&self, name: &str) -> Result<Tag, AccessByNameError> {
        self.dict
            .by_name(name)
            .context(NoSuchAttributeNameSnafu { name })
            .map(|e| e.tag())
    }
}

impl<D> ApplyOp for InMemDicomObject<D>
where
    D: DataDictionary,
    D: Clone,
{
    type Err = ApplyError;

    #[inline]
    fn apply(&mut self, op: AttributeOp) -> ApplyResult {
        self.apply(op)
    }
}

impl<'a, D> IntoIterator for &'a InMemDicomObject<D> {
    type Item = &'a InMemElement<D>;
    type IntoIter = ::std::collections::btree_map::Values<'a, Tag, InMemElement<D>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.values()
    }
}

impl<D> IntoIterator for InMemDicomObject<D> {
    type Item = InMemElement<D>;
    type IntoIter = Iter<D>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            inner: self.entries.into_iter(),
        }
    }
}

/// Base iterator type for an in-memory DICOM object.
#[derive(Debug)]
pub struct Iter<D> {
    inner: ::std::collections::btree_map::IntoIter<Tag, InMemElement<D>>,
}

impl<D> Iterator for Iter<D> {
    type Item = InMemElement<D>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|x| x.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn count(self) -> usize {
        self.inner.count()
    }
}

impl<D> Extend<InMemElement<D>> for InMemDicomObject<D> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = InMemElement<D>>,
    {
        self.len = Length::UNDEFINED;
        self.entries.extend(iter.into_iter().map(|e| (e.tag(), e)))
    }
}

fn even_len(l: u32) -> u32 {
    (l + 1) & !1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_file;
    use byteordered::Endianness;
    use dicom_core::chrono::FixedOffset;
    use dicom_core::value::{DicomDate, DicomDateTime, DicomTime};
    use dicom_core::{dicom_value, header::DataElementHeader};
    use dicom_encoding::{
        decode::{basic::BasicDecoder, implicit_le::ImplicitVRLittleEndianDecoder},
        encode::{implicit_le::ImplicitVRLittleEndianEncoder, EncoderFor},
    };
    use dicom_parser::StatefulDecoder;

    fn assert_obj_eq<D>(obj1: &InMemDicomObject<D>, obj2: &InMemDicomObject<D>)
    where
        D: std::fmt::Debug,
    {
        // debug representation because it makes a stricter comparison and
        // assumes that Undefined lengths are equal.
        assert_eq!(format!("{:?}", obj1), format!("{:?}", obj2))
    }

    #[test]
    fn inmem_object_compare() {
        let mut obj1 = InMemDicomObject::new_empty();
        let mut obj2 = InMemDicomObject::new_empty();
        assert_eq!(obj1, obj2);
        let empty_patient_name = DataElement::empty(Tag(0x0010, 0x0010), VR::PN);
        obj1.put(empty_patient_name.clone());
        assert_ne!(obj1, obj2);
        obj2.put(empty_patient_name.clone());
        assert_obj_eq(&obj1, &obj2);
    }

    #[test]
    fn inmem_object_read_dataset() {
        let data_in = [
            0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
            0x08, 0x00, 0x00, 0x00, // Length: 8
            b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
        ];

        let decoder = ImplicitVRLittleEndianDecoder::default();
        let text = SpecificCharacterSet::default();
        let mut cursor = &data_in[..];
        let parser = StatefulDecoder::new(
            &mut cursor,
            decoder,
            BasicDecoder::new(Endianness::Little),
            text,
        );

        let obj = InMemDicomObject::read_dataset(parser).unwrap();

        let mut gt = InMemDicomObject::new_empty();

        let patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            dicom_value!(Strs, ["Doe^John"]),
        );
        gt.put(patient_name);

        assert_eq!(obj, gt);
    }

    #[test]
    fn inmem_object_read_dataset_with_ts_cs() {
        let data_in = [
            0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
            0x08, 0x00, 0x00, 0x00, // Length: 8
            b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
        ];

        let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2").unwrap();
        let cs = SpecificCharacterSet::default();
        let mut cursor = &data_in[..];

        let obj = InMemDicomObject::read_dataset_with_dict_ts_cs(
            &mut cursor,
            StandardDataDictionary,
            &ts,
            cs,
        )
        .unwrap();

        let mut gt = InMemDicomObject::new_empty();

        let patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            dicom_value!(Strs, ["Doe^John"]),
        );
        gt.put(patient_name);

        assert_eq!(obj, gt);
    }

    /// Reading a data set
    /// saves the original length of a text element.
    #[test]
    fn inmem_object_read_dataset_saves_len() {
        let data_in = [
            // SpecificCharacterSet (0008,0005)
            0x08, 0x00, 0x05, 0x00, //
            // Length: 10
            0x0a, 0x00, 0x00, 0x00, //
            b'I', b'S', b'O', b'_', b'I', b'R', b' ', b'1', b'0', b'0',
            // ReferringPhysicianName (0008,0090)
            0x08, 0x00, 0x90, 0x00, //
            // Length: 12
            0x0c, 0x00, 0x00, 0x00, b'S', b'i', b'm', 0xF5, b'e', b's', b'^', b'J', b'o', 0xE3,
            b'o', b' ',
        ];

        let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2").unwrap();
        let mut cursor = &data_in[..];

        let obj =
            InMemDicomObject::read_dataset_with_dict_ts(&mut cursor, StandardDataDictionary, &ts)
                .unwrap();

        let physician_name = obj.element(Tag(0x0008, 0x0090)).unwrap();
        assert_eq!(physician_name.header().len, Length(12));
        assert_eq!(physician_name.value().to_str().unwrap(), "Simões^João");
    }

    #[test]
    fn inmem_object_write_dataset() {
        let mut obj = InMemDicomObject::new_empty();

        let patient_name =
            DataElement::new(Tag(0x0010, 0x0010), VR::PN, dicom_value!(Str, "Doe^John"));
        obj.put(patient_name);

        let mut out = Vec::new();

        let printer = EncoderFor::new(ImplicitVRLittleEndianEncoder::default());

        obj.write_dataset(&mut out, printer).unwrap();

        assert_eq!(
            out,
            &[
                0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
                0x08, 0x00, 0x00, 0x00, // Length: 8
                b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
            ][..],
        );
    }

    #[test]
    fn inmem_object_write_dataset_with_ts() {
        let mut obj = InMemDicomObject::new_empty();

        let patient_name =
            DataElement::new(Tag(0x0010, 0x0010), VR::PN, dicom_value!(Str, "Doe^John"));
        obj.put(patient_name);

        let mut out = Vec::new();

        let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1").unwrap();

        obj.write_dataset_with_ts(&mut out, &ts).unwrap();

        assert_eq!(
            out,
            &[
                0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
                b'P', b'N', // VR: PN
                0x08, 0x00, // Length: 8
                b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
            ][..],
        );
    }

    #[test]
    fn inmem_object_write_dataset_with_ts_cs() {
        let mut obj = InMemDicomObject::new_empty();

        let patient_name =
            DataElement::new(Tag(0x0010, 0x0010), VR::PN, dicom_value!(Str, "Doe^John"));
        obj.put(patient_name);

        let mut out = Vec::new();

        let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2").unwrap();
        let cs = SpecificCharacterSet::default();

        obj.write_dataset_with_ts_cs(&mut out, &ts, cs).unwrap();

        assert_eq!(
            out,
            &[
                0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
                0x08, 0x00, 0x00, 0x00, // Length: 8
                b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
            ][..],
        );
    }

    /// writing a DICOM date time into an object
    /// should include value padding
    #[test]
    fn inmem_object_write_datetime_odd() {
        let mut obj = InMemDicomObject::new_empty();

        // add a number that will be encoded in text
        let instance_number =
            DataElement::new(Tag(0x0020, 0x0013), VR::IS, PrimitiveValue::from(1_i32));
        obj.put(instance_number);

        // add a date time
        let dt = DicomDateTime::from_date_and_time_with_time_zone(
            DicomDate::from_ymd(2022, 11, 22).unwrap(),
            DicomTime::from_hms(18, 09, 35).unwrap(),
            FixedOffset::east_opt(3600).unwrap(),
        )
        .unwrap();
        let instance_coercion_date_time =
            DataElement::new(Tag(0x0008, 0x0015), VR::DT, dicom_value!(DateTime, dt));
        obj.put(instance_coercion_date_time);

        // explicit VR Little Endian
        let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1").unwrap();

        let mut out = Vec::new();
        obj.write_dataset_with_ts(&mut out, &ts)
            .expect("should write DICOM data without errors");

        assert_eq!(
            out,
            &[
                // instance coercion date time
                0x08, 0x00, 0x15, 0x00, // Tag(0x0008, 0x0015)
                b'D', b'T', // VR: DT
                0x14, 0x00, // Length: 20 bytes
                b'2', b'0', b'2', b'2', b'1', b'1', b'2', b'2', // date
                b'1', b'8', b'0', b'9', b'3', b'5', // time
                b'+', b'0', b'1', b'0', b'0', // offset
                b' ', // padding to even length
                // instance number
                0x20, 0x00, 0x13, 0x00, // Tag(0x0020, 0x0013)
                b'I', b'S', // VR: IS
                0x02, 0x00, // Length: 2 bytes
                b'1', b' ' // 1, with padding
            ][..],
        );
    }

    /// Writes a file from scratch
    /// and opens it to check that the data is equivalent.
    #[test]
    fn inmem_write_to_file_with_meta() {
        let sop_uid = "1.4.645.212121";
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            dicom_value!(Strs, ["Doe^John"]),
        ));
        obj.put(DataElement::new(
            Tag(0x0008, 0x0060),
            VR::CS,
            dicom_value!(Strs, ["CR"]),
        ));
        obj.put(DataElement::new(
            Tag(0x0008, 0x0018),
            VR::UI,
            dicom_value!(Strs, [sop_uid]),
        ));

        let file_object = obj
            .with_meta(
                FileMetaTableBuilder::default()
                    // Explicit VR Little Endian
                    .transfer_syntax("1.2.840.10008.1.2.1")
                    // Computed Radiography image storage
                    .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
                    .media_storage_sop_instance_uid(sop_uid),
            )
            .unwrap();

        // create temporary file path and write object to that file
        let dir = tempfile::tempdir().unwrap();
        let mut file_path = dir.into_path();
        file_path.push(format!("{}.dcm", sop_uid));

        file_object.write_to_file(&file_path).unwrap();

        // read the file back to validate the outcome
        let saved_object = open_file(file_path).unwrap();
        assert_eq!(file_object, saved_object);
    }

    /// Creating a file DICOM object from an in-mem DICOM object
    /// infers the SOP instance UID.
    #[test]
    fn inmem_with_meta_infers_sop_instance_uid() {
        let sop_uid = "1.4.645.252521";
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            tags::SOP_INSTANCE_UID,
            VR::UI,
            PrimitiveValue::from(sop_uid),
        ));

        let file_object = obj
            .with_meta(
                // Media Storage SOP Instance UID deliberately not set
                FileMetaTableBuilder::default()
                    // Explicit VR Little Endian
                    .transfer_syntax("1.2.840.10008.1.2.1")
                    // Computed Radiography image storage
                    .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1"),
            )
            .unwrap();

        let meta = file_object.meta();

        assert_eq!(
            meta.media_storage_sop_instance_uid
                .trim_end_matches(|c| c == '\0'),
            sop_uid.trim_end_matches(|c| c == '\0'),
        );
    }

    /// Write a file from scratch, with exact file meta table.
    #[test]
    fn inmem_write_to_file_with_exact_meta() {
        let sop_uid = "1.4.645.212121";
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            dicom_value!(Strs, ["Doe^John"]),
        ));
        obj.put(DataElement::new(
            Tag(0x0008, 0x0060),
            VR::CS,
            dicom_value!(Strs, ["CR"]),
        ));
        obj.put(DataElement::new(
            Tag(0x0008, 0x0018),
            VR::UI,
            dicom_value!(Strs, [sop_uid]),
        ));

        let file_object = obj.with_exact_meta(
            FileMetaTableBuilder::default()
                // Explicit VR Little Endian
                .transfer_syntax("1.2.840.10008.1.2.1")
                // Computed Radiography image storage
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
                .media_storage_sop_instance_uid(sop_uid)
                .build()
                .unwrap(),
        );

        // create temporary file path and write object to that file
        let dir = tempfile::tempdir().unwrap();
        let mut file_path = dir.into_path();
        file_path.push(format!("{}.dcm", sop_uid));

        file_object.write_to_file(&file_path).unwrap();

        // read the file back to validate the outcome
        let saved_object = open_file(file_path).unwrap();
        assert_eq!(file_object, saved_object);
    }

    #[test]
    fn inmem_object_get() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        let elem1 = (&obj).element(Tag(0x0010, 0x0010)).unwrap();
        assert_eq!(elem1, &another_patient_name);
    }

    #[test]
    fn infer_media_sop_from_dataset_sop_elements() {
        let sop_instance_uid = "1.4.645.313131";
        let sop_class_uid = "1.2.840.10008.5.1.4.1.1.2";
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            Tag(0x0008, 0x0018),
            VR::UI,
            dicom_value!(Strs, [sop_instance_uid]),
        ));
        obj.put(DataElement::new(
            Tag(0x0008, 0x0016),
            VR::UI,
            dicom_value!(Strs, [sop_class_uid]),
        ));

        let file_object = obj.with_exact_meta(
            FileMetaTableBuilder::default()
                .transfer_syntax("1.2.840.10008.1.2.1")
                // Media Storage SOP Class and Instance UIDs are missing and set to an empty string
                .media_storage_sop_class_uid("")
                .media_storage_sop_instance_uid("")
                .build()
                .unwrap(),
        );

        // create temporary file path and write object to that file
        let dir = tempfile::tempdir().unwrap();
        let mut file_path = dir.into_path();
        file_path.push(format!("{}.dcm", sop_instance_uid));

        file_object.write_to_file(&file_path).unwrap();

        // read the file back to validate the outcome
        let saved_object = open_file(file_path).unwrap();

        // verify that the empty string media storage sop instance and class UIDs have been inferred from the sop instance and class UID
        assert_eq!(
            saved_object.meta().media_storage_sop_instance_uid(),
            sop_instance_uid
        );
        assert_eq!(
            saved_object.meta().media_storage_sop_class_uid(),
            sop_class_uid
        );
    }

    #[test]
    fn inmem_object_get_opt() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        let elem1 = obj.element_opt(Tag(0x0010, 0x0010)).unwrap();
        assert_eq!(elem1, Some(&another_patient_name));

        // try a missing element, should return None
        assert_eq!(obj.element_opt(Tag(0x0010, 0x0020)).unwrap(), None);
    }

    #[test]
    fn inmem_object_get_by_name() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        let elem1 = (&obj).element_by_name("PatientName").unwrap();
        assert_eq!(elem1, &another_patient_name);
    }

    #[test]
    fn inmem_object_get_by_name_opt() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        let elem1 = obj.element_by_name_opt("PatientName").unwrap();
        assert_eq!(elem1, Some(&another_patient_name));

        // try a missing element, should return None
        assert_eq!(obj.element_by_name_opt("PatientID").unwrap(), None);
    }

    #[test]
    fn inmem_object_take_element() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        let elem1 = obj.take_element(Tag(0x0010, 0x0010)).unwrap();
        assert_eq!(elem1, another_patient_name);
        assert!(matches!(
            obj.take_element(Tag(0x0010, 0x0010)),
            Err(AccessError::NoSuchDataElementTag {
                tag: Tag(0x0010, 0x0010),
                ..
            })
        ));
    }

    #[test]
    fn inmem_object_take_element_by_name() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        let elem1 = obj.take_element_by_name("PatientName").unwrap();
        assert_eq!(elem1, another_patient_name);
        assert!(matches!(
            obj.take_element_by_name("PatientName"),
            Err(AccessByNameError::NoSuchDataElementAlias {
                tag: Tag(0x0010, 0x0010),
                alias,
                ..
            }) if alias == "PatientName"));
    }

    #[test]
    fn inmem_object_remove_element() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        assert!(obj.remove_element(Tag(0x0010, 0x0010)));
        assert_eq!(obj.remove_element(Tag(0x0010, 0x0010)), false);
    }

    #[test]
    fn inmem_object_remove_element_by_name() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(another_patient_name.clone());
        assert!(obj.remove_element_by_name("PatientName").unwrap());
        assert_eq!(obj.remove_element_by_name("PatientName").unwrap(), false);
    }

    /// Elements are traversed in tag order.
    #[test]
    fn inmem_traverse_elements() {
        let sop_uid = "1.4.645.212121";
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            dicom_value!(Strs, ["Doe^John"]),
        ));
        obj.put(DataElement::new(
            Tag(0x0008, 0x0060),
            VR::CS,
            dicom_value!(Strs, ["CR"]),
        ));
        obj.put(DataElement::new(
            Tag(0x0008, 0x0018),
            VR::UI,
            dicom_value!(Strs, [sop_uid]),
        ));

        {
            let mut iter = obj.iter();
            assert_eq!(
                *iter.next().unwrap().header(),
                DataElementHeader::new(Tag(0x0008, 0x0018), VR::UI, Length(sop_uid.len() as u32)),
            );
            assert_eq!(
                *iter.next().unwrap().header(),
                DataElementHeader::new(Tag(0x0008, 0x0060), VR::CS, Length(2)),
            );
            assert_eq!(
                *iter.next().unwrap().header(),
                DataElementHeader::new(Tag(0x0010, 0x0010), VR::PN, Length(8)),
            );
        }

        // .tags()
        let tags: Vec<_> = obj.tags().collect();
        assert_eq!(
            tags,
            vec![
                Tag(0x0008, 0x0018),
                Tag(0x0008, 0x0060),
                Tag(0x0010, 0x0010),
            ]
        );

        // .into_iter()
        let mut iter = obj.into_iter();
        assert_eq!(
            iter.next(),
            Some(DataElement::new(
                Tag(0x0008, 0x0018),
                VR::UI,
                dicom_value!(Strs, [sop_uid]),
            )),
        );
        assert_eq!(
            iter.next(),
            Some(DataElement::new(
                Tag(0x0008, 0x0060),
                VR::CS,
                dicom_value!(Strs, ["CR"]),
            )),
        );
        assert_eq!(
            iter.next(),
            Some(DataElement::new(
                Tag(0x0010, 0x0010),
                VR::PN,
                PrimitiveValue::from("Doe^John"),
            )),
        );
    }

    #[test]
    fn inmem_empty_object_into_tokens() {
        let obj = InMemDicomObject::new_empty();
        let tokens = obj.into_tokens();
        assert_eq!(tokens.count(), 0);
    }

    #[test]
    fn inmem_shallow_object_from_tokens() {
        let tokens = vec![
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0008, 0x0060),
                vr: VR::CS,
                len: Length(2),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("MG".to_owned())),
            DataToken::ElementHeader(DataElementHeader {
                tag: Tag(0x0010, 0x0010),
                vr: VR::PN,
                len: Length(8),
            }),
            DataToken::PrimitiveValue(PrimitiveValue::Str("Doe^John".to_owned())),
        ];

        let gt_obj = InMemDicomObject::from_element_iter(vec![
            DataElement::new(
                Tag(0x0010, 0x0010),
                VR::PN,
                PrimitiveValue::Str("Doe^John".to_string()),
            ),
            DataElement::new(
                Tag(0x0008, 0x0060),
                VR::CS,
                PrimitiveValue::Str("MG".to_string()),
            ),
        ]);

        let obj = InMemDicomObject::build_object(
            &mut tokens.into_iter().map(Result::Ok),
            StandardDataDictionary,
            false,
            Length::UNDEFINED,
            None,
        )
        .unwrap();

        assert_obj_eq(&obj, &gt_obj);
    }

    #[test]
    fn inmem_shallow_object_into_tokens() {
        let patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()),
        );
        let modality = DataElement::new(
            Tag(0x0008, 0x0060),
            VR::CS,
            PrimitiveValue::Str("MG".to_string()),
        );
        let mut obj = InMemDicomObject::new_empty();
        obj.put(patient_name);
        obj.put(modality);

        let tokens: Vec<_> = obj.into_tokens().collect();

        assert_eq!(
            tokens,
            vec![
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0060),
                    vr: VR::CS,
                    len: Length(2),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Str("MG".to_owned())),
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0010, 0x0010),
                    vr: VR::PN,
                    len: Length(8),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Str("Doe^John".to_owned())),
            ]
        );
    }

    #[test]
    fn inmem_deep_object_from_tokens() {
        use smallvec::smallvec;

        let obj_1 = InMemDicomObject::from_element_iter(vec![
            DataElement::new(Tag(0x0018, 0x6012), VR::US, Value::Primitive(1_u16.into())),
            DataElement::new(Tag(0x0018, 0x6014), VR::US, Value::Primitive(2_u16.into())),
        ]);

        let obj_2 = InMemDicomObject::from_element_iter(vec![DataElement::new(
            Tag(0x0018, 0x6012),
            VR::US,
            Value::Primitive(4_u16.into()),
        )]);

        let gt_obj = InMemDicomObject::from_element_iter(vec![
            DataElement::new(
                Tag(0x0018, 0x6011),
                VR::SQ,
                Value::from(DataSetSequence::new(
                    smallvec![obj_1, obj_2],
                    Length::UNDEFINED,
                )),
            ),
            DataElement::new(Tag(0x0020, 0x4000), VR::LT, Value::Primitive("TEST".into())),
        ]);

        let tokens: Vec<_> = vec![
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

        let obj = InMemDicomObject::build_object(
            &mut tokens.into_iter().map(Result::Ok),
            StandardDataDictionary,
            false,
            Length::UNDEFINED,
            None,
        )
        .unwrap();

        assert_obj_eq(&obj, &gt_obj);
    }

    #[test]
    fn inmem_deep_object_into_tokens() {
        use smallvec::smallvec;

        let obj_1 = InMemDicomObject::from_element_iter(vec![
            DataElement::new(Tag(0x0018, 0x6012), VR::US, Value::Primitive(1_u16.into())),
            DataElement::new(Tag(0x0018, 0x6014), VR::US, Value::Primitive(2_u16.into())),
        ]);

        let obj_2 = InMemDicomObject::from_element_iter(vec![DataElement::new(
            Tag(0x0018, 0x6012),
            VR::US,
            Value::Primitive(4_u16.into()),
        )]);

        let main_obj = InMemDicomObject::from_element_iter(vec![
            DataElement::new(
                Tag(0x0018, 0x6011),
                VR::SQ,
                Value::from(DataSetSequence::new(
                    smallvec![obj_1, obj_2],
                    Length::UNDEFINED,
                )),
            ),
            DataElement::new(Tag(0x0020, 0x4000), VR::LT, Value::Primitive("TEST".into())),
        ]);

        let tokens: Vec<_> = main_obj.into_tokens().collect();

        assert_eq!(
            tokens,
            vec![
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
            ]
        );
    }

    #[test]
    fn inmem_encapsulated_pixel_data_from_tokens() {
        use smallvec::smallvec;

        let gt_obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            Tag(0x7fe0, 0x0010),
            VR::OB,
            Value::from(PixelFragmentSequence::new_fragments(smallvec![vec![
                0x33;
                32
            ]])),
        )]);

        let tokens: Vec<_> = vec![
            DataToken::PixelSequenceStart,
            DataToken::ItemStart { len: Length(0) },
            DataToken::ItemEnd,
            DataToken::ItemStart { len: Length(32) },
            DataToken::ItemValue(vec![0x33; 32]),
            DataToken::ItemEnd,
            DataToken::SequenceEnd,
        ];

        let obj = InMemDicomObject::build_object(
            &mut tokens.into_iter().map(Result::Ok),
            StandardDataDictionary,
            false,
            Length::UNDEFINED,
            None,
        )
        .unwrap();

        assert_obj_eq(&obj, &gt_obj);
    }

    #[test]
    fn inmem_encapsulated_pixel_data_into_tokens() {
        use smallvec::smallvec;

        let main_obj = InMemDicomObject::from_element_iter(vec![DataElement::new(
            Tag(0x7fe0, 0x0010),
            VR::OB,
            Value::from(PixelFragmentSequence::new_fragments(smallvec![vec![
                0x33;
                32
            ]])),
        )]);

        let tokens: Vec<_> = main_obj.into_tokens().collect();

        assert_eq!(
            tokens,
            vec![
                DataToken::PixelSequenceStart,
                DataToken::ItemStart { len: Length(0) },
                DataToken::ItemEnd,
                DataToken::ItemStart { len: Length(32) },
                DataToken::ItemValue(vec![0x33; 32]),
                DataToken::ItemEnd,
                DataToken::SequenceEnd,
            ]
        );
    }

    /// Test attribute operations on in-memory DICOM objects.
    #[test]
    fn inmem_ops() {
        // create a base DICOM object
        let base_obj = InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::SERIES_INSTANCE_UID,
                VR::UI,
                PrimitiveValue::from("2.25.137041794342168732369025909031346220736.1"),
            ),
            DataElement::new(
                tags::SERIES_INSTANCE_UID,
                VR::UI,
                PrimitiveValue::from("2.25.137041794342168732369025909031346220736.1"),
            ),
            DataElement::new(
                tags::SOP_INSTANCE_UID,
                VR::UI,
                PrimitiveValue::from("2.25.137041794342168732369025909031346220736.1.1"),
            ),
            DataElement::new(
                tags::STUDY_DESCRIPTION,
                VR::LO,
                PrimitiveValue::from("Test study"),
            ),
            DataElement::new(
                tags::INSTITUTION_NAME,
                VR::LO,
                PrimitiveValue::from("Test Hospital"),
            ),
            DataElement::new(tags::ROWS, VR::US, PrimitiveValue::from(768_u16)),
            DataElement::new(tags::COLUMNS, VR::US, PrimitiveValue::from(1024_u16)),
            DataElement::new(
                tags::LOSSY_IMAGE_COMPRESSION,
                VR::CS,
                PrimitiveValue::from("01"),
            ),
            DataElement::new(
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                VR::DS,
                PrimitiveValue::from("5"),
            ),
            DataElement::new(
                tags::LOSSY_IMAGE_COMPRESSION_METHOD,
                VR::DS,
                PrimitiveValue::from("ISO_10918_1"),
            ),
        ]);

        {
            // remove
            let mut obj = base_obj.clone();
            let op = AttributeOp {
                selector: AttributeSelector::from(tags::STUDY_DESCRIPTION),
                action: AttributeAction::Remove,
            };

            obj.apply(op).unwrap();

            assert_eq!(obj.get(tags::STUDY_DESCRIPTION), None);
        }
        {
            let mut obj = base_obj.clone();

            // set if missing does nothing
            // on an existing string
            let op = AttributeOp {
                selector: tags::INSTITUTION_NAME.into(),
                action: AttributeAction::SetIfMissing("Nope Hospital".into()),
            };

            obj.apply(op).unwrap();

            assert_eq!(
                obj.get(tags::INSTITUTION_NAME),
                Some(&DataElement::new(
                    tags::INSTITUTION_NAME,
                    VR::LO,
                    PrimitiveValue::from("Test Hospital"),
                ))
            );

            // replace string
            let op = AttributeOp::new(
                tags::INSTITUTION_NAME,
                AttributeAction::ReplaceStr("REMOVED".into()),
            );

            obj.apply(op).unwrap();

            assert_eq!(
                obj.get(tags::INSTITUTION_NAME),
                Some(&DataElement::new(
                    tags::INSTITUTION_NAME,
                    VR::LO,
                    PrimitiveValue::from("REMOVED"),
                ))
            );

            // replacing a non-existing attribute
            // does nothing
            let op = AttributeOp::new(
                tags::REQUESTING_PHYSICIAN,
                AttributeAction::ReplaceStr("Doctor^Anonymous".into()),
            );

            obj.apply(op).unwrap();

            assert_eq!(obj.get(tags::REQUESTING_PHYSICIAN), None);

            // but DetIfMissing works
            let op = AttributeOp::new(
                tags::REQUESTING_PHYSICIAN,
                AttributeAction::SetStrIfMissing("Doctor^Anonymous".into()),
            );

            obj.apply(op).unwrap();

            assert_eq!(
                obj.get(tags::REQUESTING_PHYSICIAN),
                Some(&DataElement::new(
                    tags::REQUESTING_PHYSICIAN,
                    VR::PN,
                    PrimitiveValue::from("Doctor^Anonymous"),
                ))
            );
        }
        {
            // reset string
            let mut obj = base_obj.clone();
            let op = AttributeOp::new(
                tags::REQUESTING_PHYSICIAN,
                AttributeAction::SetStr("Doctor^Anonymous".into()),
            );

            obj.apply(op).unwrap();

            assert_eq!(
                obj.get(tags::REQUESTING_PHYSICIAN),
                Some(&DataElement::new(
                    tags::REQUESTING_PHYSICIAN,
                    VR::PN,
                    PrimitiveValue::from("Doctor^Anonymous"),
                ))
            );
        }

        {
            // extend with number
            let mut obj = base_obj.clone();
            let op = AttributeOp::new(
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                AttributeAction::PushF64(1.25),
            );

            obj.apply(op).unwrap();

            assert_eq!(
                obj.get(tags::LOSSY_IMAGE_COMPRESSION_RATIO),
                Some(&DataElement::new(
                    tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                    VR::DS,
                    dicom_value!(Strs, ["5", "1.25"]),
                ))
            );
        }
    }

    /// Test attribute operations on nested data sets.
    #[test]
    fn nested_inmem_ops() {
        let obj_1 = InMemDicomObject::from_element_iter([
            DataElement::new(Tag(0x0018, 0x6012), VR::US, PrimitiveValue::from(1_u16)),
            DataElement::new(Tag(0x0018, 0x6014), VR::US, PrimitiveValue::from(2_u16)),
        ]);

        let obj_2 = InMemDicomObject::from_element_iter([DataElement::new(
            Tag(0x0018, 0x6012),
            VR::US,
            PrimitiveValue::from(4_u16),
        )]);

        let mut main_obj = InMemDicomObject::from_element_iter(vec![
            DataElement::new(
                tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                VR::SQ,
                DataSetSequence::from(vec![obj_1, obj_2]),
            ),
            DataElement::new(Tag(0x0020, 0x4000), VR::LT, Value::Primitive("TEST".into())),
        ]);

        let selector: AttributeSelector =
            (tags::SEQUENCE_OF_ULTRASOUND_REGIONS, 0, Tag(0x0018, 0x6014)).into();

        main_obj
            .apply(AttributeOp::new(selector, AttributeAction::Set(3.into())))
            .unwrap();

        assert_eq!(
            main_obj
                .get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                .unwrap()
                .items()
                .unwrap()[0]
                .get(Tag(0x0018, 0x6014))
                .unwrap()
                .value(),
            &PrimitiveValue::from(3).into(),
        );

        let selector: AttributeSelector =
            (tags::SEQUENCE_OF_ULTRASOUND_REGIONS, 1, Tag(0x0018, 0x6012)).into();

        main_obj
            .apply(AttributeOp::new(selector, AttributeAction::Remove))
            .unwrap();

        // item should be empty
        assert_eq!(
            main_obj
                .get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                .unwrap()
                .items()
                .unwrap()[1]
                .tags()
                .collect::<Vec<_>>(),
            Vec::<Tag>::new(),
        );

        // trying to access the removed element returns an error
        assert!(matches!(
            main_obj.value_at((tags::SEQUENCE_OF_ULTRASOUND_REGIONS, 1, Tag(0x0018, 0x6012),)),
            Err(AtAccessError::MissingLeafElement { .. })
        ))
    }

    /// Test that constructive operations create items if necessary.
    #[test]
    fn constructive_op() {
        let mut obj = InMemDicomObject::from_element_iter([DataElement::new(
            tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
            VR::SQ,
            DataSetSequence::empty(),
        )]);

        let op = AttributeOp::new(
            (
                tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                0,
                tags::REGION_SPATIAL_FORMAT,
            ),
            AttributeAction::Set(5_u16.into()),
        );

        obj.apply(op).unwrap();

        // should have an item
        assert_eq!(
            obj.get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                .unwrap()
                .items()
                .unwrap()
                .len(),
            1,
        );

        // item should have 1 element
        assert_eq!(
            &obj.get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                .unwrap()
                .items()
                .unwrap()[0],
            &InMemDicomObject::from_element_iter([DataElement::new(
                tags::REGION_SPATIAL_FORMAT,
                VR::US,
                PrimitiveValue::from(5_u16)
            )]),
        );

        // new value can be accessed using value_at
        assert_eq!(
            obj.value_at((
                tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                0,
                tags::REGION_SPATIAL_FORMAT
            ))
            .unwrap(),
            &Value::from(PrimitiveValue::from(5_u16)),
        )
    }

    /// Test that operations on in-memory DICOM objects
    /// can create sequences from scratch.
    #[test]
    fn inmem_ops_can_create_seq() {
        let mut obj = InMemDicomObject::new_empty();

        obj.apply(AttributeOp::new(
            tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
            AttributeAction::SetIfMissing(PrimitiveValue::Empty),
        ))
        .unwrap();

        {
            // should create an empty sequence
            let sequence_ultrasound = obj
                .get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                .expect("should have sequence element");

            assert_eq!(sequence_ultrasound.vr(), VR::SQ);

            assert_eq!(sequence_ultrasound.items().as_deref(), Some(&[][..]),);
        }

        obj.apply(AttributeOp::new(
            (
                tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                tags::REGION_SPATIAL_FORMAT,
            ),
            AttributeAction::Set(1_u16.into()),
        ))
        .unwrap();

        {
            // sequence should now have an item
            assert_eq!(
                obj.get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                    .unwrap()
                    .items()
                    .map(|items| items.len()),
                Some(1),
            );
        }
    }

    /// Test that operations on in-memory DICOM objects
    /// can create deeply nested attributes from scratch.
    #[test]
    fn inmem_ops_can_create_nested_attribute() {
        let mut obj = InMemDicomObject::new_empty();

        obj.apply(AttributeOp::new(
            (
                tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                tags::REGION_SPATIAL_FORMAT,
            ),
            AttributeAction::Set(1_u16.into()),
        ))
        .unwrap();

        {
            // should create a sequence with a single item
            assert_eq!(
                obj.get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                    .unwrap()
                    .items()
                    .map(|items| items.len()),
                Some(1),
            );

            // item should have Region Spatial Format
            assert_eq!(
                obj.value_at((
                    tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                    tags::REGION_SPATIAL_FORMAT
                ))
                .unwrap(),
                &PrimitiveValue::from(1_u16).into(),
            )
        }
    }

    /// Test that operations on in-memory DICOM objects
    /// can truncate sequences.
    #[test]
    fn inmem_ops_can_truncate_seq() {
        let mut obj = InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::new_empty()]),
            ),
            DataElement::new_with_len(
                tags::PIXEL_DATA,
                VR::OB,
                Length::UNDEFINED,
                PixelFragmentSequence::new(vec![], vec![vec![0xcc; 8192], vec![0x55; 1024]]),
            ),
        ]);

        // removes the single item in the sequences
        obj.apply(AttributeOp::new(
            tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
            AttributeAction::Truncate(0),
        ))
        .unwrap();

        {
            let sequence_ultrasound = obj
                .get(tags::SEQUENCE_OF_ULTRASOUND_REGIONS)
                .expect("should have sequence element");
            assert_eq!(sequence_ultrasound.items().as_deref(), Some(&[][..]),);
        }

        // remove one of the fragments
        obj.apply(AttributeOp::new(
            tags::PIXEL_DATA,
            AttributeAction::Truncate(1),
        ))
        .unwrap();

        {
            // pixel data should now have a single fragment
            assert_eq!(
                obj.get(tags::PIXEL_DATA)
                    .unwrap()
                    .fragments()
                    .map(|fragments| fragments.len()),
                Some(1),
            );
        }
    }

    #[test]
    fn inmem_obj_reset_defined_length() {
        let mut entries: BTreeMap<Tag, InMemElement<StandardDataDictionary>> = BTreeMap::new();

        let patient_name =
            DataElement::new(tags::PATIENT_NAME, VR::CS, PrimitiveValue::from("Doe^John"));

        let study_description = DataElement::new(
            tags::STUDY_DESCRIPTION,
            VR::LO,
            PrimitiveValue::from("Test study"),
        );

        entries.insert(tags::PATIENT_NAME, patient_name.clone());

        // create object and force an arbitrary defined Length value
        let obj = InMemDicomObject::<StandardDataDictionary> {
            entries,
            dict: StandardDataDictionary,
            len: Length(1),
            charset_changed: false,
        };

        assert!(obj.length().is_defined());

        let mut o = obj.clone();
        o.put_element(study_description);
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.remove_element(tags::PATIENT_NAME);
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.remove_element_by_name("PatientName").unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.take_element(tags::PATIENT_NAME).unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.take_element_by_name("PatientName").unwrap();
        assert!(o.length().is_undefined());

        // resets Length even when retain does not make any changes
        let mut o = obj.clone();
        o.retain(|e| e.tag() == tags::PATIENT_NAME);
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::Remove,
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(tags::PATIENT_NAME, AttributeAction::Empty))
            .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::SetVr(VR::IS),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::Set(dicom_value!(Str, "Unknown")),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::SetStr("Patient^Anonymous".into()),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_AGE,
            AttributeAction::SetIfMissing(dicom_value!(75)),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_ADDRESS,
            AttributeAction::SetStrIfMissing("Chicago".into()),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::Replace(dicom_value!(Str, "Unknown")),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::ReplaceStr("Unknown".into()),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::PushStr("^Prof".into()),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::PushI32(-16),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::PushU32(16),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::PushI16(-16),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::PushU16(16),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::PushF32(16.16),
        ))
        .unwrap();
        assert!(o.length().is_undefined());

        let mut o = obj.clone();
        o.apply(AttributeOp::new(
            tags::PATIENT_NAME,
            AttributeAction::PushF64(16.1616),
        ))
        .unwrap();
        assert!(o.length().is_undefined());
    }

    #[test]
    fn create_commands() {
        // empty
        let obj = InMemDicomObject::command_from_element_iter([]);
        assert_eq!(
            obj.get(tags::COMMAND_GROUP_LENGTH)
                .map(|e| e.value().to_int::<u32>().unwrap()),
            Some(0)
        );

        // C-FIND-RQ
        let obj = InMemDicomObject::command_from_element_iter([
            // affected SOP class UID: 8 + 28 = 36
            DataElement::new(
                tags::AFFECTED_SOP_CLASS_UID,
                VR::UI,
                PrimitiveValue::from("1.2.840.10008.5.1.4.1.2.1.1"),
            ),
            // command field: 36 + 8 + 2 = 46
            DataElement::new(
                tags::COMMAND_FIELD,
                VR::US,
                // 0020H: C-FIND-RQ message
                dicom_value!(U16, [0x0020]),
            ),
            // message ID: 46 + 8 + 2 = 56
            DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [0])),
            //priority: 56 + 8 + 2 = 66
            DataElement::new(
                tags::PRIORITY,
                VR::US,
                // medium
                dicom_value!(U16, [0x0000]),
            ),
            // data set type: 66 + 8 + 2 = 76
            DataElement::new(
                tags::COMMAND_DATA_SET_TYPE,
                VR::US,
                dicom_value!(U16, [0x0001]),
            ),
        ]);
        assert_eq!(
            obj.get(tags::COMMAND_GROUP_LENGTH)
                .map(|e| e.value().to_int::<u32>().unwrap()),
            Some(76)
        );

        let storage_sop_class_uid = "1.2.840.10008.5.1.4.1.1.4";
        let storage_sop_instance_uid = "2.25.221314879990624101283043547144116927116";

        // C-STORE-RQ
        let obj = InMemDicomObject::command_from_element_iter([
            // group length (should be ignored in calculations and overridden)
            DataElement::new(
                tags::COMMAND_GROUP_LENGTH,
                VR::UL,
                PrimitiveValue::from(9999_u32),
            ),
            // SOP Class UID: 8 + 26 = 34
            DataElement::new(
                tags::AFFECTED_SOP_CLASS_UID,
                VR::UI,
                dicom_value!(Str, storage_sop_class_uid),
            ),
            // command field: 34 + 8 + 2 = 44
            DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x0001])),
            // message ID: 44 + 8 + 2 = 54
            DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [1])),
            //priority: 54 + 8 + 2 = 64
            DataElement::new(tags::PRIORITY, VR::US, dicom_value!(U16, [0x0000])),
            // data set type: 64 + 8 + 2 = 74
            DataElement::new(
                tags::COMMAND_DATA_SET_TYPE,
                VR::US,
                dicom_value!(U16, [0x0000]),
            ),
            // affected SOP Instance UID: 74 + 8 + 44 = 126
            DataElement::new(
                tags::AFFECTED_SOP_INSTANCE_UID,
                VR::UI,
                dicom_value!(Str, storage_sop_instance_uid),
            ),
        ]);

        assert_eq!(
            obj.get(tags::COMMAND_GROUP_LENGTH)
                .map(|e| e.value().to_int::<u32>().unwrap()),
            Some(126)
        );
    }

    #[test]
    fn test_even_len() {
        assert_eq!(even_len(0), 0);
        assert_eq!(even_len(1), 2);
        assert_eq!(even_len(2), 2);
        assert_eq!(even_len(3), 4);
        assert_eq!(even_len(4), 4);
        assert_eq!(even_len(5), 6);
    }

    #[test]
    fn can_update_value() {
        let mut obj = InMemDicomObject::from_element_iter([DataElement::new(
            tags::ANATOMIC_REGION_SEQUENCE,
            VR::SQ,
            DataSetSequence::empty(),
        )]);
        assert_eq!(
            obj.get(tags::ANATOMIC_REGION_SEQUENCE).map(|e| e.length()),
            Some(Length(0)),
        );

        assert_eq!(
            obj.update_value(tags::BURNED_IN_ANNOTATION, |_value| {
                panic!("should not be called")
            }),
            false,
        );

        let o = obj.update_value(tags::ANATOMIC_REGION_SEQUENCE, |value| {
            // add an item
            let items = value.items_mut().unwrap();
            items.push(InMemDicomObject::from_element_iter([DataElement::new(
                tags::INSTANCE_NUMBER,
                VR::IS,
                PrimitiveValue::from(1),
            )]));
        });
        assert_eq!(o, true);

        assert!(obj
            .get(tags::ANATOMIC_REGION_SEQUENCE)
            .unwrap()
            .length()
            .is_undefined());
    }

    #[test]
    fn deep_sequence_change_encoding_writes_undefined_sequence_length() {
        use smallvec::smallvec;

        let obj_1 = InMemDicomObject::from_element_iter(vec![
            //The length of this string is 20 bytes in ISO_IR 100 but should be 22 bytes in ISO_IR 192 (UTF-8)
            DataElement::new(
                tags::STUDY_DESCRIPTION,
                VR::SL,
                Value::Primitive("MORFOLOGÍA Y FUNCIÓN".into()),
            ),
            //ISO_IR 100 and ISO_IR 192 length are the same
            DataElement::new(
                tags::SERIES_DESCRIPTION,
                VR::SL,
                Value::Primitive("0123456789".into()),
            ),
        ]);

        let some_tag = Tag(0x0018, 0x6011);

        let inner_sequence = InMemDicomObject::from_element_iter(vec![DataElement::new(
            some_tag,
            VR::SQ,
            Value::from(DataSetSequence::new(
                smallvec![obj_1],
                Length(30), //20 bytes from study, 10 from series
            )),
        )]);
        let outer_sequence = DataElement::new(
            some_tag,
            VR::SQ,
            Value::from(DataSetSequence::new(
                smallvec![inner_sequence.clone(), inner_sequence],
                Length(60), //20 bytes from study, 10 from series
            )),
        );

        let original_object = InMemDicomObject::from_element_iter(vec![
            DataElement::new(tags::SPECIFIC_CHARACTER_SET, VR::CS, "ISO_IR 100"),
            outer_sequence,
        ]);

        assert_eq!(
            original_object
                .get(some_tag)
                .expect("object should be present")
                .length(),
            Length(60)
        );

        let mut changed_charset = original_object.clone();
        changed_charset.convert_to_utf8();
        assert!(changed_charset.charset_changed);

        use dicom_parser::dataset::DataToken as token;
        let options = IntoTokensOptions::new(true);
        let converted_tokens: Vec<_> = changed_charset.into_tokens_with_options(options).collect();

        assert_eq!(
            vec![
                token::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0005),
                    vr: VR::CS,
                    len: Length(10),
                }),
                token::PrimitiveValue("ISO_IR 192".into()),
                token::SequenceStart {
                    tag: Tag(0x0018, 0x6011),
                    len: Length::UNDEFINED,
                },
                token::ItemStart {
                    len: Length::UNDEFINED
                },
                token::SequenceStart {
                    tag: Tag(0x0018, 0x6011),
                    len: Length::UNDEFINED,
                },
                token::ItemStart {
                    len: Length::UNDEFINED
                },
                token::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x1030),
                    vr: VR::SL,
                    len: Length(22),
                }),
                token::PrimitiveValue("MORFOLOGÍA Y FUNCIÓN".into()),
                token::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x103E),
                    vr: VR::SL,
                    len: Length(10),
                }),
                token::PrimitiveValue("0123456789".into()),
                token::ItemEnd,
                token::SequenceEnd,
                token::ItemEnd,
                token::ItemStart {
                    len: Length::UNDEFINED
                },
                token::SequenceStart {
                    tag: Tag(0x0018, 0x6011),
                    len: Length::UNDEFINED,
                },
                token::ItemStart {
                    len: Length::UNDEFINED
                },
                token::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x1030),
                    vr: VR::SL,
                    len: Length(22),
                }),
                token::PrimitiveValue("MORFOLOGÍA Y FUNCIÓN".into()),
                token::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x103E),
                    vr: VR::SL,
                    len: Length(10),
                }),
                token::PrimitiveValue("0123456789".into()),
                token::ItemEnd,
                token::SequenceEnd,
                token::ItemEnd,
                token::SequenceEnd,
            ],
            converted_tokens
        );
    }

    #[test]
    fn private_elements() {
        let mut ds = InMemDicomObject::from_element_iter(vec![
            DataElement::new(
                Tag(0x0009, 0x0010),
                VR::LO,
                PrimitiveValue::from("CREATOR 1"),
            ),
            DataElement::new(
                Tag(0x0009, 0x0011),
                VR::LO,
                PrimitiveValue::from("CREATOR 2"),
            ),
            DataElement::new(
                Tag(0x0011, 0x0010),
                VR::LO,
                PrimitiveValue::from("CREATOR 3"),
            ),
        ]);
        ds.put_private_element(
            0x0009,
            "CREATOR 1",
            0x01,
            VR::DS,
            PrimitiveValue::Str("1.0".to_string()),
        )
        .unwrap();
        ds.put_private_element(
            0x0009,
            "CREATOR 4",
            0x02,
            VR::DS,
            PrimitiveValue::Str("1.0".to_string()),
        )
        .unwrap();

        let res = ds.put_private_element(
            0x0012,
            "CREATOR 4",
            0x02,
            VR::DS,
            PrimitiveValue::Str("1.0".to_string()),
        );
        assert_eq!(
            &res.err().unwrap().to_string(),
            "Group number must be odd, found 0x0012"
        );

        assert_eq!(
            ds.private_element(0x0009, "CREATOR 1", 0x01)
                .unwrap()
                .value()
                .to_str()
                .unwrap(),
            "1.0"
        );
        assert_eq!(
            ds.private_element(0x0009, "CREATOR 4", 0x02)
                .unwrap()
                .value()
                .to_str()
                .unwrap(),
            "1.0"
        );
        assert_eq!(
            ds.private_element(0x0009, "CREATOR 4", 0x02)
                .unwrap()
                .header()
                .tag(),
            Tag(0x0009, 0x1202)
        );
    }

    #[test]
    fn private_element_group_full() {
        let mut ds = InMemDicomObject::from_element_iter(
            (0..=0x00FFu16)
                .into_iter()
                .map(|i| {
                    DataElement::new(Tag(0x0009, i), VR::LO, PrimitiveValue::from("CREATOR 1"))
                })
                .collect::<Vec<DataElement<_>>>(),
        );
        let res = ds.put_private_element(0x0009, "TEST", 0x01, VR::DS, PrimitiveValue::from("1.0"));
        assert_eq!(
            res.err().unwrap().to_string(),
            "No space available in group 0x0009"
        );
    }
}
