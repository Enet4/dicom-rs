//! This module contains the implementation for an in-memory DICOM object.

use itertools::Itertools;
use smallvec::SmallVec;
use snafu::{OptionExt, ResultExt};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::meta::FileMetaTable;
use crate::{
    CreateParser, DicomObject, MissingElementValue, NoSuchAttributeName, NoSuchDataElementAlias,
    NoSuchDataElementTag, OpenFile, ParseMetaDataSet, PrematureEnd, ReadFile, ReadToken, Result,
    RootDicomObject, UnexpectedToken, UnsupportedTransferSyntax,
};
use dicom_core::dictionary::{DataDictionary, DictionaryEntry};
use dicom_core::header::{HasLength, Header};
use dicom_core::value::{Value, C};
use dicom_core::{DataElement, Length, Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_encoding::text::SpecificCharacterSet;
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_parser::dataset::read::Error as ParserError;
use dicom_parser::dataset::{DataSetReader, DataToken};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

/// A full in-memory DICOM data element.
pub type InMemElement<D> = DataElement<InMemDicomObject<D>, InMemFragment>;

/// The type of a pixel data fragment.
pub type InMemFragment = Vec<u8>;

type ParserResult<T> = std::result::Result<T, ParserError>;

/** A DICOM object that is fully contained in memory.
 */
#[derive(Debug, Clone)]
pub struct InMemDicomObject<D> {
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
        self.entries.get(&tag).context(NoSuchDataElementTag { tag })
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        let tag = self.lookup_name(name)?;
        self.element(tag)
    }
}

impl RootDicomObject<InMemDicomObject<StandardDataDictionary>> {
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
    pub fn create_empty() -> Self {
        InMemDicomObject {
            entries: BTreeMap::new(),
            dict: StandardDataDictionary,
            len: Length::UNDEFINED,
        }
    }

    /// Construct a DICOM object from a fallible source of structured elements.
    pub fn from_element_source<I>(iter: I) -> Result<Self>
    where
        I: IntoIterator<Item = Result<InMemElement<StandardDataDictionary>>>,
    {
        Self::from_element_source_with_dict(iter, StandardDataDictionary)
    }

    /// Construct a DICOM object from a non-fallible source of structured elements.
    pub fn from_element_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = InMemElement<StandardDataDictionary>>,
    {
        Self::from_iter_with_dict(iter, StandardDataDictionary)
    }
}

impl<D> RootDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary,
    D: Clone,
{
    /// Create a new empty object, using the given dictionary and
    /// file meta table.
    pub fn new_empty_with_dict_and_meta(dict: D, meta: FileMetaTable) -> Self {
        RootDicomObject {
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
        let path = path.as_ref();
        let mut file =
            BufReader::new(File::open(path).with_context(|| OpenFile { filename: path })?);

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
            let mut dataset = DataSetReader::new_with_dictionary(file, dict.clone(), ts, cs, Default::default())
                .context(CreateParser)?;

            Ok(RootDicomObject {
                meta,
                obj: InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED)?,
            })
        } else {
            UnsupportedTransferSyntax {
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
        let mut file = BufReader::new(src);

        // read metadata header
        let meta = FileMetaTable::from_reader(&mut file).context(ParseMetaDataSet)?;

        // read rest of data according to metadata, feed it to object
        if let Some(ts) = ts_index.get(&meta.transfer_syntax) {
            let cs = SpecificCharacterSet::Default;
            let mut dataset = DataSetReader::new_with_dictionary(file, dict.clone(), ts, cs, Default::default())
                .context(CreateParser)?;
            let obj = InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED)?;
            Ok(RootDicomObject { meta, obj })
        } else {
            UnsupportedTransferSyntax {
                uid: meta.transfer_syntax,
            }
            .fail()
        }
    }
}

impl RootDicomObject<InMemDicomObject<StandardDataDictionary>> {
    /// Create a new empty object, using the given file meta table.
    pub fn new_empty_with_meta(meta: FileMetaTable) -> Self {
        RootDicomObject {
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
        let entries: Result<_> = iter.into_iter().map_results(|e| (e.tag(), e)).collect();
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
        self.entries.get(&tag).context(NoSuchDataElementTag { tag })
    }

    /// Retrieve a particular DICOM element by its name.
    pub fn element_by_name(&self, name: &str) -> Result<&InMemElement<D>> {
        let tag = self.lookup_name(name)?;
        self.entries
            .get(&tag)
            .with_context(|| NoSuchDataElementAlias {
                tag,
                alias: name.to_string(),
            })
    }

    /// Insert a data element to the object, replacing (and returning) any
    /// previous element of the same attribute.
    pub fn put(&mut self, elt: InMemElement<D>) -> Option<InMemElement<D>> {
        self.entries.insert(elt.tag(), elt)
    }

    // private methods

    /// Build an object by consuming a data set parser.
    fn build_object<I: ?Sized>(dataset: &mut I, dict: D, in_item: bool, len: Length) -> Result<Self>
    where
        I: Iterator<Item = ParserResult<DataToken>>,
    {
        let mut entries: BTreeMap<Tag, InMemElement<D>> = BTreeMap::new();
        // perform a structured parsing of incoming tokens
        while let Some(token) = dataset.next() {
            let elem = match token.context(ReadToken)? {
                DataToken::PixelSequenceStart => {
                    let value = InMemDicomObject::build_encapsulated_data(&mut *dataset)?;
                    DataElement::new(Tag(0x7fe0, 0x0010), VR::OB, value)
                }
                DataToken::ElementHeader(header) => {
                    // fetch respective value, place it in the entries
                    let next_token = dataset.next().context(MissingElementValue)?;
                    match next_token.context(ReadToken)? {
                        DataToken::PrimitiveValue(v) => {
                            InMemElement::new(header.tag, header.vr, Value::Primitive(v))
                        }
                        token => {
                            return UnexpectedToken { token }.fail();
                        }
                    }
                }
                DataToken::SequenceStart { tag, len } => {
                    // delegate sequence building to another function
                    let items = Self::build_sequence(tag, len, &mut *dataset, &dict)?;
                    DataElement::new(tag, VR::SQ, Value::Sequence { items, size: len })
                }
                DataToken::ItemEnd if in_item => {
                    // end of item, leave now
                    return Ok(InMemDicomObject { entries, dict, len });
                }
                token => return UnexpectedToken { token }.fail(),
            };
            entries.insert(elem.tag(), elem);
        }

        Ok(InMemDicomObject { entries, dict, len })
    }

    /// Build an encapsulated pixel data by collecting all fragments into an
    /// in-memory DICOM value.
    fn build_encapsulated_data<I>(
        mut dataset: I,
    ) -> Result<Value<InMemDicomObject<D>, InMemFragment>>
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

        while let Some(token) = dataset.next() {
            match token.context(ReadToken)? {
                DataToken::ItemValue(data) => {
                    if offset_table.is_none() {
                        offset_table = Some(data.into());
                    } else {
                        fragments.push(data);
                    }
                }
                DataToken::ItemEnd => {
                    // at the end of the first item ensure the presence of
                    // an empty offset_table here, so that the next items
                    // are seen as compressed fragments
                    if offset_table.is_none() {
                        offset_table = Some(C::new())
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
                    return UnexpectedToken { token }.fail();
                }
            }
        }

        Ok(Value::PixelSequence {
            fragments,
            offset_table: offset_table.unwrap_or_default(),
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
            match token.context(ReadToken)? {
                DataToken::ItemStart { len } => {
                    items.push(Self::build_object(&mut *dataset, dict.clone(), true, len)?);
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

    fn lookup_name(&self, name: &str) -> Result<Tag> {
        self.dict
            .by_name(name)
            .context(NoSuchAttributeName { name })
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

#[cfg(test)]
mod tests {

    use super::*;
    use dicom_core::header::{DataElementHeader, Length, VR};
    use dicom_core::value::PrimitiveValue;
    use dicom_parser::dataset::IntoTokens;

    fn assert_obj_eq<D>(obj1: &InMemDicomObject<D>, obj2: &InMemDicomObject<D>)
    where
        D: std::fmt::Debug,
    {
        // debug representation because it makes a stricter comparison and
        // assumes that Undefined lengths are equal.
        assert_eq!(format!("{:?}", obj1), format!("{:?}", obj2))
    }

    #[test]
    fn inmem_object_write() {
        let mut obj1 = InMemDicomObject::create_empty();
        let mut obj2 = InMemDicomObject::create_empty();
        assert_eq!(obj1, obj2);
        let empty_patient_name = DataElement::empty(Tag(0x0010, 0x0010), VR::PN);
        obj1.put(empty_patient_name.clone());
        assert_ne!(obj1, obj2);
        obj2.put(empty_patient_name.clone());
        assert_obj_eq(&obj1, &obj2);
    }

    #[test]
    fn inmem_object_get() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()).into(),
        );
        let mut obj = InMemDicomObject::create_empty();
        obj.put(another_patient_name.clone());
        let elem1 = (&obj).element(Tag(0x0010, 0x0010)).unwrap();
        assert_eq!(elem1, &another_patient_name);
    }

    #[test]
    fn inmem_object_get_by_name() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()).into(),
        );
        let mut obj = InMemDicomObject::create_empty();
        obj.put(another_patient_name.clone());
        let elem1 = (&obj).element_by_name("PatientName").unwrap();
        assert_eq!(elem1, &another_patient_name);
    }

    #[test]
    fn inmem_empty_object_into_tokens() {
        let obj = InMemDicomObject::create_empty();
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
                PrimitiveValue::Str("Doe^John".to_string()).into(),
            ),
            DataElement::new(
                Tag(0x0008, 0x0060),
                VR::CS,
                PrimitiveValue::Str("MG".to_string()).into(),
            ),
        ]);

        let obj = InMemDicomObject::build_object(
            &mut tokens.into_iter().map(Result::Ok),
            StandardDataDictionary,
            false,
            Length::UNDEFINED,
        )
        .unwrap();

        assert_obj_eq(&obj, &gt_obj);
    }

    #[test]
    fn inmem_shallow_object_into_tokens() {
        let patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::Str("Doe^John".to_string()).into(),
        );
        let modality = DataElement::new(
            Tag(0x0008, 0x0060),
            VR::CS,
            PrimitiveValue::Str("MG".to_string()).into(),
        );
        let mut obj = InMemDicomObject::create_empty();
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
