use crate::DicomObject;
use dicom_dictionary_std::StandardDataDictionary;
use std::marker::PhantomData;

/// A data type
#[derive(Debug)]
pub struct DicomLoaderOptions<D, O> {
    dict: D,
    phantom: PhantomData<O>,
}

impl<'s, O> DicomLoaderOptions<StandardDataDictionary, O>
where
    O: DicomObject,
{
    /// Construct a new DICOM loader with the standard data dictionary.
    pub fn new() -> Self {
        DicomLoaderOptions::default()
    }
}

impl<D, O> Default for DicomLoaderOptions<D, O>
where
    D: Default,
{
    fn default() -> Self {
        DicomLoaderOptions {
            dict: D::default(),
            phantom: PhantomData,
        }
    }
}

impl<'s, D, O> DicomLoaderOptions<D, O>
where
    O: DicomObject,
{
    pub fn with_dict<NewD>(self, dict: NewD) -> DicomLoaderOptions<NewD, O> {
        DicomLoaderOptions {
            dict,
            phantom: PhantomData,
        }
    }

    pub fn with_std_dict(self) -> DicomLoaderOptions<StandardDataDictionary, O> {
        self.with_dict(StandardDataDictionary)
    }
}
