//! Decode pixel data using GDCM when the default features are enabled.

use crate::*;
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
            .context(MissingRequiredField)?;
        let cols = cols(self)?;
        let rows = rows(self)?;

        let photometric_interpretation = photometric_interpretation(self)?;
        let pi_type = GDCMPhotometricInterpretation::from_str(&photometric_interpretation)
            .map_err(|_| Error::UnsupportedPhotometricInterpretation {
                pi: photometric_interpretation.clone(),
            })?;

        let transfer_syntax = &self.meta().transfer_syntax;
        let registry =
            TransferSyntaxRegistry
                .get(&&transfer_syntax)
                .context(UnsupportedTransferSyntax {
                    ts: transfer_syntax,
                })?;
        let ts_type = GDCMTransferSyntax::from_str(&registry.uid()).map_err(|_| {
            Error::UnsupportedTransferSyntax {
                ts: transfer_syntax.clone(),
            }
        })?;

        let samples_per_pixel = samples_per_pixel(self)?;
        let bits_allocated = bits_allocated(self)?;
        let bits_stored = bits_stored(self)?;
        let high_bit = high_bit(self)?;
        let pixel_representation = pixel_representation(self)?;
        let rescale_intercept = rescale_intercept(self);
        let rescale_slope = rescale_slope(self);

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
                .map_err(|source| Error::ImplementerError {
                    source: Box::new(source),
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

    // "pydicom/RG1_J2KI.dcm",
    // "pydicom/RG1_J2KR.dcm",
    // "pydicom/RG1_UNCI.dcm",
    // "pydicom/RG1_UNCR.dcm",
    // "pydicom/RG3_J2KI.dcm",
    // "pydicom/RG3_J2KR.dcm",
    // "pydicom/RG3_UNCI.dcm",
    // "pydicom/RG3_UNCR.dcm",
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
    // "pydicom/SC_rgb_16bit_2frame.dcm",
    // "pydicom/SC_rgb_2frame.dcm",
    // "pydicom/SC_rgb_32bit.dcm",
    // "pydicom/SC_rgb_32bit_2frame.dcm",
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
    // "pydicom/SC_rgb_expb_16bit_2frame.dcm",
    // "pydicom/SC_rgb_expb_2frame.dcm",
    // "pydicom/SC_rgb_expb_32bit.dcm",
    // "pydicom/SC_rgb_expb_32bit_2frame.dcm",
    // "pydicom/SC_rgb_rle_16bit_2frame.dcm",
    // "pydicom/SC_rgb_rle_2frame.dcm",
    // "pydicom/SC_rgb_rle_32bit.dcm",
    // "pydicom/SC_rgb_rle_32bit_2frame.dcm",
    // "pydicom/SC_rgb_small_odd.dcm",
    // "pydicom/SC_rgb_small_odd_jpeg.dcm",
    // "pydicom/SC_rgb_jpeg_dcmtk.dcm",
    // "pydicom/SC_ybr_full_422_uncompressed.dcm",
    // "pydicom/US1_J2KI.dcm",
    // "pydicom/US1_J2KR.dcm",
    // "pydicom/US1_UNCI.dcm",
    // "pydicom/US1_UNCR.dcm",
    // "pydicom/badVR.dcm",
    // "pydicom/bad_sequence.dcm",
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
        println!("Parsing pixel data for {}", test_file.display());
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
}
