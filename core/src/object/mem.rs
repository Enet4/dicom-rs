//! This module contains the implementation for an in-memory DICOM object.

use std::collections::HashMap;

use super::DicomObject;
use data::{DataElement, Header, Tag};
use dictionary::{DataDictionary, DictionaryEntry, StandardDataDictionary, get_standard_dictionary};
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

impl<D> PartialEq for InMemDicomObject<D> {
    // This implementation ignores the data dictionary.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<D> DicomObject for InMemDicomObject<D> 
        where D: DataDictionary {
    type Element = DataElement;
    type Sequence = InMemSequence<D>;

    fn element(&self, tag: Tag) -> Result<&Self::Element> {
        self.entries.get(&tag).ok_or(Error::NoSuchDataElement)
    }

    fn element_by_name(&self, name: &str) -> Result<&Self::Element> {
        let tag = self.lookup_name(name)?;
        self.element(tag)
    }

    fn pixel_data<PV, PD: PixelData<PV>>(&self) -> Result<PD> {
        unimplemented!()
    }
}

impl InMemDicomObject<&'static StandardDataDictionary> {
    /// Create a new empty DICOM object.
    pub fn create_empty() -> Self {
        InMemDicomObject {
            entries: HashMap::new(),
            dict: get_standard_dictionary()
        }
    }
}

impl<D> InMemDicomObject<D>
    where D: DataDictionary
{

    /// Create a new empty object, using the given dictionary
    /// for name lookup.
    pub fn create_empty_with_dict(dict: D) -> Self {
        InMemDicomObject {
            entries: HashMap::new(),
            dict: dict
        }
    }

    fn lookup_name(&self, name: &str) -> Result<Tag> {
        self.dict.get_by_name(name)
            .ok_or(Error::NoSuchAttributeName)
            .map(|e| e.tag())
    }

    /// Insert a data element to the object.
    pub fn put(&mut self, elt: <Self as DicomObject>::Element) -> Result<()> {
        self.entries.insert(elt.tag(), elt);
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use data::VR;

    #[test]
    fn inmem_object() {
        let mut obj1 = InMemDicomObject::create_empty();
        let mut obj2 = InMemDicomObject::create_empty();
        assert_eq!(obj1, obj2);
        obj1.put(DataElement::empty(Tag(0x0010, 0x0010), VR::PN)).unwrap();
        assert!(obj1 != obj2);
        obj2.put(DataElement::empty(Tag(0x0010, 0x0010), VR::PN)).unwrap();
        assert_eq!(obj1, obj2);
    }
}
