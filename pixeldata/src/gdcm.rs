//! Decode pixel data using GDCM when the default features are enabled.

use crate::*;
use dicom_encoding::adapters::DecodeError;
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use gdcm_rs::{decode_single_frame_compressed, GDCMPhotometricInterpretation, GDCMTransferSyntax};
use std::str::FromStr;

impl<D> PixelDecoder for FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary + Clone,
{
    fn decode_pixel_data(&self) -> Result<DecodedPixelData> {
        let pixel_data = self
            .element(dicom_dictionary_std::tags::PIXEL_DATA)
            .context(MissingRequiredFieldSnafu)?;
        let cols = cols(self)?;
        let rows = rows(self)?;

        let photometric_interpretation = photometric_interpretation(self)?;
        let pi_type = GDCMPhotometricInterpretation::from_str(&photometric_interpretation)
            .map_err(|_| {
                UnsupportedPhotometricInterpretationSnafu {
                    pi: photometric_interpretation.clone(),
                }
                .build()
            })?;

        let transfer_syntax = &self.meta().transfer_syntax;
        let registry =
            TransferSyntaxRegistry
                .get(&&transfer_syntax)
                .context(UnknownTransferSyntaxSnafu {
                    ts_uid: transfer_syntax,
                })?;
        let ts_type = GDCMTransferSyntax::from_str(&registry.uid()).map_err(|_| {
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
        let number_of_frames = number_of_frames(self);

        let decoded_pixel_data = match pixel_data.value() {
            Value::PixelSequence {
                fragments,
                offset_table: _,
            } => {
                if fragments.len() > 1 {
                    // Bundle fragments and decode multi-frame dicoms
                    UnsupportedMultiFrameSnafu.fail()?
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
                .map_err(|source| Error::DecodePixelData {
                    source: DecodeError::Custom {
                        message: "Could not decode frame via GDCM".to_string(),
                        source: Some(Box::new(source) as Box<_>),
                    },
                })?;
                decoded_frame.to_vec()
            }
            Value::Primitive(p) => {
                // Non-encoded, just return the pixel data of the first frame
                let total_bytes = rows as usize
                    * cols as usize
                    * samples_per_pixel as usize
                    * (bits_allocated as usize / 8);
                p.to_bytes()[0..total_bytes].to_vec()
            }
            Value::Sequence { items: _, size: _ } => InvalidPixelDataSnafu.fail()?,
        };

        // pixels are already interpreted,
        // set new photometric interpretation
        let new_pi = match samples_per_pixel {
            1 => "MONOCHROME2".to_owned(),
            3 => "RGB".to_owned(),
            _ => photometric_interpretation,
        };

        Ok(DecodedPixelData {
            data: Cow::from(decoded_pixel_data),
            cols: cols.into(),
            rows: rows.into(),
            number_of_frames,
            photometric_interpretation: new_pi,
            samples_per_pixel,
            planar_configuration: 0,
            bits_allocated,
            bits_stored,
            high_bit,
            pixel_representation,
            rescale_intercept,
            rescale_slope,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_object::open_file;
    use dicom_test_files;
    use rstest::rstest;
    use std::fs;
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
        "pydicom/SC_rgb.dcm",
        "pydicom/SC_rgb_16bit.dcm",
        "pydicom/SC_rgb_dcmtk_+eb+cr.dcm",
        "pydicom/SC_rgb_expb.dcm",
        "pydicom/SC_rgb_expb_16bit.dcm",
        "pydicom/SC_rgb_gdcm2k_uncompressed.dcm",
        "pydicom/SC_rgb_gdcm_KY.dcm",
        "pydicom/SC_rgb_jpeg_gdcm.dcm",
        "pydicom/SC_rgb_jpeg_lossy_gdcm.dcm",
        "pydicom/SC_rgb_rle.dcm",
        "pydicom/SC_rgb_rle_16bit.dcm",
        "pydicom/color-pl.dcm",
        "pydicom/color-px.dcm",
        "pydicom/SC_ybr_full_uncompressed.dcm",
])]
    fn test_parse_dicom_pixel_data(value: &str) {
        let test_file = dicom_test_files::path(value).unwrap();
        println!("Parsing pixel data for {}", test_file.display());
        let obj = open_file(test_file).unwrap();
        let pixel_data = obj.decode_pixel_data().unwrap();
        let output_dir =
            Path::new("../target/dicom_test_files/_out/test_gdcm_parse_dicom_pixel_data");
        fs::create_dir_all(output_dir).unwrap();

        for i in 0..pixel_data.number_of_frames {
            let image = pixel_data.to_dynamic_image(i).unwrap();
            let image_path = output_dir.join(format!(
                "{}-{}.png",
                Path::new(value).file_stem().unwrap().to_str().unwrap(),
                i,
            ));
            image.save(image_path).unwrap();
        }
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
        assert_eq!(ndarray.shape(), &[1, 1024, 256, 1]);
        assert_eq!(ndarray.len(), 262144);
        assert_eq!(ndarray[[0, 260, 0, 0]], -3);
    }
}
