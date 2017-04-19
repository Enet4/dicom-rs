use std::marker::PhantomData;
use dictionary::{DataDictionary, get_standard_dictionary, StandardDataDictionary};
use object::DicomObject;

/// A data type
pub struct DicomLoaderOptions<D, O> {
    dict: D,
    phantom: PhantomData<O>,
}

impl<'s, O> DicomLoaderOptions<&'static StandardDataDictionary, O>
    where O: DicomObject<'s>
{
    pub fn new() -> Self {
        DicomLoaderOptions {
            dict: get_standard_dictionary(),
            phantom: PhantomData,
        }
    }
}

impl<'s, D, O> DicomLoaderOptions<D, O>
    where O: DicomObject<'s>
{
    pub fn with_dict(self, dict: D) -> Self {
        DicomLoaderOptions {
            dict: dict,
            phantom: PhantomData,
        }
    }

    pub fn with_std_dict(self) -> DicomLoaderOptions<&'static StandardDataDictionary, O> {
        DicomLoaderOptions {
            dict: get_standard_dictionary(),
            phantom: PhantomData,
        }
    }
}
