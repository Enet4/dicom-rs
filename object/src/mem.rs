//! This module contains the implementation for an in-memory DICOM object.

use itertools::Itertools;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::meta::FileMetaTable;
use crate::{DicomObject, RootDicomObject};
use dicom_core::dictionary::{DataDictionary, DictionaryEntry};
use dicom_core::header::Header;
use dicom_core::value::{DicomValueType, Value, ValueType, C};
use dicom_core::{DataElement, Length, Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_encoding::text::SpecificCharacterSet;
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_parser::dataset::{DataSetReader, DataToken};
use dicom_parser::error::{DataSetSyntaxError, Error, Result};
use dicom_parser::StatefulDecode;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

/// A full in-memory DICOM data element.
pub type InMemElement<D> = DataElement<InMemDicomObject<D>>;

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

impl<D> DicomValueType for InMemDicomObject<D> {
    fn value_type(&self) -> ValueType {
        ValueType::Item
    }

    fn size(&self) -> Length {
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
        self.entries.get(&tag).ok_or(Error::NoSuchDataElement)
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

    /// Construct a DICOM object from an iterator of structured elements.
    pub fn from_element_iter<I>(iter: I) -> Result<Self>
    where
        I: IntoIterator<Item = Result<InMemElement<StandardDataDictionary>>>,
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
        let mut file = BufReader::new(File::open(path)?);

        // skip preamble
        {
            let mut buf = [0u8; 128];
            // skip the preamble
            file.read_exact(&mut buf)?;
        }

        // read metadata header
        let meta = FileMetaTable::from_reader(&mut file)?;

        // read rest of data according to metadata, feed it to object
        let ts = ts_index
            .get(&meta.transfer_syntax)
            .ok_or(Error::UnsupportedTransferSyntax)?;
        let cs = SpecificCharacterSet::Default;
        let mut dataset = DataSetReader::new_with_dictionary(file, dict.clone(), ts, cs)?;

        Ok(RootDicomObject {
            meta,
            obj: InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED)?,
        })
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
        let meta = FileMetaTable::from_reader(&mut file)?;

        // read rest of data according to metadata, feed it to object
        let ts = ts_index
            .get(&meta.transfer_syntax)
            .ok_or(Error::UnsupportedTransferSyntax)?;
        let cs = SpecificCharacterSet::Default;
        let mut dataset = DataSetReader::new_with_dictionary(file, dict.clone(), ts, cs)?;
        let obj = InMemDicomObject::build_object(&mut dataset, dict, false, Length::UNDEFINED)?;
        Ok(RootDicomObject { meta, obj })
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
    pub fn from_iter_with_dict<I>(iter: I, dict: D) -> Result<Self>
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
        self.entries.get(&tag).ok_or(Error::NoSuchDataElement)
    }

    /// Retrieve a particular DICOM element by its name.
    pub fn element_by_name(&self, name: &str) -> Result<&InMemElement<D>> {
        let tag = self.lookup_name(name)?;
        self.element(tag)
    }

    /// Insert a data element to the object, replacing (and returning) any
    /// previous element of the same attribute.
    pub fn put(&mut self, elt: InMemElement<D>) -> Option<InMemElement<D>> {
        self.entries.insert(elt.tag(), elt)
    }

    // private methods

    /// Build an object by consuming a data set parser.
    fn build_object<P>(
        dataset: &mut DataSetReader<P, D>,
        dict: D,
        in_item: bool,
        len: Length,
    ) -> Result<Self>
    where
        P: StatefulDecode,
    {
        let mut entries: BTreeMap<Tag, InMemElement<D>> = BTreeMap::new();
        // perform a structured parsing of incoming tokens
        while let Some(token) = dataset.next() {
            let elem = match token? {
                DataToken::ElementHeader(header) => {
                    // fetch respective value, place it in the entries
                    let next_token = dataset.next().ok_or_else(|| Error::MissingElementValue)?;
                    match next_token? {
                        DataToken::PrimitiveValue(v) => {
                            InMemElement::new(header.tag, header.vr, Value::Primitive(v))
                        }
                        token => {
                            return Err(DataSetSyntaxError::UnexpectedToken(token).into());
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
                token => return Err(DataSetSyntaxError::UnexpectedToken(token).into()),
            };
            entries.insert(elem.tag(), elem);
        }

        Ok(InMemDicomObject { entries, dict, len })
    }

    /// Build a DICOM sequence by consuming a data set parser.
    fn build_sequence<P>(
        _tag: Tag,
        _len: Length,
        dataset: &mut DataSetReader<P, D>,
        dict: &D,
    ) -> Result<C<InMemDicomObject<D>>>
    where
        P: StatefulDecode,
    {
        let mut items: C<_> = SmallVec::new();
        while let Some(token) = dataset.next() {
            match token? {
                DataToken::ItemStart { len } => {
                    // TODO if length is well defined, then it should be
                    // considered instead of finding the item delimiter.
                    items.push(Self::build_object(&mut *dataset, dict.clone(), true, len)?);
                }
                DataToken::SequenceEnd => {
                    return Ok(items);
                }
                token => return Err(DataSetSyntaxError::UnexpectedToken(token).into()),
            };
        }

        // iterator fully consumed without a sequence delimiter
        Err(DataSetSyntaxError::PrematureEnd.into())
    }

    fn lookup_name(&self, name: &str) -> Result<Tag> {
        self.dict
            .by_name(name)
            .ok_or(Error::NoSuchAttributeName)
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
    use dicom_core::value::PrimitiveValue;
    use dicom_core::VR;

    #[test]
    fn inmem_object_write() {
        let mut obj1 = InMemDicomObject::create_empty();
        let mut obj2 = InMemDicomObject::create_empty();
        assert_eq!(obj1, obj2);
        let empty_patient_name = DataElement::empty(Tag(0x0010, 0x0010), VR::PN);
        obj1.put(empty_patient_name.clone());
        assert_ne!(obj1, obj2);
        obj2.put(empty_patient_name.clone());
        assert_eq!(obj1, obj2);
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
}
