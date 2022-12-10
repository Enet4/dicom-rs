//! Utility module for fetching key attributes from a DICOM object.

use dicom_core::{DataDictionary, Tag};
use dicom_dictionary_std::tags;
use dicom_object::{mem::InMemElement, FileDicomObject, InMemDicomObject};
use snafu::{ensure, Backtrace, ResultExt, Snafu, OptionExt};
use std::fmt;

/// An enum for a DICOM attribute which can be retrieved
/// for the purposes of decoding pixel data.
/// 
/// Since the set of attributes needed is more constrained,
/// this is a more compact representation than a tag or a static string.
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum AttributeName {
    Columns,
    Rows,
    BitsAllocated,
    BitsStored,
    HighBit,
    NumberOfFrames,
    PhotometricInterpretation,
    PixelData,
    PixelRepresentation,
    PlanarConfiguration,
    SamplesPerPixel,
    VoiLutFunction,
    WindowCenter,
    WindowWidth,
}

impl std::fmt::Display for AttributeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttributeName::VoiLutFunction => f.write_str("VOILUTFunction"),
            _ => std::fmt::Debug::fmt(self, f)
        }
    }
}

#[derive(Debug, Snafu)]
pub enum GetAttributeError {
    #[snafu(display("Missing required attribute `{}`", name))]
    MissingRequired {
        name: AttributeName,
        backtrace: Backtrace,
    },

    #[snafu(display("Could not retrieve attribute `{}`", name))]
    Retrieve {
        name: AttributeName,
        #[snafu(backtrace)]
        source: dicom_object::Error,
    },

    #[snafu(display("Could not get attribute `{}`", name))]
    CastValue {
        name: AttributeName,
        source: dicom_core::value::CastValueError,
        backtrace: Backtrace,
    },

    #[snafu(display("Could not convert attribute `{}`", name))]
    ConvertValue {
        name: AttributeName,
        source: dicom_core::value::ConvertValueError,
        backtrace: Backtrace,
    },

    #[snafu(display("Semantically invalid value `{}` for attribute `{}`", value, name))]
    InvalidValue {
        name: AttributeName,
        value: String,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = GetAttributeError> = std::result::Result<T, E>;

/// Get the Columns from the DICOM object
pub fn cols<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    retrieve_required_u16(obj, tags::COLUMNS, AttributeName::Columns)
}

/// Get the Rows from the DICOM object
pub fn rows<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    retrieve_required_u16(obj, tags::ROWS, AttributeName::Rows)
}

/// Get the VOILUTFunction from the DICOM object
pub fn voi_lut_function<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<Option<String>> {
    let elem = if let Some(elem) =
        obj.element_opt(tags::VOILUT_FUNCTION)
            .context(RetrieveSnafu {
                name: AttributeName::VoiLutFunction,
            })? {
        elem
    } else {
        return Ok(None);
    };

    let value = elem
        .string()
        .context(CastValueSnafu {
            name: AttributeName::VoiLutFunction,
        })?
        .trim()
        .to_string();
    Ok(Some(value))
}

/// Get the SamplesPerPixel from the DICOM object
pub fn samples_per_pixel<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::SAMPLES_PER_PIXEL, AttributeName::SamplesPerPixel)
}

/// Get the BitsAllocated from the DICOM object
pub fn bits_allocated<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::BITS_ALLOCATED, AttributeName::BitsAllocated)
}

/// Get the BitsStored from the DICOM object
pub fn bits_stored<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::BITS_STORED, AttributeName::BitsStored)
}

/// Get the HighBit from the DICOM object
pub fn high_bit<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    retrieve_required_u16(obj, tags::HIGH_BIT, AttributeName::HighBit)
}

/// Get the PixelData element from the DICOM object
pub fn pixel_data<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<&InMemElement<D>> {
    let name = AttributeName::PixelData;
    obj.element_opt(tags::PIXEL_DATA)
        .context(RetrieveSnafu { name })?
        .context(MissingRequiredSnafu { name })
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
    let name = AttributeName::NumberOfFrames;
    let elem = if let Some(elem) =
        obj.element_opt(tags::NUMBER_OF_FRAMES)
            .context(RetrieveSnafu { name })? {
        elem
    } else {
        return Ok(1);
    };

    let integer = elem.to_int::<i32>()
        .context(ConvertValueSnafu { name })?;

    ensure!(
        integer > 0,
        InvalidValueSnafu {
            name,
            value: integer.to_string(),
        }
    );

    Ok(integer as u32)
}

/// Retrieve the WindowCenter from the DICOM object if it exists.
pub fn window_center<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<Option<f64>> {
    retrieve_optional_to_f64(obj, tags::WINDOW_CENTER, AttributeName::WindowCenter)
}

/// Retrieve the WindowWidth from the DICOM object if it exists.
pub fn window_width<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<Option<f64>> {
    retrieve_optional_to_f64(obj, tags::WINDOW_WIDTH, AttributeName::WindowWidth)
}

#[inline]
fn retrieve_required_u16<D>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
    tag: Tag,
    name: AttributeName,
) -> Result<u16>
where
    D: DataDictionary + Clone,
{
    obj.element_opt(tag)
        .context(RetrieveSnafu { name })?
        .context(MissingRequiredSnafu { name })?
        .uint16()
        .context(CastValueSnafu { name })
}

#[inline]
fn retrieve_optional_to_f64<D>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
    tag: Tag,
    name: AttributeName,
) -> Result<Option<f64>>
where
    D: DataDictionary + Clone,
{
    match obj.element_opt(tag).context(RetrieveSnafu { name })? {
        Some(e) => e.to_float64().context(ConvertValueSnafu { name }).map(Some),
        None => Ok(None),
    }
}

/// A decoded representation of the DICOM _Pixel Representation_ attribute.
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum PixelRepresentation {
    /// 0: unsigned pixel data sample values
    Unsigned = 0,
    /// 1: signed pixel data sample values
    Signed = 1,
}

/// Get the PixelRepresentation from the DICOM object
pub fn pixel_representation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<PixelRepresentation> {
    let p = retrieve_required_u16(obj, tags::PIXEL_REPRESENTATION, AttributeName::PixelRepresentation)?;

    match p {
        0 => Ok(PixelRepresentation::Unsigned),
        1 => Ok(PixelRepresentation::Signed),
        _ => InvalidValueSnafu {
            name: AttributeName::PixelRepresentation,
            value: p.to_string(),
        }
        .fail(),
    }
}

/// A decoded representation of the DICOM _Planar Configuration_ attribute.
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum PlanarConfiguration {
    /// 0: Standard planar configuration.
    /// Each pixel is encoded contiguously.
    Standard = 0,
    /// 1: Pixel-first planar configuration.
    /// Each color plane is encoded contiguously.
    PixelFirst = 1,
}

impl fmt::Display for PlanarConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (*self as u16).fmt(f)
    }
}

/// Get the PlanarConfiguration from the DICOM object,
/// returning the standard planar configuration by default
#[cfg(not(feature = "gdcm"))]
pub fn planar_configuration<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<PlanarConfiguration> {
    let elem = if let Some(elem) =
        obj.element_opt(tags::PLANAR_CONFIGURATION)
            .context(RetrieveSnafu {
                name: AttributeName::PlanarConfiguration,
            })? {
        elem
    } else {
        return Ok(PlanarConfiguration::Standard);
    };

    let p = elem.to_int::<i16>().context(ConvertValueSnafu {
        name: AttributeName::PlanarConfiguration,
    })?;

    match p {
        0 => Ok(PlanarConfiguration::Standard),
        1 => Ok(PlanarConfiguration::PixelFirst),
        _ => InvalidValueSnafu {
            name: AttributeName::PlanarConfiguration,
            value: p.to_string(),
        }
        .fail(),
    }
}

/// A decoded representation of the
/// DICOM _Photometric Interpretation_ attribute.
///
/// See [section C.7.6.3][1] of the standard
/// for more details about each photometric interpretation.
///
/// In the event that the photometric interpretation is not
/// any of the specified variants,
/// the `Other` variant is used.
/// Note that this enumeration covers the ones which are known,
/// not necessarily supported in the decoding process.
///
/// [1]: https://dicom.nema.org/medical/dicom/current/output/chtml/part03/sect_C.7.6.3.html#sect_C.7.6.3.1.2
#[derive(Debug, Clone, PartialEq)]
pub enum PhotometricInterpretation {
    /// `MONOCHROME1`:
    /// Pixel data represent a single monochrome image plane.
    /// The minimum sample value is intended to be displayed as white.
    Monochrome1,
    /// `MONOCHROME2`:
    /// Pixel data represent a single monochrome image plane.
    /// The minimum sample value is intended to be displayed as black.
    Monochrome2,
    /// `PALETTE COLOR`:
    /// Pixel data describe a color image with a single sample per pixel
    /// (single image plane).
    PaletteColor,
    /// `RGB`:
    /// Pixel data represent a color image described by
    /// red, green, and blue image planes.
    Rgb,
    /// `YBR_FULL`:
    /// Pixel data represent a color image described by
    /// one luminance (Y) and two chrominance planes (CB and CR)
    /// and as a result there are half as many CB and CR values as Y values.
    YbrFull,
    /// `YBR_FULL_422`:
    /// The same as YBR_FULL except that the CB and CR values
    /// are sampled horizontally at half the Y rate.
    YbrFull422,
    /// `YBR_PARTIAL_420`:
    /// Pixel data represent a color image described by
    /// one luminance (Y) and two chrominance planes (CB and CR).
    /// The CB and CR values are sampled
    /// horizontally and vertically at half the Y rate
    /// and as a result there are four times less CB and CR values than Y values.
    YbrPartial420,
    /// `YBR_ICT`:
    /// Irreversible Color Transformation.
    /// Pixel data represent a color image described by
    /// one luminance (Y) and two chrominance planes (CB and CR).
    YbrIct,
    /// `YBR_RCT`:
    /// Rreversible Color Transformation.
    /// Pixel data represent a color image described by
    /// one luminance (Y) and two chrominance planes (CB and CR).
    YbrRct,
    /// The photometric interpretation is not one of the known variants.
    ///
    /// **Note:** this value is assumed to be different from
    /// any other variant listed above,
    /// and no checks are made to ensure this assumption.
    /// The construction of `PhotometricInterpretation::Other`
    /// when one of the existing variants is applicable
    /// is considered a bug.
    ///
    /// **Note 2:** subsequent crate versions may introduce new variants,
    /// and as a consequence break any user depending on the `Other` variant.
    /// If you need to depend on an unspecified variant,
    /// you should also double check the photometric interpretations here
    /// every time the crate is updated.
    Other(String),
}

impl PhotometricInterpretation {
    /// Obtain a string representation of the photometric interpretation.
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    /// Get whether this photometric interpretation is
    /// one of the monochrome variants
    /// (`MONOCHROME1` or `MONOCHROME2`).
    pub fn is_monochrome(&self) -> bool {
        matches!(
            self,
            PhotometricInterpretation::Monochrome1 | PhotometricInterpretation::Monochrome2
        )
    }
}

impl AsRef<str> for PhotometricInterpretation {
    fn as_ref(&self) -> &str {
        match self {
            PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
            PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
            PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
            PhotometricInterpretation::Rgb => "RGB",
            PhotometricInterpretation::YbrFull => "YBR_FULL",
            PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
            PhotometricInterpretation::YbrPartial420 => "YBR_PARTIAL_420",
            PhotometricInterpretation::YbrIct => "YBR_ICT",
            PhotometricInterpretation::YbrRct => "YBR_RCT",
            PhotometricInterpretation::Other(s) => s,
        }
    }
}

impl From<String> for PhotometricInterpretation {
    fn from(s: String) -> Self {
        match s.as_str() {
            "MONOCHROME1" => PhotometricInterpretation::Monochrome1,
            "MONOCHROME2" => PhotometricInterpretation::Monochrome2,
            "PALETTE COLOR" => PhotometricInterpretation::PaletteColor,
            "RGB" => PhotometricInterpretation::Rgb,
            "YBR_FULL" => PhotometricInterpretation::YbrFull,
            "YBR_FULL_422" => PhotometricInterpretation::YbrFull422,
            "YBR_PARTIAL_420" => PhotometricInterpretation::YbrPartial420,
            "YBR_ICT" => PhotometricInterpretation::YbrIct,
            "YBR_RCT" => PhotometricInterpretation::YbrRct,
            _ => PhotometricInterpretation::Other(s),
        }
    }
}

impl From<&str> for PhotometricInterpretation {
    fn from(s: &str) -> Self {
        match s {
            "MONOCHROME1" => PhotometricInterpretation::Monochrome1,
            "MONOCHROME2" => PhotometricInterpretation::Monochrome2,
            "PALETTE COLOR" => PhotometricInterpretation::PaletteColor,
            "RGB" => PhotometricInterpretation::Rgb,
            "YBR_FULL" => PhotometricInterpretation::YbrFull,
            "YBR_FULL_422" => PhotometricInterpretation::YbrFull422,
            "YBR_PARTIAL_420" => PhotometricInterpretation::YbrPartial420,
            "YBR_ICT" => PhotometricInterpretation::YbrIct,
            "YBR_RCT" => PhotometricInterpretation::YbrRct,
            _ => PhotometricInterpretation::Other(s.to_string()),
        }
    }
}

impl fmt::Display for PhotometricInterpretation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhotometricInterpretation::Monochrome1 => f.write_str("MONOCHROME1"),
            PhotometricInterpretation::Monochrome2 => f.write_str("MONOCHROME2"),
            PhotometricInterpretation::PaletteColor => f.write_str("PALETTE COLOR"),
            PhotometricInterpretation::Rgb => f.write_str("RGB"),
            PhotometricInterpretation::YbrFull => f.write_str("YBR_FULL"),
            PhotometricInterpretation::YbrFull422 => f.write_str("YBR_FULL_422"),
            PhotometricInterpretation::YbrPartial420 => f.write_str("YBR_PARTIAL_420"),
            PhotometricInterpretation::YbrIct => f.write_str("YBR_ICT"),
            PhotometricInterpretation::YbrRct => f.write_str("YBR_RCT"),
            PhotometricInterpretation::Other(s) => f.write_str(s),
        }
    }
}

/// Get the PhotoMetricInterpretation from the DICOM object
pub fn photometric_interpretation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<PhotometricInterpretation> {
    let name = AttributeName::PhotometricInterpretation;
    Ok(obj
        .element_opt(tags::PHOTOMETRIC_INTERPRETATION)
        .context(RetrieveSnafu { name })?
        .context(MissingRequiredSnafu { name })?
        .string()
        .context(CastValueSnafu { name })?
        .trim()
        .into())
}

#[cfg(test)]
mod tests {

    #[test]
    fn errors_are_not_too_large() {
        let size = std::mem::size_of::<super::GetAttributeError>();
        assert!(size <= 64, "GetAttributeError size is too large ({} > 64)", size);
    }
}