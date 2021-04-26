//! This crate contains the Dicom pixeldata handlers and is
//! responsible for decoding pixeldata, such as JPEG-lossy and convert it
//! into a [`DynamicImage`] or raw [`DecodedPixelData`].
//!
//! This crate is using GDCM bindings to convert
//! different compression formats to raw pixeldata.
//! This should become a pure Rust implementation in the future.
//!
//! # Examples
//! ```no_run
//! # use std::error::Error;
//! use dicom_object::open_file;
//! use dicom_pixeldata::PixelDecoder;
//!
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let obj = open_file("dicom.dcm")?;
//! let image = obj.decode_pixel_data()?;
//! let dynamic_image = image.to_dynamic_image()?;
//! dynamic_image.save("out.png")?;
//! #   Ok(())
//! # }
//! ```

use byteorder::{ByteOrder, NativeEndian};
use dicom_core::{value::Value, DataDictionary};
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_object::{FileDicomObject, InMemDicomObject};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use gdcm_rs::{decode_single_frame_compressed, GDCMPhotometricInterpretation, GDCMTransferSyntax};
use image::{DynamicImage, ImageBuffer, Luma};
use ndarray::{Array, IxDyn};
use num_traits::NumCast;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use snafu::OptionExt;
use snafu::{ResultExt, Snafu};
use std::{borrow::Cow, str::FromStr};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Missing required element"))]
    MissingRequiredField { source: dicom_object::Error },

    #[snafu(display("Could not cast element"))]
    CastValueError {
        source: dicom_core::value::CastValueError,
    },

    #[snafu(display("Non supported GDCM PhotometricInterpretation: {}", pi))]
    GDCMNonSupportedPI {
        source: gdcm_rs::InvalidGDCMPI,
        pi: String,
    },

    #[snafu(display("Non supported GDCM TransferSyntax: {}", ts))]
    GDCMNonSupportedTS {
        source: gdcm_rs::InvalidGDCMTS,
        ts: String,
    },

    #[snafu(display("Invalid PixelData"))]
    InvalidPixelData,

    #[snafu(display("Invalid PixelRepresentation, must be 0 or 1"))]
    InvalidPixelRepresentation,

    #[snafu(display("Invalid BitsAllocated, must be 8 or 16"))]
    InvalidBitsAllocated,

    #[snafu(display("Unsupported PhotometricInterpretation {}", pi))]
    UnsupportedPhotometricInterpretation { pi: String },

    #[snafu(display("Unsupported SamplesPerPixel {}", spp))]
    UnsupportedSamplesPerPixel { spp: u16 },

    #[snafu(display("Unsupported TransferSyntax {}", ts))]
    UnsupportedTransferSyntax { ts: String },

    #[snafu(display("Multi-frame dicoms are not supported"))]
    UnsupportedMultiFrame,

    #[snafu(display("Invalid buffer when constructing ImageBuffer"))]
    InvalidImageBuffer,

    #[snafu(display("Unknown GDCM error while decoding image"))]
    UnknownGdcmError { source: gdcm_rs::Error },

    #[snafu(display("Invalid shape for ndarray"))]
    ShapeError { source: ndarray::ShapeError },

    #[snafu(display("Invalid data type for ndarray element"))]
    InvalidDataType,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Decoded pixel data
pub struct DecodedPixelData<'a> {
    pub data: Cow<'a, [u8]>,
    pub rows: u32,
    pub cols: u32,
    pub photometric_interpretation: String,
    pub samples_per_pixel: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
}

impl DecodedPixelData<'_> {
    /// Convert decoded pixel data into a DynamicImage.
    /// A new <u8> or <u16> vector is created in memory
    /// with normalized grayscale values
    pub fn to_dynamic_image(&self) -> Result<DynamicImage> {
        if self.photometric_interpretation != "MONOCHROME2" {
            UnsupportedPhotometricInterpretation {
                pi: self.photometric_interpretation.clone(),
            }
            .fail()?
        }

        if self.samples_per_pixel > 1 {
            // RGB, YBR, etc. color space
            UnsupportedSamplesPerPixel {
                spp: self.samples_per_pixel,
            }
            .fail()?
        }

        match self.bits_allocated {
            8 => {
                // Single grayscale channel
                let image_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
                    ImageBuffer::from_raw(self.cols, self.rows, self.data.to_vec())
                        .context(InvalidImageBuffer)?;
                Ok(DynamicImage::ImageLuma8(image_buffer))
            }
            16 => {
                let mut dest = vec![0; self.data.len() / 2];
                match self.pixel_representation {
                    // Unsigned 16 bit representation
                    0 => {
                        NativeEndian::read_u16_into(&self.data, &mut dest);

                        // Normalize values between 0 - u16::MAX
                        dest = normalize_u16(&dest);
                    }

                    // Signed 16 bit 2s complement representation
                    1 => {
                        let mut signed_buffer = vec![0; self.data.len() / 2];
                        NativeEndian::read_i16_into(&self.data, &mut signed_buffer);

                        // Normalize values between 0 - u16::MAX
                        dest = normalize_i16(&signed_buffer);
                    }
                    _ => InvalidPixelRepresentation.fail()?,
                }
                let image_buffer: ImageBuffer<Luma<u16>, Vec<u16>> =
                    ImageBuffer::from_raw(self.cols, self.rows, dest)
                        .context(InvalidImageBuffer)?;
                Ok(DynamicImage::ImageLuma16(image_buffer))
            }
            _ => InvalidBitsAllocated.fail()?,
        }
    }

    /// Convert decoded pixel data into an ndarray of a given type T.
    /// The pixel data type is extracted from the bits_allocated and
    /// pixel_representation, and automatically converted to the requested type T.
    pub fn to_ndarray<T>(&self) -> Result<Array<T, IxDyn>>
    where
        T: NumCast,
        T: Send,
    {
        // Array size is Rows x Cols x SamplesPerPixel (1 for grayscale, 3 for RGB)
        let shape = IxDyn(&[
            self.rows as usize,
            self.cols as usize,
            self.samples_per_pixel as usize,
        ]);

        match self.bits_allocated {
            8 => {
                // 1-channel Grayscale image
                let converted: Result<Vec<T>, _> = self
                    .data
                    .into_par_iter()
                    .map(|v| T::from(*v).ok_or(snafu::NoneError))
                    .collect();
                let converted = converted.context(InvalidDataType)?;
                let ndarray = Array::from_shape_vec(shape, converted).context(ShapeError)?;
                Ok(ndarray)
            }
            16 => match self.pixel_representation {
                // Unsigned 16 bit representation
                0 => {
                    let mut dest = vec![0; self.data.len() / 2];
                    NativeEndian::read_u16_into(&self.data, &mut dest);

                    let converted: Result<Vec<T>, _> = dest
                        .into_par_iter()
                        .map(|v| T::from(v).ok_or(snafu::NoneError))
                        .collect();
                    let converted = converted.context(InvalidDataType)?;
                    let ndarray = Array::from_shape_vec(shape, converted).context(ShapeError)?;
                    Ok(ndarray)
                }
                // Signed 16 bit 2s complement representation
                1 => {
                    let mut signed_buffer = vec![0; self.data.len() / 2];
                    NativeEndian::read_i16_into(&self.data, &mut signed_buffer);

                    let converted: Result<Vec<T>, _> = signed_buffer
                        .into_par_iter()
                        .map(|v| T::from(v).ok_or(snafu::NoneError))
                        .collect();
                    let converted = converted.context(InvalidDataType)?;
                    let ndarray = Array::from_shape_vec(shape, converted).context(ShapeError)?;
                    Ok(ndarray)
                }
                _ => InvalidPixelRepresentation.fail()?,
            },
            _ => InvalidBitsAllocated.fail()?,
        }
    }
}

// Normalize i16 vector to u16 vector using min/max normalization
fn normalize_i16(i: &[i16]) -> Vec<u16> {
    let min = *i.iter().min().unwrap() as f32;
    let max = *i.iter().max().unwrap() as f32;
    i.par_iter()
        .map(|p| (u16::MAX as f32 * (*p as f32 - min) / (max - min)) as u16)
        .collect()
}

// Normalize u16 vector using min/max normalization
fn normalize_u16(i: &[u16]) -> Vec<u16> {
    let min = *i.iter().min().unwrap() as f32;
    let max = *i.iter().max().unwrap() as f32;
    i.par_iter()
        .map(|p| (u16::MAX as f32 * (*p as f32 - min) / (max - min)) as u16)
        .collect()
}

pub trait PixelDecoder {
    /// Decode compressed pixel data.
    /// A new buffer (Vec<u8>) is created holding the decoded pixel data.
    fn decode_pixel_data(&self) -> Result<DecodedPixelData>;
}

impl<D> PixelDecoder for FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary + Clone,
{
    fn decode_pixel_data(&self) -> Result<DecodedPixelData> {
        let pixel_data = self
            .element(dicom_dictionary_std::tags::PIXEL_DATA)
            .context(MissingRequiredField)?;
        let cols = cols(self)?;
        let rows = rows(self)?;

        let photometric_interpretation = photometric_interpretation(self)?;
        let pi_type = GDCMPhotometricInterpretation::from_str(&photometric_interpretation)
            .context(GDCMNonSupportedPI {
                pi: &photometric_interpretation,
            })?;

        let transfer_syntax = &self.meta().transfer_syntax;
        let registry =
            TransferSyntaxRegistry
                .get(&&transfer_syntax)
                .context(UnsupportedTransferSyntax {
                    ts: transfer_syntax,
                })?;
        let ts_type =
            GDCMTransferSyntax::from_str(&registry.uid()).context(GDCMNonSupportedTS {
                ts: transfer_syntax,
            })?;

        let samples_per_pixel = samples_per_pixel(self)?;
        let bits_allocated = bits_allocated(self)?;
        let bits_stored = bits_stored(self)?;
        let high_bit = high_bit(self)?;
        let pixel_representation = pixel_representation(self)?;

        let decoded_pixel_data = match pixel_data.value() {
            Value::PixelSequence {
                fragments,
                offset_table: _,
            } => {
                if fragments.len() > 1 {
                    // Bundle fragments and decode multi-frame dicoms
                    UnsupportedMultiFrame.fail()?
                }
                let decoded_frame = decode_single_frame_compressed(
                    &fragments[0],
                    cols.into(),
                    rows.into(),
                    pi_type,
                    ts_type,
                    samples_per_pixel,
                    bits_allocated,
                    bits_stored,
                    high_bit,
                    pixel_representation,
                )
                .context(UnknownGdcmError)?;
                decoded_frame.to_vec()
            }
            Value::Primitive(p) => {
                // Non-encoded, just return the pixel data
                p.to_bytes().to_vec()
            }
            Value::Sequence { items: _, size: _ } => InvalidPixelData.fail()?,
        };

        Ok(DecodedPixelData {
            data: Cow::from(decoded_pixel_data),
            cols: cols.into(),
            rows: rows.into(),
            photometric_interpretation,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
        })
    }
}

/// Get the Columns of the dicom
fn cols<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::COLUMNS)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the Rows of the dicom
fn rows<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::ROWS)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the PhotoMetricInterpretation of the Dicom
fn photometric_interpretation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<String> {
    Ok(obj
        .element(dicom_dictionary_std::tags::PHOTOMETRIC_INTERPRETATION)
        .context(MissingRequiredField)?
        .string()
        .context(CastValueError)?
        .trim()
        .to_string())
}

/// Get the SamplesPerPixel of the Dicom
fn samples_per_pixel<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::SAMPLES_PER_PIXEL)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the BitsAllocated of the Dicom
fn bits_allocated<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_ALLOCATED)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the BitsStored of the Dicom
fn bits_stored<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::BITS_STORED)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the HighBit of the Dicom
fn high_bit<D: DataDictionary + Clone>(obj: &FileDicomObject<InMemDicomObject<D>>) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::HIGH_BIT)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

/// Get the PixelRepresentation of the Dicom
fn pixel_representation<D: DataDictionary + Clone>(
    obj: &FileDicomObject<InMemDicomObject<D>>,
) -> Result<u16> {
    obj.element(dicom_dictionary_std::tags::PIXEL_REPRESENTATION)
        .context(MissingRequiredField)?
        .uint16()
        .context(CastValueError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_object::open_file;
    use dicom_test_files;
    use rstest::rstest;
    use std::path::Path;

    #[rstest(value => [
         "pydicom/693_J2KI.dcm",
         "pydicom/693_J2KR.dcm",
         "pydicom/693_UNCI.dcm",
         "pydicom/693_UNCR.dcm",
         "pydicom/CT_small.dcm",
         "pydicom/JPEG-lossy.dcm",
         "pydicom/JPEG2000.dcm",
         "pydicom/JPEG2000_UNC.dcm",
         "pydicom/JPGLosslessP14SV1_1s_1f_8b.dcm",
         "pydicom/MR_small.dcm",
         "pydicom/MR_small_RLE.dcm",
         "pydicom/MR_small_implicit.dcm",
         "pydicom/MR_small_jp2klossless.dcm",
         "pydicom/MR_small_jpeg_ls_lossless.dcm",
         "pydicom/explicit_VR-UN.dcm",
         "pydicom/MR_small_bigendian.dcm",
         "pydicom/MR_small_expb.dcm",

        // "pydicom/ExplVR_BigEnd.dcm",
        // "pydicom/ExplVR_BigEndNoMeta.dcm",
        // "pydicom/ExplVR_LitEndNoMeta.dcm",
        // "pydicom/JPEG-LL.dcm",                       // More than 1 fragment
        // "pydicom/MR-SIEMENS-DICOM-WithOverlays.dcm", // Overlays not supported
        // "pydicom/MR2_J2KI.dcm",  // Multi-frame
        // "pydicom/MR2_J2KR.dcm",
        // "pydicom/MR2_UNCI.dcm",
        // "pydicom/MR2_UNCR.dcm",
        // "pydicom/MR_small_padded.dcm",
        // "pydicom/MR_truncated.dcm",
        // "pydicom/OBXXXX1A.dcm",
        // "pydicom/OBXXXX1A_2frame.dcm",
        // "pydicom/OBXXXX1A_expb.dcm",
        // "pydicom/OBXXXX1A_expb_2frame.dcm",
        // "pydicom/OBXXXX1A_rle.dcm",
        // "pydicom/OBXXXX1A_rle_2frame.dcm",
        // "pydicom/OT-PAL-8-face.dcm",
        // "pydicom/RG1_J2KI.dcm",
        // "pydicom/RG1_J2KR.dcm",
        // "pydicom/RG1_UNCI.dcm",
        // "pydicom/RG1_UNCR.dcm",
        // "pydicom/RG3_J2KI.dcm",
        // "pydicom/RG3_J2KR.dcm",
        // "pydicom/RG3_UNCI.dcm",
        // "pydicom/RG3_UNCR.dcm",
        // "pydicom/SC_rgb.dcm",
        // "pydicom/SC_rgb_16bit.dcm",
        // "pydicom/SC_rgb_16bit_2frame.dcm",
        // "pydicom/SC_rgb_2frame.dcm",
        // "pydicom/SC_rgb_32bit.dcm",
        // "pydicom/SC_rgb_32bit_2frame.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cr.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+n1.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+n2.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+np.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+s2.dcm",
        // "pydicom/SC_rgb_dcmtk_+eb+cy+s4.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcr_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcyn1_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcyn2_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcynp_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcys2_dcmd.dcm",
        // "pydicom/SC_rgb_dcmtk_ebcys4_dcmd.dcm",
        // "pydicom/SC_rgb_expb.dcm",
        // "pydicom/SC_rgb_expb_16bit.dcm",
        // "pydicom/SC_rgb_expb_16bit_2frame.dcm",
        // "pydicom/SC_rgb_expb_2frame.dcm",
        // "pydicom/SC_rgb_expb_32bit.dcm",
        // "pydicom/SC_rgb_expb_32bit_2frame.dcm",
        // "pydicom/SC_rgb_gdcm2k_uncompressed.dcm",
        // "pydicom/SC_rgb_gdcm_KY.dcm",
        // "pydicom/SC_rgb_jpeg_dcmtk.dcm",
        // "pydicom/SC_rgb_jpeg_gdcm.dcm",
        // "pydicom/SC_rgb_jpeg_lossy_gdcm.dcm",
        // "pydicom/SC_rgb_rle.dcm",
        // "pydicom/SC_rgb_rle_16bit.dcm",
        // "pydicom/SC_rgb_rle_16bit_2frame.dcm",
        // "pydicom/SC_rgb_rle_2frame.dcm",
        // "pydicom/SC_rgb_rle_32bit.dcm",
        // "pydicom/SC_rgb_rle_32bit_2frame.dcm",
        // "pydicom/SC_rgb_small_odd.dcm",
        // "pydicom/SC_rgb_small_odd_jpeg.dcm",
        // "pydicom/SC_ybr_full_422_uncompressed.dcm",
        // "pydicom/SC_ybr_full_uncompressed.dcm",
        // "pydicom/US1_J2KI.dcm",
        // "pydicom/US1_J2KR.dcm",
        // "pydicom/US1_UNCI.dcm",
        // "pydicom/US1_UNCR.dcm",
        // "pydicom/badVR.dcm",
        // "pydicom/bad_sequence.dcm",
        // "pydicom/color-pl.dcm",
        // "pydicom/color-px.dcm",
        // "pydicom/color3d_jpeg_baseline.dcm",
        // "pydicom/eCT_Supplemental.dcm",
        // "pydicom/empty_charset_LEI.dcm",
        // "pydicom/emri_small.dcm",
        // "pydicom/emri_small_RLE.dcm",
        // "pydicom/emri_small_big_endian.dcm",
        // "pydicom/emri_small_jpeg_2k_lossless.dcm",
        // "pydicom/emri_small_jpeg_2k_lossless_too_short.dcm",
        // "pydicom/emri_small_jpeg_ls_lossless.dcm",
        // "pydicom/gdcm-US-ALOKA-16.dcm",
        // "pydicom/gdcm-US-ALOKA-16_big.dcm",
        // "pydicom/image_dfl.dcm",
        // "pydicom/liver.dcm",
        // "pydicom/liver_1frame.dcm",
        // "pydicom/liver_expb.dcm",
        // "pydicom/liver_expb_1frame.dcm",
        // "pydicom/meta_missing_tsyntax.dcm",
        // "pydicom/mlut_18.dcm",
        // "pydicom/nested_priv_SQ.dcm",
        // "pydicom/no_meta.dcm",
        // "pydicom/no_meta_group_length.dcm",
        // "pydicom/priv_SQ.dcm",
        // "pydicom/reportsi.dcm",
        // "pydicom/reportsi_with_empty_number_tags.dcm",
        // "pydicom/rtdose.dcm",
        // "pydicom/rtdose_1frame.dcm",
        // "pydicom/rtdose_expb.dcm",
        // "pydicom/rtdose_expb_1frame.dcm",
        // "pydicom/rtdose_rle.dcm",
        // "pydicom/rtdose_rle_1frame.dcm",
        // "pydicom/rtplan.dcm",
        // "pydicom/rtplan_truncated.dcm",
        // "pydicom/rtstruct.dcm",
        // "pydicom/test-SR.dcm",
        // "pydicom/vlut_04.dcm",
    ])]
    fn test_parse_dicom_pixel_data(value: &str) {
        let test_file = dicom_test_files::path(value).unwrap();
        println!("Parsing pixel data for {:?}", test_file);
        let obj = open_file(test_file).unwrap();
        let image = obj.decode_pixel_data().unwrap().to_dynamic_image().unwrap();
        image
            .save(format!(
                "../target/dicom_test_files/pydicom/{}.png",
                Path::new(value).file_stem().unwrap().to_str().unwrap()
            ))
            .unwrap();
    }

    #[test]
    fn test_to_ndarray_signed_word() {
        let test_file = dicom_test_files::path("pydicom/JPEG2000.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let ndarray = obj
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<i16>()
            .unwrap();
        assert_eq!(ndarray.shape(), &[1024, 256, 1]);
        assert_eq!(ndarray.len(), 262144);
        assert_eq!(ndarray[[260, 0, 0]], -3);
    }

    #[test]
    fn test_to_ndarray_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_16bit.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let ndarray = obj
            .decode_pixel_data()
            .unwrap()
            .to_ndarray::<u16>()
            .unwrap();
        assert_eq!(ndarray.shape(), &[100, 100, 3]);
        assert_eq!(ndarray.len(), 30000);
        assert_eq!(ndarray[[50, 80, 1]], 32896);
    }

    #[test]
    fn test_to_ndarray_error() {
        let test_file = dicom_test_files::path("pydicom/JPEG2000.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let ndarray = obj.decode_pixel_data().unwrap().to_ndarray::<u8>();
        if let Err(_) = ndarray {
            //
        } else {
            panic!("should fail");
        }
    }
}
