use std::marker::PhantomData;
use dictionary::StandardDataDictionary;
use object::DicomObject;

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
    pub fn new() -> Self {
        DicomLoaderOptions {
            dict: StandardDataDictionary,
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
