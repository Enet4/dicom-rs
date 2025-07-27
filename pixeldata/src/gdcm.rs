//! Decode pixel data using GDCM when the default features are enabled.

use crate::{
    DecodePixelDataSnafu, DecodedPixelData, InvalidPixelDataSnafu,
    LengthMismatchRescaleSnafu, LengthMismatchWindowLevelSnafu, PixelDecoder, Rescale, Result,
    UnknownTransferSyntaxSnafu, UnsupportedPhotometricInterpretationSnafu,
    UnsupportedTransferSyntaxSnafu, VoiLutFunction, WindowLevel,
};
use dicom_core::{DataDictionary, DicomValue};
use dicom_dictionary_std::tags;
use dicom_encoding::{adapters::DecodeError, transfer_syntax::TransferSyntaxIndex};
use dicom_object::{FileDicomObject, InMemDicomObject};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use gdcm_rs::{
    decode_multi_frame_compressed, decode_single_frame_compressed, Error as GDCMError,
    GDCMPhotometricInterpretation, GDCMTransferSyntax,
};
use snafu::{ensure, OptionExt, ResultExt};
use std::{borrow::Cow, convert::TryFrom, iter::zip, str::FromStr};

impl<D> PixelDecoder for FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary + Clone,
{
    fn decode_pixel_data(&self) -> Result<DecodedPixelData<'_>> {
        use super::attribute::*;

        let pixel_data = pixel_data(self)?;

        let cols = cols(self)?;
        let rows = rows(self)?;

        let photometric_interpretation =
            photometric_interpretation(self)?;
        let pi_type = match photometric_interpretation {
            PhotometricInterpretation::PaletteColor => GDCMPhotometricInterpretation::PALETTE_COLOR,
            _ => GDCMPhotometricInterpretation::from_str(photometric_interpretation.as_str())
                .map_err(|_| {
                    UnsupportedPhotometricInterpretationSnafu {
                        pi: photometric_interpretation.clone(),
                    }
                    .build()
                })?,
        };

        let transfer_syntax = &self.meta().transfer_syntax;
        let registry =
            TransferSyntaxRegistry
                .get(transfer_syntax)
                .context(UnknownTransferSyntaxSnafu {
                    ts_uid: transfer_syntax,
                })?;
        let ts_type = GDCMTransferSyntax::from_str(registry.uid()).map_err(|_| {
            UnsupportedTransferSyntaxSnafu {
                ts: transfer_syntax.clone(),
            }
            .build()
        })?;

        let samples_per_pixel = samples_per_pixel(self)?;
        let bits_allocated = bits_allocated(self)?;
        let bits_stored = bits_stored(self)?;
        let high_bit = high_bit(self)?;
        let pixel_representation = pixel_representation(self)?;
        let rescale_intercept = rescale_intercept(self);
        let rescale_slope = rescale_slope(self);
        let number_of_frames = number_of_frames(self)?;
        let voi_lut_function: Option<Vec<VoiLutFunction>> =
            voi_lut_function(self).unwrap_or(None).and_then(|fns| {
                fns.iter()
                    .map(|v| VoiLutFunction::try_from((*v).as_str()).ok())
                    .collect()
            });
        let voi_lut_sequence = voi_lut_sequence(self);

        ensure!(
            rescale_intercept.len() == rescale_slope.len(),
            LengthMismatchRescaleSnafu {
                slope_vm: rescale_slope.len() as u32,
                intercept_vm: rescale_intercept.len() as u32,
            }
        );

        let decoded_pixel_data = match pixel_data.value() {
            DicomValue::PixelSequence(v) => {
                let fragments = v.fragments();
                let gdcm_error_mapper = |source: GDCMError| DecodeError::Custom {
                    message: source.to_string(),
                    source: Some(Box::new(source)),
                };
                if fragments.len() > 1 {
                    // Bundle fragments and decode multi-frame dicoms
                    let dims = [cols.into(), rows.into(), number_of_frames];
                    let fragments: Vec<_> = fragments.iter().map(|frag| frag.as_slice()).collect();
                    decode_multi_frame_compressed(
                        fragments.as_slice(),
                        &dims,
                        pi_type,
                        ts_type,
                        samples_per_pixel,
                        bits_allocated,
                        bits_stored,
                        high_bit,
                        pixel_representation as u16,
                    )
                    .map_err(gdcm_error_mapper)
                    .context(DecodePixelDataSnafu)?
                    .to_vec()
                } else {
                    decode_single_frame_compressed(
                        &fragments[0],
                        cols.into(),
                        rows.into(),
                        pi_type,
                        ts_type,
                        samples_per_pixel,
                        bits_allocated,
                        bits_stored,
                        high_bit,
                        pixel_representation as u16,
                    )
                    .map_err(gdcm_error_mapper)
                    .context(DecodePixelDataSnafu)?
                    .to_vec()
                }
            }
            DicomValue::Primitive(p) => {
                // Non-encoded, just return the pixel data of the first frame
                p.to_bytes().to_vec()
            }
            DicomValue::Sequence(_) => InvalidPixelDataSnafu.fail()?,
        };


        let rescale = zip(&rescale_intercept, &rescale_slope)
            .map(|(intercept, slope)| Rescale {
                intercept: *intercept,
                slope: *slope,
            })
            .collect();

        let window = if let Some(wcs) = window_center(&self) {
            let width = window_width(&self);
            if let Some(wws) = width {
                ensure!(
                    wcs.len() == wws.len(),
                    LengthMismatchWindowLevelSnafu {
                        wc_vm: wcs.len() as u32,
                        ww_vm: wws.len() as u32,
                    }
                );
                Some(
                    zip(wcs, wws)
                        .map(|(wc, ww)| WindowLevel {
                            center: wc,
                            width: ww,
                        })
                        .collect(),
                )
            } else {
                None
            }
        } else {
            None
        };

        Ok(DecodedPixelData {
            data: Cow::from(decoded_pixel_data),
            cols: cols.into(),
            rows: rows.into(),
            number_of_frames,
            photometric_interpretation,
            samples_per_pixel,
            planar_configuration: PlanarConfiguration::Standard,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            rescale,
            voi_lut_function,
            window,
            voi_lut_sequence,
            enforce_frame_fg_vm_match: false,
        })
    }

    fn decode_pixel_data_frame(&self, frame: u32) -> Result<DecodedPixelData<'_>> {
        use super::attribute::*;

        let pixel_data = pixel_data(self)?;

        let cols = cols(self)?;
        let rows = rows(self)?;

        let photometric_interpretation =
            photometric_interpretation(self)?;
        let pi_type = match photometric_interpretation {
            PhotometricInterpretation::PaletteColor => GDCMPhotometricInterpretation::PALETTE_COLOR,
            _ => GDCMPhotometricInterpretation::from_str(photometric_interpretation.as_str())
                .map_err(|_| {
                    UnsupportedPhotometricInterpretationSnafu {
                        pi: photometric_interpretation.clone(),
                    }
                    .build()
                })?,
        };

        let transfer_syntax = &self.meta().transfer_syntax;
        let registry =
            TransferSyntaxRegistry
                .get(transfer_syntax)
                .context(UnknownTransferSyntaxSnafu {
                    ts_uid: transfer_syntax,
                })?;
        let ts_type = GDCMTransferSyntax::from_str(registry.uid()).map_err(|_| {
            UnsupportedTransferSyntaxSnafu {
                ts: transfer_syntax.clone(),
            }
            .build()
        })?;

        let samples_per_pixel = samples_per_pixel(self)?;
        let bits_allocated = bits_allocated(self)?;
        let bits_stored = bits_stored(self)?;
        let high_bit = high_bit(self)?;
        let pixel_representation = pixel_representation(self)?;
        let planar_configuration = if let Ok(el) = self.element(tags::PLANAR_CONFIGURATION) {
            el.uint16().unwrap_or(0)
        } else {
            0
        };
        let rescale_intercept = rescale_intercept(self);
        let rescale_slope = rescale_slope(self);
        let number_of_frames = number_of_frames(self)?;
        let voi_lut_function: Option<Vec<VoiLutFunction>> =
            voi_lut_function(self).unwrap_or(None).and_then(|fns| {
                fns.iter()
                    .map(|v| VoiLutFunction::try_from((*v).as_str()).ok())
                    .collect()
            });
        let voi_lut_sequence = voi_lut_sequence(self);

        let decoded_pixel_data = match pixel_data.value() {
            DicomValue::PixelSequence(v) => {
                let fragments = v.fragments();
                let gdcm_error_mapper = |source: GDCMError| DecodeError::Custom {
                    message: source.to_string(),
                    source: Some(Box::new(source)),
                };

                let frame = frame as usize;
                let data = if number_of_frames == 1 && fragments.len() > 1 {
                    fragments.iter().flat_map(|frame| frame.to_vec()).collect()
                } else {
                    fragments[frame].to_vec()
                };

                match ts_type {
                    GDCMTransferSyntax::ImplicitVRLittleEndian
                    | GDCMTransferSyntax::ExplicitVRLittleEndian => {
                        // This is just in case of encapsulated uncompressed data
                        let frame_size = cols * rows * samples_per_pixel * (bits_allocated / 8);
                        data.chunks_exact(frame_size as usize)
                            .nth(frame)
                            .map(|frame| frame.to_vec())
                            .unwrap_or_default()
                    }
                    _ => {
                        let buffer = [data.as_slice()];
                        let dims = [cols.into(), rows.into(), 1];

                        decode_multi_frame_compressed(
                            &buffer,
                            &dims,
                            pi_type,
                            ts_type,
                            samples_per_pixel,
                            bits_allocated,
                            bits_stored,
                            high_bit,
                            pixel_representation as u16,
                        )
                        .map_err(gdcm_error_mapper)
                        .context(DecodePixelDataSnafu)?
                        .to_vec()
                    }
                }
            }
            DicomValue::Primitive(p) => {
                // Uncompressed data
                let frame_size = cols as usize
                    * rows as usize
                    * samples_per_pixel as usize
                    * (bits_allocated as usize / 8);
                p.to_bytes()
                    .chunks_exact(frame_size)
                    .nth(frame as usize)
                    .map(|frame| frame.to_vec())
                    .unwrap_or_default()
            }
            DicomValue::Sequence(_) => InvalidPixelDataSnafu.fail()?,
        };

        // Convert to PlanarConfiguration::Standard
        let decoded_pixel_data = if planar_configuration == 1 && samples_per_pixel == 3 {
            interleave_planes(
                cols as usize,
                rows as usize,
                bits_allocated as usize,
                decoded_pixel_data,
            )
        } else {
            decoded_pixel_data
        };

        let window = if let Some(wcs) = window_center(self) {
            let width = window_width(self);
            if let Some(wws) = width {
                ensure!(
                    wcs.len() == wws.len(),
                    LengthMismatchWindowLevelSnafu {
                        wc_vm: wcs.len() as u32,
                        ww_vm: wws.len() as u32,
                    }
                );
                Some(
                    zip(wcs, wws)
                        .map(|(wc, ww)| WindowLevel {
                            center: wc,
                            width: ww,
                        })
                        .collect(),
                )
            } else {
                None
            }
        } else {
            None
        };
        let rescale = zip(&rescale_intercept, &rescale_slope)
            .map(|(intercept, slope)| Rescale {
                intercept: *intercept,
                slope: *slope,
            })
            .collect();

        Ok(DecodedPixelData {
            data: Cow::from(decoded_pixel_data),
            cols: cols.into(),
            rows: rows.into(),
            number_of_frames: 1,
            photometric_interpretation,
            samples_per_pixel,
            planar_configuration: PlanarConfiguration::Standard,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            rescale: rescale,
            voi_lut_function,
            window,
            voi_lut_sequence,
            enforce_frame_fg_vm_match: false,
        })
    }
}

fn interleave_planes(cols: usize, rows: usize, bits_allocated: usize, data: Vec<u8>) -> Vec<u8> {
    let frame_size = cols * rows * (bits_allocated / 8);
    let mut interleaved = Vec::with_capacity(data.len());

    let mut i = 0;
    while i < frame_size {
        interleaved.push(data[i]);
        if bits_allocated > 8 {
            interleaved.push(data[i + 1])
        }

        interleaved.push(data[i + frame_size]);
        if bits_allocated > 8 {
            interleaved.push(data[i + frame_size + 1])
        }

        interleaved.push(data[i + frame_size * 2]);
        if bits_allocated > 8 {
            interleaved.push(data[i + frame_size * 2 + 1])
        }

        i = if bits_allocated > 8 { i + 2 } else { i + 1 };
    }

    interleaved
}

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "ndarray", feature = "image"))]
    use super::*;
    #[cfg(any(feature = "ndarray", feature = "image"))]
    use dicom_object::open_file;
    #[cfg(feature = "image")]
    use rstest::rstest;
    #[cfg(feature = "image")]
    use std::path::Path;

    #[cfg(feature = "image")]
    const MAX_TEST_FRAMES: u32 = 16;

    #[cfg(feature = "image")]
    #[rstest]
    #[case("pydicom/693_J2KI.dcm")]
    #[case("pydicom/693_J2KR.dcm")]
    #[case("pydicom/693_UNCI.dcm")]
    #[case("pydicom/693_UNCR.dcm")]
    #[case("pydicom/CT_small.dcm")]
    #[case("pydicom/JPEG-lossy.dcm")]
    #[case("pydicom/JPEG2000.dcm")]
    #[case("pydicom/JPEG2000_UNC.dcm")]
    #[case("pydicom/JPGLosslessP14SV1_1s_1f_8b.dcm")]
    #[case("pydicom/MR_small.dcm")]
    #[case("pydicom/MR_small_RLE.dcm")]
    #[case("pydicom/MR_small_implicit.dcm")]
    #[case("pydicom/MR_small_jp2klossless.dcm")]
    #[case("pydicom/MR_small_jpeg_ls_lossless.dcm")]
    #[case("pydicom/explicit_VR-UN.dcm")]
    #[case("pydicom/MR_small_bigendian.dcm")]
    #[case("pydicom/MR_small_expb.dcm")]
    #[case("pydicom/SC_rgb.dcm")]
    #[case("pydicom/SC_rgb_16bit.dcm")]
    #[case("pydicom/SC_rgb_dcmtk_+eb+cr.dcm")]
    #[case("pydicom/SC_rgb_expb.dcm")]
    #[case("pydicom/SC_rgb_expb_16bit.dcm")]
    #[case("pydicom/SC_rgb_gdcm2k_uncompressed.dcm")]
    #[case("pydicom/SC_rgb_gdcm_KY.dcm")]
    #[case("pydicom/SC_rgb_jpeg_gdcm.dcm")]
    #[case("pydicom/SC_rgb_jpeg_lossy_gdcm.dcm")]
    #[case("pydicom/SC_rgb_rle.dcm")]
    #[case("pydicom/SC_rgb_rle_16bit.dcm")]
    #[case("pydicom/color-pl.dcm")]
    #[case("pydicom/color-px.dcm")]
    #[case("pydicom/SC_ybr_full_uncompressed.dcm")]
    #[case("pydicom/color3d_jpeg_baseline.dcm")]
    #[case("pydicom/emri_small_jpeg_ls_lossless.dcm")]
    fn test_parse_dicom_pixel_data(#[case] value: &str) {
        let test_file = dicom_test_files::path(value).unwrap();
        println!("Parsing pixel data for {}", test_file.display());
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        let output_dir =
            Path::new("../target/dicom_test_files/_out/test_gdcm_parse_dicom_pixel_data");
        std::fs::create_dir_all(output_dir).unwrap();

        for i in 0..pixel_data.number_of_frames.min(MAX_TEST_FRAMES) {
            let image = pixel_data.to_dynamic_image(i).unwrap();
            let image_path = output_dir.join(format!(
                "{}-{}.png",
                Path::new(value).file_stem().unwrap().to_str().unwrap(),
                i,
            ));
            image.save(image_path).unwrap();
        }
    }

    #[cfg(feature = "image")]
    #[rstest]
    #[case("pydicom/color3d_jpeg_baseline.dcm", 0)]
    #[case("pydicom/color3d_jpeg_baseline.dcm", 1)]
    #[case("pydicom/color3d_jpeg_baseline.dcm", 78)]
    #[case("pydicom/color3d_jpeg_baseline.dcm", 119)]
    #[case("pydicom/SC_rgb_rle_2frame.dcm", 0)]
    #[case("pydicom/SC_rgb_rle_2frame.dcm", 1)]
    #[case("pydicom/JPEG2000.dcm", 0)]
    #[case("pydicom/JPEG2000_UNC.dcm", 0)]
    fn test_parse_dicom_pixel_data_individual_frames(#[case] value: &str, #[case] frame: u32) {
        let test_file = dicom_test_files::path(value).unwrap();
        println!("Parsing pixel data for {}", test_file.display());
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data_frame(frame).unwrap();
        let output_dir = Path::new(
            "../target/dicom_test_files/_out/test_gdcm_parse_dicom_pixel_data_individual_frames",
        );
        std::fs::create_dir_all(output_dir).unwrap();

        assert_eq!(pixel_data.number_of_frames(), 1);

        let image = pixel_data.to_dynamic_image(0).unwrap();
        let image_path = output_dir.join(format!(
            "{}-{}.png",
            Path::new(value).file_stem().unwrap().to_str().unwrap(),
            frame,
        ));
        image.save(image_path).unwrap();
    }

    #[cfg(feature = "ndarray")]
    #[test]
    fn test_to_ndarray_signed_word_no_lut() {
        use crate::{ConvertOptions, ModalityLutOption};

        let test_file = dicom_test_files::path("pydicom/JPEG2000.dcm").unwrap();
        let obj = open_file(test_file).unwrap();
        let options = ConvertOptions::new().with_modality_lut(ModalityLutOption::None);
        let ndarray = obj
            .decode_pixel_data()
            .unwrap()
            .to_ndarray_with_options::<i16>(&options)
            .unwrap();
        assert_eq!(ndarray.shape(), &[1, 1024, 256, 1]);
        assert_eq!(ndarray.len(), 262144);
        assert_eq!(ndarray[[0, 260, 0, 0]], -3);
    }
}
