//! This module contains the implementation for an in-memory DICOM object.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;
use std::fs::File;
use std::io::BufReader;

use meta::DicomMetaTable;
use iterator::DicomElementIterator;
use super::DicomObject;
use data::{DataElement, Header, Tag};
use data::value::DicomValue;
use dictionary::{get_standard_dictionary, DataDictionary, DictionaryEntry, StandardDataDictionary};
use error::{Error, Result};
use object::pixeldata::PixelData;

/** A DICOM sequence that is fully contained in memory.
 */
#[derive(Debug, Clone)]
pub struct InMemSequence<D> {
    tag: Tag,
    objects: Vec<InMemDicomObject<D>>,
}

/** A DICOM object that is fully contained in memory.
 */
#[derive(Debug, Clone)]
pub struct InMemDicomObject<D> {
    entries: HashMap<Tag, DataElement>,
    dict: D,
}

impl<'s, D> PartialEq for InMemDicomObject<D> {
    // This implementation ignores the data dictionary.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<'s, D: 's> DicomObject for &'s InMemDicomObject<D>
where
    D: DataDictionary,
{
    type Element = &'s DataElement;
    type Sequence = InMemSequence<D>;

    fn get_element(&self, tag: Tag) -> Result<Self::Element> {
        self.entries.get(&tag).ok_or(Error::NoSuchDataElement)
    }

    fn get_element_by_name(&self, name: &str) -> Result<Self::Element> {
        let tag = self.lookup_name(name)?;
        self.get_element(tag)
    }

    fn get_pixel_data<PV, PD: PixelData<PV>>(&self) -> Result<PD> {
        unimplemented!()
    }
}

impl InMemDicomObject<&'static StandardDataDictionary> {
    /// Create a new empty DICOM object.
    pub fn create_empty() -> Self {
        InMemDicomObject {
            entries: HashMap::new(),
            dict: get_standard_dictionary(),
        }
    }

    /// Create a DICOM object by reading from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::from_file_with_dict(path, get_standard_dictionary())
    }
}

impl<D> InMemDicomObject<D>
where
    D: DataDictionary,
{
    /// Create a new empty object, using the given dictionary
    /// for name lookup.
    pub fn new_empty_with_dict(dict: D) -> Self {
        InMemDicomObject {
            entries: HashMap::new(),
            dict: dict,
        }
    }

    /// Create a DICOM object by reading from a file.
    pub fn from_file_with_dict<P: AsRef<Path>>(path: P, dict: D) -> Result<Self> {
        let mut file = BufReader::new(File::open(path)?);

        // read metadata header
        let meta = DicomMetaTable::from_readseek_stream(&mut file)?;
        let mut entries = HashMap::new();
        // TODO feed data to object

        // TODO read rest of data according to metadata, feed it to object
        let ts = unimplemented!();
        let cs = unimplemented!();
        let mut it = DicomElementIterator::new_with(&mut file, ts, cs);

        Ok(InMemDicomObject { entries, dict })
    }

    fn lookup_name(&self, name: &str) -> Result<Tag> {
        self.dict
            .get_by_name(name)
            .ok_or(Error::NoSuchAttributeName)
            .map(|e| e.tag())
    }

    /// Insert a data element to the object.
    pub fn put(&mut self, elt: DataElement) {
        self.entries.insert(elt.tag(), elt);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use object::DicomObject;
    use data::VR;

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
            DicomValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::create_empty();
        obj.put(another_patient_name.clone());
        let elem1 = (&obj).get_element(Tag(0x0010, 0x0010)).unwrap();
        assert_eq!(elem1, &another_patient_name);
    }

    #[test]
    fn inmem_object_get_by_name() {
        let another_patient_name = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            DicomValue::Str("Doe^John".to_string()),
        );
        let mut obj = InMemDicomObject::create_empty();
        obj.put(another_patient_name.clone());
        let elem1 = (&obj).get_element_by_name("PatientName").unwrap();
        assert_eq!(elem1, &another_patient_name);
    }
}
