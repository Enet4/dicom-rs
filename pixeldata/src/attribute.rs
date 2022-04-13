//! Utility module for fetching key attributes from a DICOM object.

use dicom_core::DataDictionary;
use dicom_object::{mem::InMemElement, FileDicomObject, InMemDicomObject};
use snafu::{Backtrace, ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum GetAttributeError {
    #[snafu(display("Missing required attribute `{}`", name))]
    MissingRequiredField {
        name: &'static str,
        #[snafu(backtrace)]
        source: dicom_object::Error,
    },

    #[snafu(display("Could not convert attribute `{}`", name))]
    CastValue {
        name: &'static str,
        source: dicom_core::value::CastValueError,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = GetAttributeError> = std::result::Result<T, E>;

/// Get the Columns from the DICOM object
pub fn cols<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::COLUMNS)
        .context(MissingRequiredFieldSnafu { name: "Columns" })?
        .uint16()
        .context(CastValueSnafu { name: "Columns" })
}

/// Get the Rows from the DICOM object
pub fn rows<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::ROWS)
        .context(MissingRequiredFieldSnafu { name: "Rows" })?
        .uint16()
        .context(CastValueSnafu { name: "Rows" })
}

/// Get the PhotoMetricInterpretation from the DICOM object
pub fn photometric_interpretation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<String> {
    Ok(obj
        .element(dicom_dictionary_std::tags::PHOTOMETRIC_INTERPRETATION)
        .context(MissingRequiredFieldSnafu {
            name: "PhotometricInterpretation",
        })?
        .string()
        .context(CastValueSnafu {
            name: "PhotometricInterpretation",
        })?
        .trim()
        .to_string())
}

/// Get the SamplesPerPixel from the DICOM object
pub fn samples_per_pixel<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::SAMPLES_PER_PIXEL)
        .context(MissingRequiredFieldSnafu {
            name: "SamplesPerPixel",
        })?
        .uint16()
        .context(CastValueSnafu {
            name: "SamplesPerPixel",
        })
}

/// Get the PlanarConfiguration from the DICOM object, returning 0 by default
#[cfg(not(feature = "gdcm"))]
pub fn planar_configuration<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> u16 {
    obj.element(dicom_dictionary_std::tags::PLANAR_CONFIGURATION)
        .map_or(Ok(0), |e| e.to_int())
        .unwrap_or(0)
}

/// Get the BitsAllocated from the DICOM object
pub fn bits_allocated<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_ALLOCATED)
        .context(MissingRequiredFieldSnafu {
            name: "BitsAllocated",
        })?
        .uint16()
        .context(CastValueSnafu {
            name: "BitsAllocated",
        })
}

/// Get the BitsStored from the DICOM object
pub fn bits_stored<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_STORED)
        .context(MissingRequiredFieldSnafu { name: "BitsStored" })?
        .uint16()
        .context(CastValueSnafu { name: "BitsStored" })
}

/// Get the HighBit from the DICOM object
pub fn high_bit<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::HIGH_BIT)
        .context(MissingRequiredFieldSnafu { name: "HighBit" })?
        .uint16()
        .context(CastValueSnafu { name: "HighBit" })
}

/// Get the PixelRepresentation from the DICOM object
pub fn pixel_representation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::PIXEL_REPRESENTATION)
        .context(MissingRequiredFieldSnafu {
            name: "PixelRepresentation",
        })?
        .uint16()
        .context(CastValueSnafu {
            name: "PixelRepresentation",
        })
}

/// Get the PixelData element from the DICOM object
pub fn pixel_data<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<&InMemElement<D>> {
    obj.element(dicom_dictionary_std::tags::PIXEL_DATA)
        .context(MissingRequiredFieldSnafu { name: "PixelData" })
}

/// Get the RescaleIntercept from the DICOM object or returns 0
pub fn rescale_intercept<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> i16 {
    obj.element(dicom_dictionary_std::tags::RESCALE_INTERCEPT)
        .map_or(Ok(0), |e| e.to_int())
        .unwrap_or(0)
}

/// Get the RescaleSlope from the DICOM object or returns 1.0
pub fn rescale_slope<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> f32 {
    obj.element(dicom_dictionary_std::tags::RESCALE_SLOPE)
        .map_or(Ok(1.0), |e| e.to_float32())
        .unwrap_or(1.0)
}

/// Get the NumberOfFrames from the DICOM object or returns 1
pub fn number_of_frames<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> u16 {
    obj.element(dicom_dictionary_std::tags::NUMBER_OF_FRAMES)
        .map_or(Ok(1), |e| e.to_int())
        .unwrap_or(1)
}
