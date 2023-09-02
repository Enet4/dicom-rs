//! DICOM Transcoder API
//! 
//! This module collects the pixel data decoding and encoding capabilities
//! of `dicom_encoding` and `dicom_pixeldata`
//! to offer a convenient API for converting DICOM objects
//! to different transfer syntaxes.
//! 
//! See the [`Transcode`] trait for more information.
use dicom_core::{
    value::PixelFragmentSequence, DataDictionary, DataElement,
    Length, PrimitiveValue, VR, ops::ApplyOp,
};
use dicom_dictionary_std::tags;
use dicom_encoding::{adapters::EncodeOptions, Codec, TransferSyntax, TransferSyntaxIndex};
use dicom_object::{FileDicomObject, InMemDicomObject};
use dicom_transfer_syntax_registry::{entries::EXPLICIT_VR_LITTLE_ENDIAN, TransferSyntaxRegistry};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::PixelDecoder;

/// An error occurred during the object transcoding process.
#[derive(Debug, Snafu)]
pub struct Error(InnerError);

#[derive(Debug, Snafu)]
pub(crate) enum InnerError {
    /// Unrecognized transfer syntax of receiving object ({ts})
    UnknownSrcTransferSyntax { ts: String },

    /// Unsupported target transfer syntax
    UnsupportedTransferSyntax,

    /// Unsupported transcoding capability
    UnsupportedTranscoding,

    /// Could not decode pixel data of receiving object  
    DecodePixelData { source: crate::Error },

    /// Could not read receiving object
    ReadObject { source: dicom_object::ReadError },

    /// Could not encode pixel data to target encoding
    EncodePixelData {
        source: dicom_encoding::adapters::EncodeError,
    },

    /// Unsupported bits per sample ({bits_allocated})
    UnsupportedBitsAllocated { bits_allocated: u16 },

    /// Encoding multi-frame objects is not implemented
    MultiFrameEncodingNotImplemented,
}

/// Alias for the result of transcoding a DICOM object.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Interface for transcoding a DICOM object's pixel data
/// to comply with a different transfer syntax.
/// Can be implemented by in-memory DICOM object representations
/// as well as partial or lazy DICOM object readers,
/// so that transcoding can be performed without loading the entire object.
/// 
/// # Example
/// 
/// A typical [file DICOM object in memory](FileDicomObject)
/// can be transcoded inline using [`transcode`](Transcode::transcode).
/// 
/// ```no_run
/// # use dicom_object::open_file;
/// use dicom_pixeldata::Transcode as _;
/// 
/// let mut obj = dicom_object::open_file("image.dcm").unwrap();
/// // convert to JPEG
/// obj.transcode(&dicom_transfer_syntax_registry::entries::JPEG_BASELINE.erased())?;
/// 
/// // save transcoded version to file
/// obj.write_to_file("image_jpg.dcm")?;
/// 
/// Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait Transcode {
    /// Convert the receiving object's transfer syntax
    /// to the one specified in `ts` according to the given encoding options.
    /// 
    /// This method may replace one or more attributes accordingly,
    /// including the meta group specifying the transfer syntax.
    /// The encoding options only apply if the pixel data needs to be re-encoded.
    ///
    /// If the receiving object's pixel data is encapsulated,
    /// the object might be first decoded into native pixel data.
    /// In case of an encoding error,
    /// the object may be left in an intermediate state,
    /// which should not be assumed to be consistent.
    fn transcode_with_options(&mut self, ts: &TransferSyntax, options: EncodeOptions) -> Result<()>;

    /// Convert the receiving object's transfer syntax
    /// to the one specified in `ts`.
    /// 
    /// This method may replace one or more attributes accordingly,
    /// including the meta group specifying the transfer syntax.
    ///
    /// If the receiving object's pixel data is encapsulated,
    /// the object might be first decoded into native pixel data.
    /// In case of an encoding error,
    /// the object may be left in an intermediate state,
    /// which should not be assumed to be consistent.
    fn transcode(&mut self, ts: &TransferSyntax) -> Result<()> {
        self.transcode_with_options(ts, EncodeOptions::default())
    }
}

impl<D> Transcode for FileDicomObject<InMemDicomObject<D>>
where
    D: Clone + DataDictionary,
{
    fn transcode_with_options(&mut self, ts: &TransferSyntax, options: EncodeOptions) -> Result<()> {
        let current_ts_uid = &self.meta().transfer_syntax;
        // do nothing if the transfer syntax already matches
        if current_ts_uid == ts.uid() {
            return Ok(());
        }

        // inspect current object TS
        let current_ts = TransferSyntaxRegistry
            .get(current_ts_uid)
            .with_context(|| UnknownSrcTransferSyntaxSnafu {
                ts: current_ts_uid.to_string(),
            })?;

        match (current_ts.is_codec_free(), ts.is_codec_free()) {
            (true, true) => {
                // no pixel data conversion is necessary:
                // change transfer syntax and return
                self.meta_mut().set_transfer_syntax(ts);
                Ok(())
            },
            (false, true) => {
                // decode pixel data
                let decoded_pixeldata = self.decode_pixel_data().context(DecodePixelDataSnafu)?;

                // apply change to pixel data attribute
                match decoded_pixeldata.bits_allocated {
                    8 => {
                        // 8-bit samples
                        let pixels = decoded_pixeldata.data().to_vec();
                        self.put(DataElement::new_with_len(
                            tags::PIXEL_DATA,
                            VR::OW,
                            Length::defined(pixels.len() as u32),
                            PrimitiveValue::from(pixels),
                        ));
                    }
                    16 => {
                        // 16-bit samples
                        let pixels = decoded_pixeldata.data_ow();
                        self.put(DataElement::new_with_len(
                            tags::PIXEL_DATA,
                            VR::OW,
                            Length::defined(pixels.len() as u32 * 2),
                            PrimitiveValue::U16(pixels.into()),
                        ));
                    }
                    _ => {
                        return UnsupportedBitsAllocatedSnafu {
                            bits_allocated: decoded_pixeldata.bits_allocated,
                        }
                        .fail()?
                    }
                }

                // update transfer syntax
                self.meta_mut()
                    .set_transfer_syntax(&ts);

                Ok(())
            },
            (_, false) => {
                // must decode then encode
                let writer = match ts.codec() {
                    Codec::EncapsulatedPixelData(_, Some(writer)) => writer,
                    Codec::EncapsulatedPixelData(..) => {
                        return UnsupportedTransferSyntaxSnafu.fail()?
                    }
                    Codec::Dataset(None) => return UnsupportedTransferSyntaxSnafu.fail()?,
                    Codec::Dataset(Some(_)) => return UnsupportedTranscodingSnafu.fail()?,
                    Codec::None => {
                        // already tested in `is_codec_free`
                        unreachable!("Unexpected codec from transfer syntax")
                    }
                };

                // decode pixel data
                let decoded_pixeldata = self.decode_pixel_data().context(DecodePixelDataSnafu)?;
                let bits_allocated = decoded_pixeldata.bits_allocated();

                // apply change to pixel data attribute
                match bits_allocated {
                    8 => {
                        // 8-bit samples
                        let pixels = decoded_pixeldata.data().to_vec();
                        self.put(DataElement::new_with_len(
                            tags::PIXEL_DATA,
                            VR::OW,
                            Length::defined(pixels.len() as u32),
                            PrimitiveValue::from(pixels),
                        ));
                    }
                    16 => {
                        // 16-bit samples
                        let pixels = decoded_pixeldata.data_ow();
                        self.put(DataElement::new_with_len(
                            tags::PIXEL_DATA,
                            VR::OW,
                            Length::defined(pixels.len() as u32 * 2),
                            PrimitiveValue::U16(pixels.into()),
                        ));
                    }
                    _ => return UnsupportedBitsAllocatedSnafu { bits_allocated }.fail()?,
                };

                // change transfer syntax to Explicit VR little endian
                self.meta_mut()
                    .set_transfer_syntax(&EXPLICIT_VR_LITTLE_ENDIAN.erased());

                // use RWPixel adapter API
                let mut offset_table = Vec::new();
                let mut fragments = Vec::new();

                let ops = writer
                    .encode(&*self, options, &mut fragments, &mut offset_table)
                    .context(EncodePixelDataSnafu)?;

                let num_frames = offset_table.len();
                let total_pixeldata_len: u64 = fragments.iter().map(|f| f.len() as u64).sum();

                self.put(DataElement::new_with_len(
                    tags::PIXEL_DATA,
                    VR::OB,
                    Length::UNDEFINED,
                    PixelFragmentSequence::new(offset_table, fragments),
                ));

                self.put(DataElement::new(
                    tags::NUMBER_OF_FRAMES,
                    VR::IS,
                    num_frames.to_string(),
                ));

                // provide Encapsulated Pixel Data Value Total Length
                self.put(DataElement::new(
                    tags::ENCAPSULATED_PIXEL_DATA_VALUE_TOTAL_LENGTH,
                    VR::UV,
                    PrimitiveValue::from(total_pixeldata_len),
                ));

                // try to apply operations
                for (n, op) in ops.into_iter().enumerate() {
                    match self.apply(op) {
                        Ok(_) => (),
                        Err(e) => {
                            tracing::warn!("Could not apply transcoding step #{}: {}", n, e)
                        }
                    }
                }

                // change transfer syntax
                self.meta_mut().set_transfer_syntax(ts);

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_dictionary_std::uids;
    use dicom_object::open_file;
    use dicom_test_files;
    use dicom_transfer_syntax_registry::entries::{JPEG_BASELINE, JPEG_EXTENDED};

    #[test]
    fn test_transcode_from_jpeg_lossless_to_native_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_jpeg_gdcm.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data conversion is needed here
        assert_eq!(&obj.meta().transfer_syntax, uids::JPEG_LOSSLESS_SV1);

        // transcode to explicit VR little endian
        obj.transcode(&EXPLICIT_VR_LITTLE_ENDIAN.erased())
            .expect("Should have transcoded successfully");

        // check transfer syntax
        assert_eq!(obj.meta().transfer_syntax(), EXPLICIT_VR_LITTLE_ENDIAN.uid());

        // check that the pixel data is in its native form
        // and has the expected size
        let pixel_data = obj.element(tags::PIXEL_DATA).unwrap();
        let pixels = pixel_data
            .to_bytes()
            .expect("Pixel Data should be in bytes");

        let rows = 100;
        let cols = 100;
        let spp = 3;

        assert_eq!(pixels.len(), rows * cols * spp);
    }

    #[test]
    fn test_transcode_from_native_to_jpeg_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb.dcm").unwrap();
        let mut obj = open_file(&test_file).unwrap();

        // pre-condition check: pixel data is native
        assert_eq!(obj.meta().transfer_syntax(), uids::EXPLICIT_VR_LITTLE_ENDIAN);

        // transcode to JPEG baseline
        obj.transcode(&JPEG_BASELINE.erased())
            .expect("Should have transcoded successfully");

        // check transfer syntax
        assert_eq!(obj.meta().transfer_syntax(), JPEG_BASELINE.uid());

        // check that the pixel data is encapsulated
        // and has the expected number of fragments
        let pixel_data = obj.get(tags::PIXEL_DATA).unwrap();
        let fragments = pixel_data
            .fragments()
            .expect("Pixel Data should be in encapsulated fragments");

        // one frame, one fragment (as required by JPEG baseline)
        assert_eq!(fragments.len(), 1);

        // check that the fragment data is in valid JPEG (magic code)
        let fragment = &fragments[0];
        assert!(fragment.len() > 4);
        assert_eq!(&fragment[0..2], &[0xFF, 0xD8]);

        let size_1 = fragment.len();

        // re-encode with different options

        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data is native
        assert_eq!(obj.meta().transfer_syntax(), uids::EXPLICIT_VR_LITTLE_ENDIAN);

        // transcode to JPEG baseline
        let mut options = EncodeOptions::new();
        // low quality
        options.quality = Some(50);
        obj.transcode_with_options(&JPEG_BASELINE.erased(), options)
            .expect("Should have transcoded successfully");

        // check transfer syntax
        assert_eq!(obj.meta().transfer_syntax(), JPEG_BASELINE.uid());

        // check that the pixel data is encapsulated
        // and has the expected number of fragments
        let pixel_data = obj.get(tags::PIXEL_DATA).unwrap();
        let fragments = pixel_data
            .fragments()
            .expect("Pixel Data should be in encapsulated fragments");

        // one frame, one fragment (as required by JPEG baseline)
        assert_eq!(fragments.len(), 1);

        // check that the fragment data is in valid JPEG (magic code)
        let fragment = &fragments[0];
        assert!(fragment.len() > 4);
        assert_eq!(&fragment[0..2], &[0xFF, 0xD8]);

        let size_2 = fragment.len();

        // the size of the second fragment should be smaller
        // due to lower quality
        assert!(size_2 < size_1, "expected smaller size for lower quality, but {} => {}", size_2, size_1);

    }

    #[test]
    // Note: Test ignored until 12-bit JPEG decoding is supported
    #[ignore]
    fn test_transcode_from_jpeg_to_native_16bit() {
        let test_file = dicom_test_files::path("pydicom/JPEG-lossy.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data conversion is needed here
        assert_eq!(&obj.meta().transfer_syntax, uids::JPEG_EXTENDED12_BIT);

        // transcode to explicit VR little endian
        obj.transcode(&EXPLICIT_VR_LITTLE_ENDIAN.erased())
            .expect("Should have transcoded successfully");

        // check transfer syntax
        assert_eq!(&obj.meta().transfer_syntax, EXPLICIT_VR_LITTLE_ENDIAN.uid());

        // check that the pixel data is in its native form
        // and has the expected size
        let pixel_data = obj.element(tags::PIXEL_DATA).unwrap();
        let pixels = pixel_data
            .to_bytes()
            .expect("Pixel Data should be in bytes");

        let rows = 1024;
        let cols = 256;
        let spp = 3;
        let bps = 2;

        assert_eq!(pixels.len(), rows * cols * spp * bps);
    }

    /// if the transfer syntax is the same, no transcoding should be performed
    #[test]
    fn test_no_transcoding_needed() {
        {
            let test_file = dicom_test_files::path("pydicom/SC_rgb.dcm").unwrap();
            let mut obj = open_file(test_file).unwrap();

            // transcode to the same TS
            obj.transcode(&EXPLICIT_VR_LITTLE_ENDIAN.erased())
                .expect("Should have transcoded successfully");

            assert_eq!(obj.meta().transfer_syntax(), EXPLICIT_VR_LITTLE_ENDIAN.uid());
            // pixel data is still native
            let pixel_data = obj.get(tags::PIXEL_DATA).unwrap().to_bytes().unwrap();
            assert_eq!(pixel_data.len(), 100 * 100 * 3);
        }
        {
            let test_file = dicom_test_files::path("pydicom/JPEG-lossy.dcm").unwrap();
            let mut obj = open_file(test_file).unwrap();

            // transcode to the same TS
            obj.transcode(&JPEG_EXTENDED.erased())
                .expect("Should have transcoded successfully");

            assert_eq!(obj.meta().transfer_syntax(), JPEG_EXTENDED.uid());
            // pixel data is still encapsulated
            let fragments = obj.get(tags::PIXEL_DATA).unwrap().fragments().unwrap();
            assert_eq!(fragments.len(), 1);
        }
    }
}
