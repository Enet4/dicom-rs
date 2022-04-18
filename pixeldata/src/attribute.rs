//! Utility module for fetching key attributes from a DICOM object.

use dicom_core::{DataDictionary, Tag};
use dicom_dictionary_std::tags;
use dicom_object::{mem::InMemElement, FileDicomObject, InMemDicomObject};
use snafu::{ensure, Backtrace, ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum GetAttributeError {
    #[snafu(display("Missing required attribute `{}`", name))]
    MissingRequiredField {
        name: &'static str,
        #[snafu(backtrace)]
        source: dicom_object::Error,
    },

    #[snafu(display("Could not get attribute `{}`", name))]
    CastValue {
        name: &'static str,
        source: dicom_core::value::CastValueError,
        backtrace: Backtrace,
    },

    #[snafu(display("Could not convert attribute `{}`", name))]
    ConvertValue {
        name: &'static str,
        source: dicom_core::value::ConvertValueError,
        backtrace: Backtrace,
    },

    #[snafu(display("Semantically invalid value `{}` for attribute `{}`", value, name))]
    InvalidValue {
        name: &'static str,
        value: String,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = GetAttributeError> = std::result::Result<T, E>;

/// Get the Columns from the DICOM object
pub fn cols<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    retrieve_required_u16(obj, tags::COLUMNS, "Columns")
}

/// Get the Rows from the DICOM object
pub fn rows<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    retrieve_required_u16(obj, tags::ROWS, "Rows")
}

/// Get the PhotoMetricInterpretation from the DICOM object
pub fn photometric_interpretation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<String> {
    Ok(obj
        .element(tags::PHOTOMETRIC_INTERPRETATION)
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

/// Get the VOILUTFunction from the DICOM object
pub fn voi_lut_function<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<Option<String>> {
    let elem = match obj.element(tags::VOILUT_FUNCTION) {
        Ok(e) => e,
        Err(dicom_object::Error::NoSuchDataElementTag { .. }) => return Ok(None),
        Err(e) => {
            return Err(e).context(MissingRequiredFieldSnafu {
                name: "VOILUTFunction",
            })
        }
    };

    let value = elem
        .string()
        .context(CastValueSnafu {
            name: "VOILUTFunction",
        })?
        .trim()
        .to_string();
    Ok(Some(value))
}

/// Get the SamplesPerPixel from the DICOM object
pub fn samples_per_pixel<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::SAMPLES_PER_PIXEL, "SamplesPerPixel")
}

/// Get the PlanarConfiguration from the DICOM object, returning 0 by default
#[cfg(not(feature = "gdcm"))]
pub fn planar_configuration<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> u16 {
    obj.element(tags::PLANAR_CONFIGURATION)
        .map_or(Ok(0), |e| e.to_int())
        .unwrap_or(0)
}

/// Get the BitsAllocated from the DICOM object
pub fn bits_allocated<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::BITS_ALLOCATED, "BitsAllocated")
}

/// Get the BitsStored from the DICOM object
pub fn bits_stored<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::BITS_STORED, "BitsStored")
}

/// Get the HighBit from the DICOM object
pub fn high_bit<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::HIGH_BIT, "HighBit")
}

/// Get the PixelData element from the DICOM object
pub fn pixel_data<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<&InMemElement<D>> {
    obj.element(tags::PIXEL_DATA)
        .context(MissingRequiredFieldSnafu { name: "PixelData" })
}

/// Get the RescaleIntercept from the DICOM object or returns 0
pub fn rescale_intercept<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> f64 {
    obj.element(tags::RESCALE_INTERCEPT)
        .map_or(Ok(0.), |e| e.to_float64())
        .unwrap_or(0.)
}

/// Get the RescaleSlope from the DICOM object or returns 1.0
pub fn rescale_slope<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> f64 {
    obj.element(tags::RESCALE_SLOPE)
        .map_or(Ok(1.0), |e| e.to_float64())
        .unwrap_or(1.0)
}

/// Get the NumberOfFrames from the DICOM object,
/// returning 1 if it is not present
pub fn number_of_frames<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u32> {
    let elem = match obj.element(tags::NUMBER_OF_FRAMES) {
        Ok(e) => e,
        Err(dicom_object::Error::NoSuchDataElementTag { .. }) => return Ok(1),
        Err(e) => {
            return Err(e).context(MissingRequiredFieldSnafu {
                name: "NumberOfFrames",
            })
        }
    };

    let integer = elem.to_int::<i32>().context(ConvertValueSnafu {
        name: "NumberOfFrames",
    })?;

    ensure!(
        integer > 0,
        InvalidValueSnafu {
            name: "NumberOfFrames",
            value: integer.to_string(),
        }
    );

    Ok(integer as u32)
}

/// Retrieve the WindowCenter from the DICOM object if it exists.
pub fn window_center<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<Option<f64>> {
    retrieve_optional_to_f64(obj, tags::WINDOW_CENTER, "WindowCenter")
}

/// Retrieve the WindowWidth from the DICOM object if it exists.
pub fn window_width<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<Option<f64>> {
    retrieve_optional_to_f64(obj, tags::WINDOW_WIDTH, "WindowWidth")
}

#[inline]
fn retrieve_required_u16<D>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
    tag: Tag,
    name: &'static str,
) -> Result<u16>
where
    D: DataDictionary + Clone,
{
    obj.element(tag)
        .context(MissingRequiredFieldSnafu { name })?
        .uint16()
        .context(CastValueSnafu { name })
}

#[inline]
fn retrieve_optional_to_f64<D>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
    tag: Tag,
    name: &'static str,
) -> Result<Option<f64>>
where
    D: DataDictionary + Clone,
{
    let elem = match obj.element(tag) {
        Ok(e) => e,
        Err(dicom_object::Error::NoSuchDataElementTag { .. }) => return Ok(None),
        Err(e) => return Err(e).context(MissingRequiredFieldSnafu { name }),
    };

    elem.to_float64()
        .context(ConvertValueSnafu { name })
        .map(Some)
}

/// An interpreted representation of the DICOM _Pixel Representation_ attribute.
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
pub enum PixelRepresentation {
    /// unsigned pixel data sample values
    Unsigned,
    /// signed pixel data sample values
    Signed,
}

/// Get the PixelRepresentation from the DICOM object
pub fn pixel_representation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<PixelRepresentation> {
    let p = retrieve_required_u16(obj, tags::PIXEL_REPRESENTATION, "PixelRepresentation")?;

    match p {
        0 => Ok(PixelRepresentation::Unsigned),
        1 => Ok(PixelRepresentation::Signed),
        _ => InvalidValueSnafu {
            name: "PixelRepresentation",
            value: p.to_string(),
        }
        .fail(),
    }
}
