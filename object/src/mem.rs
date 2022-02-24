//! This module contains the implementation for an in-memory DICOM object.

use itertools::Itertools;
use smallvec::SmallVec;
use snafu::{OptionExt, ResultExt};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::{collections::BTreeMap, io::Write};

use crate::file::ReadPreamble;
use crate::{meta::FileMetaTable, FileMetaTableBuilder};
use crate::{
    BuildMetaTableSnafu, CreateParserSnafu, CreatePrinterSnafu, DicomObject, FileDicomObject, MissingElementValueSnafu,
    NoSuchAttributeNameSnafu, NoSuchDataElementAliasSnafu, NoSuchDataElementTagSnafu, OpenFileSnafu, ParseMetaDataSetSnafu,
    PrematureEndSnafu, PrepareMetaTableSnafu, PrintDataSetSnafu, ReadFileSnafu, ReadPreambleBytesSnafu, ReadTokenSnafu, Result,
    UnexpectedTokenSnafu, UnsupportedTransferSyntaxSnafu,
};
use dicom_core::dictionary::{DataDictionary, DictionaryEntry};
use dicom_core::header::{HasLength, Header};
use dicom_core::value::{Value, C};
use dicom_core::{DataElement, Length, Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_encoding::{encode::EncodeTo, text::SpecificCharacterSet, TransferSyntax};
use dicom_parser::dataset::{DataSetReader, DataToken};
use dicom_parser::{
    dataset::{read::Error as ParserError, DataSetWriter, IntoTokens},
    StatefulDecode,
};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

/// A full in-memory DICOM data element.
pub type InMemElement<D = StandardDataDictionary> = DataElement<InMemDicomObject<D>, InMemFragment>;

/// The type of a pixel data fragment.
pub type InMemFragment = Vec<u8>;

type ParserResult<T> = std::result::Result<T, ParserError>;

/** A DICOM object that is fully contained in memory.
 */
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
}

impl<'s, D> PartialEq for InMemDicomObject<D> {
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
        self.entries.get(&tag).context(NoSuchDataElementTagSnafu { tag })
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        let tag = self.lookup_name(name)?;
        self.element(tag)
    }
}

impl FileDicomObject<InMemDicomObject<StandardDataDictionary>> {
    /// Create a DICOM object by reading from a file.
    ///
    /// This function assumes the standard file encoding structure: 128-byte
    /// preamble, file meta group, and the rest of the data set.
    pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_file_with_dict(path, StandardDataDictionary)
    }

    /// Create a DICOM object by reading from a byte source.
    ///
    /// This function assumes the standard file encoding structure without the
    /// preamble: file meta group, followed by the rest of the data set.
    pub fn from_reader<S>(src: S) -> Result<Self>
    where
        S: Read,
    {
        Self::from_reader_with_dict(src, StandardDataDictionary)
    }
}

impl InMemDicomObject<StandardDataDictionary> {

    /// Create a new empty DICOM object.
    #[deprecated(since = "0.5.0", note = "Use `new_empty` instead")]
    pub fn create_empty() -> Self {
        Self::new_empty()
    }

    /// Create a new empty DICOM object.
    pub fn new_empty() -> Self {
        InMemDicomObject {
            entries: BTreeMap::new(),
            dict: StandardDataDictionary,
            len: Length::UNDEFINED,
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

    /// Read an object from a source using the given decoder.
    ///
    /// Note: [`read_dataset_with_ts`] and [`read_dataset_with_ts_cs`]
    /// may be easier to use.
    ///
    /// [`read_dataset_with_ts`]: #method.read_dataset_with_ts
    /// [`read_dataset_with_ts_cs`]: #method.read_dataset_with_ts_cs
    #[inline]
    pub fn read_dataset<S>(decoder: S) -> Result<Self>
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
    ) -> Result<Self>
    where
        S: Read,
    {
        Self::read_dataset_with_dict_ts_cs(from, StandardDataDictionary, ts, cs)
    }

    /// Read an object from a source,
    /// using the given transfer syntax.
    ///
    /// The default character set is assumed
    /// until _Specific Character Set_ is found in the encoded data,
    /// after which the text decoder will be overriden accordingly.
    #[inline]
    pub fn read_dataset_with_ts<S>(from: S, ts: &TransferSyntax) -> Result<Self>
    where
        S: Read,
    {
        Self::read_dataset_with_dict_ts_cs(
            from,
            StandardDataDictionary,
            ts,
            SpecificCharacterSet::Default,
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
            },
        }
    }

    /// Create a DICOM object by reading from a file.
    ///
    /// This function assumes the standard file encoding structure: 128-byte
    /// preamble, file meta group, and the rest of the data set.
    pub fn open_file_with_dict<P: AsRef<Path>>(path: P, dict: D) -> Result<Self> {
        Self::open_file_with(path, dict, TransferSyntaxRegistry)
    }

    /// Create a DICOM object by reading from a file.
    ///
    /// This function assumes the standard file encoding structure: 128-byte
    /// preamble, file meta group, and the rest of the data set.
    ///
    /// This function allows you to choose a different transfer syntax index,
    /// but its use is only advised when the built-in transfer syntax registry
    /// is insufficient. Otherwise, please use [`open_file_with_dict`] instead.
    ///
    /// [`open_file_with_dict`]: #method.open_file_with_dict
    pub fn open_file_with<P: AsRef<Path>, R>(path: P, dict: D, ts_index: R) -> Result<Self>
    where
        P: AsRef<Path>,
        R: TransferSyntaxIndex,
    {
        Self::open_file_with_all_options(path, dict, ts_index, None, ReadPreamble::Auto)
    }

    pub(crate) fn open_file_with_all_options<P: AsRef<Path>, R>(
        path: P,
        dict: D,
        ts_index: R,
        read_until: Option<Tag>,
        read_preamble: ReadPreamble,
    ) -> Result<Self>
    where
        P: AsRef<Path>,
        R: TransferSyntaxIndex,
    {
        let path = path.as_ref();
        let mut file =
            BufReader::new(File::open(path).with_context(|_| OpenFileSnafu { filename: path })?);

        if read_preamble == ReadPreamble::Auto || read_preamble == ReadPreamble::Always {
            let mut buf = [0u8; 128];
            // skip the preamble
            file.read_exact(&mut buf)
                .with_context(|_| ReadFileSnafu { filename: path })?;
        }

        // read metadata header
        let meta = FileMetaTable::from_reader(&mut file).context(ParseMetaDataSetSnafu)?;

        // read rest of data according to metadata, feed it to object
        if let Some(ts) = ts_index.get(&meta.transfer_syntax) {
            let cs = SpecificCharacterSet::Default;
            let mut dataset =
                DataSetReader::new_with_dictionary(file, dict.clone(), ts, cs, Default::default())
                    .context(CreateParserSnafu)?;

            Ok(FileDicomObject {
                meta,
                obj: InMemDicomObject::build_object(
                    &mut dataset,
                    dict,
                    false,
                    Length::UNDEFINED,
                    read_until,
                )?,
            })
        } else {
            UnsupportedTransferSyntaxSnafu {
                uid: meta.transfer_syntax,
            }
            .fail()
        }
    }

    /// Create a DICOM object by reading from a byte source.
    ///
    /// This function assumes the standard file encoding structure without the
    /// preamble: file meta group, followed by the rest of the data set.
    pub fn from_reader_with_dict<S>(src: S, dict: D) -> Result<Self>
    where
        S: Read,
    {
        Self::from_reader_with(src, dict, TransferSyntaxRegistry)
    }

    /// Create a DICOM object by reading from a byte source.
    ///
    /// This function assumes the standard file encoding structure without the
    /// preamble: file meta group, followed by the rest of the data set.
    ///
    /// This function allows you to choose a different transfer syntax index,
    /// but its use is only advised when the built-in transfer syntax registry
    /// is insufficient. Otherwise, please use [`from_reader_with_dict`] instead.
    ///
    /// [`from_reader_with_dict`]: #method.from_reader_with_dict
    pub fn from_reader_with<'s, S: 's, R>(src: S, dict: D, ts_index: R) -> Result<Self>
    where
        S: Read,
        R: TransferSyntaxIndex,
    {
        Self::from_reader_with_all_options(src, dict, ts_index, None, ReadPreamble::Auto)
    }

    pub(crate) fn from_reader_with_all_options<'s, S: 's, R>(
        src: S,
        dict: D,
        ts_index: R,
        read_until: Option<Tag>,
        read_preamble: ReadPreamble,
    ) -> Result<Self>
    where
        S: Read,
        R: TransferSyntaxIndex,
    {
        let mut file = BufReader::new(src);

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
            let cs = SpecificCharacterSet::Default;
            let mut dataset =
                DataSetReader::new_with_dictionary(file, dict.clone(), ts, cs, Default::default())
                    .context(CreateParserSnafu)?;
            let obj = InMemDicomObject::build_object(
                &mut dataset,
                dict,
                false,
                Length::UNDEFINED,
                read_until,
            )?;
            Ok(FileDicomObject { meta, obj })
        } else {
            UnsupportedTransferSyntaxSnafu {
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
        }
    }

    /// Read an object from a source,
    /// using the given decoder
    /// and the given dictionary for name lookup.
    pub fn read_dataset_with_dict<S>(decoder: S, dict: D) -> Result<Self>
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
    pub fn read_dataset_with_dict_ts<S>(from: S, dict: D, ts: &TransferSyntax) -> Result<Self>
    where
        S: Read,
        D: DataDictionary,
    {
        Self::read_dataset_with_dict_ts_cs(from, dict, ts, SpecificCharacterSet::Default)
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
    ) -> Result<Self>
    where
        S: Read,
        D: DataDictionary,
    {
        let from = BufReader::new(from);
        let mut dataset =
            DataSetReader::new_with_dictionary(from, dict.clone(), ts, cs, Default::default())
                .context(CreateParserSnafu)?;
        InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED, None)
    }

    // Standard methods follow. They are not placed as a trait implementation
    // because they may require outputs to reference the lifetime of self,
    // which is not possible without GATs.

    /// Retrieve the object's meta table if available.
    ///
    /// At the moment, this is sure to return `None`, because the meta
    /// table is kept in a separate wrapper value.
    pub fn meta(&self) -> Option<&FileMetaTable> {
        None
    }

    /// Retrieve a particular DICOM element by its tag.
    pub fn element(&self, tag: Tag) -> Result<&InMemElement<D>> {
        self.entries.get(&tag).context(NoSuchDataElementTagSnafu { tag })
    }

    /// Retrieve a particular DICOM element by its name.
    pub fn element_by_name(&self, name: &str) -> Result<&InMemElement<D>> {
        let tag = self.lookup_name(name)?;
        self.entries
            .get(&tag)
            .with_context(|| NoSuchDataElementAliasSnafu {
                tag,
                alias: name.to_string(),
            })
    }

    /// Insert a data element to the object, replacing (and returning) any
    /// previous element of the same attribute.
    pub fn put(&mut self, elt: InMemElement<D>) -> Option<InMemElement<D>> {
        self.put_element(elt)
    }

    /// Insert a data element to the object, replacing (and returning) any
    /// previous element of the same attribute.
    pub fn put_element(&mut self, elt: InMemElement<D>) -> Option<InMemElement<D>> {
        self.entries.insert(elt.tag(), elt)
    }

    /// Removes a DICOM element by its tag,
    /// reporting whether it was present.
    pub fn remove_element(&mut self, tag: Tag) -> bool {
        self.entries.remove(&tag).is_some()
    }

    /// Removes a DICOM element by its keyword,
    /// reporting whether it was present.
    pub fn remove_element_by_name(&mut self, name: &str) -> Result<bool> {
        let tag = self.lookup_name(name)?;
        Ok(self.entries.remove(&tag).is_some())
    }

    /// Removes and returns a particular DICOM element by its tag.
    pub fn take_element(&mut self, tag: Tag) -> Result<InMemElement<D>> {
        self.entries
            .remove(&tag)
            .context(NoSuchDataElementTagSnafu { tag })
    }

    /// Removes and returns a particular DICOM element by its name.
    pub fn take_element_by_name(&mut self, name: &str) -> Result<InMemElement<D>> {
        let tag = self.lookup_name(name)?;
        self.entries
            .remove(&tag)
            .with_context(|| NoSuchDataElementAliasSnafu {
                tag,
                alias: name.to_string(),
            })
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
    /// may be easier to use.
    ///
    /// [`write_dataset_with_ts`]: #method.write_dataset_with_ts
    /// [`write_dataset_with_ts_cs`]: #method.write_dataset_with_ts_cs
    pub fn write_dataset<W, E>(&self, to: W, encoder: E) -> Result<()>
    where
        W: Write,
        E: EncodeTo<W>,
    {
        // prepare data set writer
        let mut dset_writer = DataSetWriter::new(to, encoder);

        // write object
        dset_writer
            .write_sequence(self.into_tokens())
            .context(PrintDataSetSnafu)?;

        Ok(())
    }

    /// Write this object's data set into the given printer,
    /// with the specified transfer syntax and character set,
    /// without preamble, magic code, nor file meta group.
    ///
    /// If the attribute _Specific Character Set_ is found in the data set,
    /// the last parameter is overridden accordingly.
    pub fn write_dataset_with_ts_cs<W>(
        &self,
        to: W,
        ts: &TransferSyntax,
        cs: SpecificCharacterSet,
    ) -> Result<()>
    where
        W: Write,
    {
        // prepare data set writer
        let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinterSnafu)?;

        // write object
        dset_writer
            .write_sequence(self.into_tokens())
            .context(PrintDataSetSnafu)?;

        Ok(())
    }

    /// Write this object's data set into the given writer,
    /// with the specified transfer syntax,
    /// without preamble, magic code, nor file meta group.
    ///
    /// The default character set is assumed
    /// until the _Specific Character Set_ is found in the data set,
    /// after which the text encoder is overridden accordingly.
    pub fn write_dataset_with_ts<W>(&self, to: W, ts: &TransferSyntax) -> Result<()>
    where
        W: Write,
    {
        self.write_dataset_with_ts_cs(to, ts, SpecificCharacterSet::Default)
    }

    /// Encapsulate this object to contain a file meta group
    /// as described exactly by the given table.
    ///
    /// **Note:** this method will not adjust the file meta group
    /// to be semantically valid for the object.
    pub fn with_exact_meta(self, meta: FileMetaTable) -> FileDicomObject<Self> {
        FileDicomObject { meta, obj: self }
    }

    /// Encapsulate this object to contain a file meta group,
    /// created through the given file meta table builder.
    ///
    /// The attribute _Media Storage SOP Instance UID_
    /// will be filled in with the contents of the object,
    /// if the attribute _SOP Instance UID_  is present.
    /// A complete file meta group should still provide
    /// the media storage SOP class UID and transfer syntax.
    pub fn with_meta(self, mut meta: FileMetaTableBuilder) -> Result<FileDicomObject<Self>> {
        match self.element(Tag(0x0008, 0x0008)) {
            Ok(elem) => {
                meta = meta.media_storage_sop_instance_uid(
                    elem.value().to_str().context(PrepareMetaTableSnafu)?,
                );
            }
            Err(crate::Error::NoSuchDataElementTag { .. }) => {}
            Err(err) => return Err(err),
        }
        Ok(FileDicomObject {
            meta: meta.build().context(BuildMetaTableSnafu)?,
            obj: self,
        })
    }

    // private methods

    /// Build an object by consuming a data set parser.
    fn build_object<I: ?Sized>(
        dataset: &mut I,
        dict: D,
        in_item: bool,
        len: Length,
        read_until: Option<Tag>,
    ) -> Result<Self>
    where
        I: Iterator<Item = ParserResult<DataToken>>,
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
                        Value::Sequence { items, size: len },
                    )
                }
                DataToken::ItemEnd if in_item => {
                    // end of item, leave now
                    return Ok(InMemDicomObject { entries, dict, len });
                }
                token => return UnexpectedTokenSnafu { token }.fail(),
            };
            entries.insert(elem.tag(), elem);
        }

        Ok(InMemDicomObject { entries, dict, len })
    }

    /// Build an encapsulated pixel data by collecting all fragments into an
    /// in-memory DICOM value.
    fn build_encapsulated_data<I>(dataset: I) -> Result<Value<InMemDicomObject<D>, InMemFragment>>
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

        Ok(Value::PixelSequence {
            fragments,
            offset_table: offset_table.unwrap_or_default().into(),
        })
    }

    /// Build a DICOM sequence by consuming a data set parser.
    fn build_sequence<I: ?Sized>(
        _tag: Tag,
        _len: Length,
        dataset: &mut I,
        dict: &D,
    ) -> Result<C<InMemDicomObject<D>>>
    where
        I: Iterator<Item = ParserResult<DataToken>>,
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

    fn lookup_name(&self, name: &str) -> Result<Tag> {
        self.dict
            .by_name(name)
            .context(NoSuchAttributeNameSnafu { name })
            .map(|e| e.tag())
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
        self.entries.extend(iter.into_iter().map(|e| (e.tag(), e)))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{meta::FileMetaTableBuilder, open_file, Error};
    use byteordered::Endianness;
    use dicom_core::value::PrimitiveValue;
    use dicom_core::{
        dicom_value,
        header::{DataElementHeader, Length, VR},
    };
    use dicom_encoding::{
        decode::{basic::BasicDecoder, implicit_le::ImplicitVRLittleEndianDecoder},
        encode::{implicit_le::ImplicitVRLittleEndianEncoder, EncoderFor},
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
        let text = SpecificCharacterSet::Default;
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
        let cs = SpecificCharacterSet::Default;
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
        assert_eq!(
            physician_name.value().to_str().unwrap(),
            "Simões^João"
        );
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
        let cs = SpecificCharacterSet::Default;

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

    /// Write a file from scratch.
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
            Err(Error::NoSuchDataElementTag {
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
            Err(Error::NoSuchDataElementAlias {
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
                Value::Sequence {
                    items: smallvec![obj_1, obj_2],
                    size: Length::UNDEFINED,
                },
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
                Value::Sequence {
                    items: smallvec![obj_1, obj_2],
                    size: Length::UNDEFINED,
                },
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
            Value::PixelSequence {
                fragments: smallvec![vec![0x33; 32]],
                offset_table: Default::default(),
            },
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
            Value::PixelSequence {
                fragments: smallvec![vec![0x33; 32]],
                offset_table: Default::default(),
            },
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
}
