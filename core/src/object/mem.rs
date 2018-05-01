//! This module contains the implementation for an in-memory DICOM object.

use itertools::Itertools;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use super::{DicomObject, DicomSequence};
use data::dataset::DataSetParser;
use data::text::SpecificCharacterSet;
use data::value::{DicomValueType, ValueType};
use data::{DataElement, Header, Tag};
use dictionary::{DataDictionary, DictionaryEntry, StandardDataDictionary};
use error::{Error, Result};
use meta::DicomMetaTable;
use transfer_syntax::codec::get_registry;

pub type InMemSequence<D> = DicomSequence<InMemDicomObject<D>>;

impl<D> InMemSequence<D> {
    fn from_iter<I>(tag: Tag, len: u32, objects: I) -> Result<Self>
    where
        I: IntoIterator<Item = InMemDicomObject<D>>,
    {
        let mut objects = objects.into_iter().take_while(|e| false);
        unimplemented!()
    }
}

type InMemElement<D> = DataElement<InMemDicomObject<D>>;

/** A DICOM object that is fully contained in memory.
 */
#[derive(Debug, Clone)]
pub struct InMemDicomObject<D> {
    entries: BTreeMap<Tag, InMemElement<D>>,
    dict: D,
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

    fn size(&self) -> u32 {
        panic!("Attempt to fetch the size of a DICOM object")
    }
}

impl<'s, D: 's> DicomObject for &'s InMemDicomObject<D>
where
    D: DataDictionary,
{
    type Element = &'s InMemElement<D>;
    type Sequence = InMemSequence<D>;

    fn element(&self, tag: Tag) -> Result<Self::Element> {
        self.entries.get(&tag).ok_or(Error::NoSuchDataElement)
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        let tag = self.lookup_name(name)?;
        self.element(tag)
    }
}

impl InMemDicomObject<StandardDataDictionary> {
    /// Create a new empty DICOM object.
    pub fn create_empty() -> Self {
        InMemDicomObject {
            entries: BTreeMap::new(),
            dict: StandardDataDictionary,
        }
    }

    /// Create a DICOM object by reading from a file.
    pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_file_with_dict(path, StandardDataDictionary)
    }

    /// Create a DICOM object by reading from a byte sources.
    pub fn from_stream<S>(src: S) -> Result<Self>
    where
        S: Read,
    {
        Self::from_stream_with_dict(src, StandardDataDictionary)
    }

    /// Construct a DICOM object from an iterator of structured elements.
    pub fn from_iter<I>(iter: I) -> Result<Self>
    where
        I: IntoIterator<Item = Result<InMemElement<StandardDataDictionary>>>,
    {
        Self::from_iter_with_dict(iter, StandardDataDictionary)
    }
}

impl<D> InMemDicomObject<D>
where
    D: DataDictionary,
{
    /// Create a new empty object, using the given dictionary for name lookup.
    pub fn new_empty_with_dict(dict: D) -> Self {
        InMemDicomObject {
            entries: BTreeMap::new(),
            dict: dict,
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
        })
    }

    /// Create a DICOM object by reading from a file.
    pub fn open_file_with_dict<P: AsRef<Path>>(path: P, dict: D) -> Result<Self>
    where
        D: Clone,
    {
        let mut file = BufReader::new(File::open(path)?);

        // skip preamble
        {
            let mut buf = [0u8; 128];
            // skip the preamble
            file.read_exact(&mut buf)?;
        }

        // read metadata header
        let meta = DicomMetaTable::from_stream(&mut file)?;

        // read rest of data according to metadata, feed it to object
        let ts = get_registry()
            .get(&meta.transfer_syntax)
            .ok_or(Error::UnsupportedTransferSyntax)?;
        let cs = SpecificCharacterSet::Default;
        let dataset = DataSetParser::new_with_dictionary(file, dict.clone(), ts, cs)?;
        let entries = Self::build_entries(dataset);

        Ok(InMemDicomObject {
            entries: entries?,
            dict,
        })
    }

    /// Create a DICOM object by reading from a byte source.
    pub fn from_stream_with_dict<S>(src: S, dict: D) -> Result<Self>
    where
        S: Read,
        D: Clone,
    {
        let mut file = BufReader::new(src);

        // read metadata header
        let meta = DicomMetaTable::from_stream(&mut file)?;

        // read rest of data according to metadata, feed it to object
        let ts = get_registry()
            .get(&meta.transfer_syntax)
            .ok_or(Error::UnsupportedTransferSyntax)?;
        let cs = SpecificCharacterSet::Default;
        let dataset = DataSetParser::new_with_dictionary(file, dict.clone(), ts, cs)?;
        let entries = Self::build_entries(dataset);

        Ok(InMemDicomObject {
            entries: entries?,
            dict,
        })
    }

    fn build_entries<S, P>(dataset: DataSetParser<S, P, D>) -> Result<BTreeMap<Tag, InMemElement<D>>>
    {
        // TODO !!! perform a structured parsing of incoming tokens
        unimplemented!()
    }

    fn lookup_name(&self, name: &str) -> Result<Tag> {
        self.dict
            .by_name(name)
            .ok_or(Error::NoSuchAttributeName)
            .map(|e| e.tag())
    }

    /// Insert a data element to the object.
    pub fn put(&mut self, elt: InMemElement<D>) {
        self.entries.insert(elt.tag(), elt);
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
    use data::value::{PrimitiveValue, Value};
    use data::VR;
    use object::DicomObject;

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
