//! This module contains the implementation for an in-memory DICOM object.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use meta::DicomMetaTable;

use super::DicomObject;
use data::{DataElement, Header, Tag};
use data::value::DicomValue;
use dictionary::{DataDictionary, DictionaryEntry, StandardDataDictionary, get_standard_dictionary};
use error::{Error, Result};
use object::pixeldata::PixelData;

/** A DICOM sequence that is fully contained in memory.
 */
#[derive(Debug, Clone)]
pub struct InMemSequence<'s, D> {
    tag: Tag,
    objects: Vec<InMemDicomObject<'s, D>>,
}

/** A DICOM object that is fully contained in memory.
 */
#[derive(Debug, Clone)]
pub struct InMemDicomObject<'s, D> {
    entries: HashMap<Tag, DataElement>,
    dict: D,
    self_phantom: PhantomData<&'s ()>,
}

impl<'s, D> PartialEq for InMemDicomObject<'s, D> {
    // This implementation ignores the data dictionary.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<'s, D: 's> DicomObject<'s> for InMemDicomObject<'s, D>
    where D: DataDictionary
{
    type Element = &'s DataElement;
    type Sequence = InMemSequence<'s, D>;

    fn get_element(&'s self, tag: Tag) -> Result<Self::Element> {
        self.entries
            .get(&tag)
            .ok_or(Error::NoSuchDataElement)
    }

    fn get_element_by_name(&'s self, name: &str) -> Result<Self::Element> {
        let tag = self.lookup_name(name)?;
        self.get_element(tag)
    }

    fn get_pixel_data<PV, PD: PixelData<PV>>(&'s self) -> Result<PD> {
        unimplemented!()
    }
}

impl<'s> InMemDicomObject<'s, &'static StandardDataDictionary> {
    /// Create a new empty DICOM object.
    pub fn create_empty() -> Self {
        InMemDicomObject {
            entries: HashMap::new(),
            dict: get_standard_dictionary(),
            self_phantom: PhantomData,
        }
    }

    /// Create a DICOM object by reading from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::from_file_with_dict(path, get_standard_dictionary())
    }
}

impl<'s, D> InMemDicomObject<'s, D>
    where D: DataDictionary
{
    /// Create a new empty object, using the given dictionary
    /// for name lookup.
    pub fn new_empty_with_dict(dict: D) -> Self {
        InMemDicomObject {
            entries: HashMap::new(),
            dict: dict,
            self_phantom: PhantomData,
        }
    }

    /// Create a DICOM object by reading from a file.
    pub fn from_file_with_dict<P: AsRef<Path>>(path: P, dict: D) -> Result<Self> {
        let mut file = BufReader::new(File::open(path)?);

        // read metadata header
        let meta = DicomMetaTable::from_readseek_stream(&mut file)?;
        // TODO feed data to object
        
        // TODO read rest of data according to metadata, feed it to object

        unimplemented!()
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
        let another_patient_name = DataElement::new(Tag(0x0010, 0x0010),
                                                    VR::PN,
                                                    DicomValue::Str("Doe^John".to_string()));
        let mut obj = InMemDicomObject::create_empty();
        obj.put(another_patient_name.clone());
        let elem1 = obj.get_element(Tag(0x0010, 0x0010)).unwrap();
        assert_eq!(elem1, &another_patient_name);
    }

    #[test]
    fn inmem_object_get_by_name() {
        let another_patient_name = DataElement::new(Tag(0x0010, 0x0010),
                                                    VR::PN,
                                                    DicomValue::Str("Doe^John".to_string()));
        let mut obj = InMemDicomObject::create_empty();
        obj.put(another_patient_name.clone());
        let elem1 = obj.get_element_by_name("PatientName").unwrap();
        assert_eq!(elem1, &another_patient_name);
    }
}
