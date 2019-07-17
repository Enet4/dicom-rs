use std::marker::PhantomData;
use dicom_dictionary_std::StandardDataDictionary;
use crate::DicomObject;

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
    /// Construct a new DICOM loader
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
    pub fn with_dict(self, dict: D) -> Self {
        DicomLoaderOptions {
            dict: dict,
            phantom: PhantomData,
        }
    }

    pub fn with_std_dict(self) -> DicomLoaderOptions<StandardDataDictionary, O> {
        DicomLoaderOptions {
            dict: StandardDataDictionary,
            phantom: PhantomData,
        }
    }
}
