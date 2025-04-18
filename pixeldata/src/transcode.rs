//! DICOM Transcoder API
//!
//! This module collects the pixel data decoding and encoding capabilities
//! of `dicom_encoding` and `dicom_pixeldata`
//! to offer a convenient API for converting DICOM objects
//! to different transfer syntaxes.
//!
//! See the [`Transcode`] trait for more information.
use dicom_core::{
    ops::ApplyOp, value::PixelFragmentSequence, DataDictionary, DataElement, Length,
    PrimitiveValue, VR,
};
use dicom_dictionary_std::{tags, uids};
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

    /// Could not encode pixel data to target encoding
    EncodePixelData {
        source: dicom_encoding::adapters::EncodeError,
    },

    /// Unsupported bits per sample ({bits_allocated})
    UnsupportedBitsAllocated { bits_allocated: u16 },
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
/// # Ok::<(), Box<dyn std::error::Error>>(())
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
    fn transcode_with_options(&mut self, ts: &TransferSyntax, options: EncodeOptions)
        -> Result<()>;

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
    fn transcode_with_options(
        &mut self,
        ts: &TransferSyntax,
        options: EncodeOptions,
    ) -> Result<()> {
        let current_ts_uid = self.meta().transfer_syntax();
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

        match (current_ts.is_encapsulated_pixel_data(), ts.is_encapsulated_pixel_data()) {
            (false, false) => {
                // no pixel data conversion is necessary:
                // change transfer syntax and return
                self.meta_mut().set_transfer_syntax(ts);
                Ok(())
            }
            (true, false) => {
                // decode pixel data
                decode_inline(self, ts)?;
                Ok(())
            }
            // make some exceptions for transfer syntaxes
            // which are best transcoded from encapsulated pixel data:
            // - JPEG baseline -> JPEG XL * (can recompress JPEG)
            // - JPEG XL recompression -> JPEG baseline (can do lossless conversion)
            (true, true)
                if (current_ts.uid() == uids::JPEG_BASELINE8_BIT
                    && (ts.uid() == uids::JPEGXLJPEG_RECOMPRESSION
                        || ts.uid() == uids::JPEGXL
                        || ts.uid() == uids::JPEGXL_LOSSLESS))
                    || (current_ts.uid() == uids::JPEGXLJPEG_RECOMPRESSION
                        && ts.uid() == uids::JPEG_BASELINE8_BIT) =>
            {
                // start by assuming that the codec can work with it as is
                let writer = match ts.codec() {
                    Codec::EncapsulatedPixelData(_, Some(writer)) => writer,
                    Codec::EncapsulatedPixelData(..) => {
                        return UnsupportedTransferSyntaxSnafu.fail()?
                    }
                    Codec::Dataset(None) => return UnsupportedTransferSyntaxSnafu.fail()?,
                    Codec::Dataset(Some(_)) => return UnsupportedTranscodingSnafu.fail()?,
                    Codec::None => {
                        // already tested in `is_encapsulated_pixel_data`
                        unreachable!("Unexpected codec from transfer syntax")
                    }
                };

                let mut offset_table = Vec::new();
                let mut fragments = Vec::new();

                match writer.encode(&*self, options.clone(), &mut fragments, &mut offset_table) {
                    Ok(ops) => {
                        // success!
                        let num_frames = offset_table.len();
                        let total_pixeldata_len: u64 =
                            fragments.iter().map(|f| f.len() as u64).sum();

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
                        self.update_meta(|meta| meta.set_transfer_syntax(ts));

                        Ok(())
                    }
                    Err(dicom_encoding::adapters::EncodeError::NotNative) => {
                        // not supported after all, fall back
                        return decode_and_encode(self, ts, options);
                    }
                    Err(e) => Err(e),
                }
                .context(EncodePixelDataSnafu)?;
                Ok(())
            }
            (_, true) => {
                // must decode then encode
                decode_and_encode(self, ts, options)
            }
        }
    }
}

/// decode and override pixel data to native form
/// (`ts` must be a native pixel data transfer syntax)
fn decode_inline<D, T, U, V>(
    obj: &mut FileDicomObject<InMemDicomObject<D>>,
    ts: &TransferSyntax<T, U, V>,
) -> Result<()>
where
    D: Clone + DataDictionary,
{
    // decode pixel data
    let decoded_pixeldata = obj.decode_pixel_data().context(DecodePixelDataSnafu)?;
    let bits_allocated = decoded_pixeldata.bits_allocated();

    // apply change to pixel data attribute
    match bits_allocated {
        8 => {
            // 8-bit samples
            let pixels = decoded_pixeldata.data().to_vec();
            obj.put(DataElement::new_with_len(
                tags::PIXEL_DATA,
                VR::OW,
                Length::defined(pixels.len() as u32),
                PrimitiveValue::from(pixels),
            ));
        }
        16 => {
            // 16-bit samples
            let pixels = decoded_pixeldata.data_ow();
            obj.put(DataElement::new_with_len(
                tags::PIXEL_DATA,
                VR::OW,
                Length::defined(pixels.len() as u32 * 2),
                PrimitiveValue::U16(pixels.into()),
            ));
        }
        _ => return UnsupportedBitsAllocatedSnafu { bits_allocated }.fail()?,
    };

    // change transfer syntax to Explicit VR little endian
    obj.update_meta(|meta| meta.set_transfer_syntax(ts));

    Ok(())
}

/// the impl of transcoding which decodes encapsulated pixel data to native
/// and then encodes it to the target transfer syntax
fn decode_and_encode<D>(
    obj: &mut FileDicomObject<InMemDicomObject<D>>,
    ts: &TransferSyntax,
    options: EncodeOptions,
) -> Result<()>
where
    D: Clone + DataDictionary,
{
    let writer = match ts.codec() {
        Codec::EncapsulatedPixelData(_, Some(writer)) => writer,
        Codec::EncapsulatedPixelData(..) => return UnsupportedTransferSyntaxSnafu.fail()?,
        Codec::Dataset(None) => return UnsupportedTransferSyntaxSnafu.fail()?,
        Codec::Dataset(Some(_)) => return UnsupportedTranscodingSnafu.fail()?,
        Codec::None => {
            // already tested in `is_codec_free`
            unreachable!("Unexpected codec from transfer syntax")
        }
    };

    // decode pixel data
    decode_inline(obj, &EXPLICIT_VR_LITTLE_ENDIAN)?;

    // use pixel data writer API
    let mut offset_table = Vec::new();
    let mut fragments = Vec::new();

    let ops = writer
        .encode(&*obj, options, &mut fragments, &mut offset_table)
        .context(EncodePixelDataSnafu)?;

    let num_frames = offset_table.len();
    let total_pixeldata_len: u64 = fragments.iter().map(|f| f.len() as u64).sum();

    obj.put(DataElement::new_with_len(
        tags::PIXEL_DATA,
        VR::OB,
        Length::UNDEFINED,
        PixelFragmentSequence::new(offset_table, fragments),
    ));

    obj.put(DataElement::new(
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        num_frames.to_string(),
    ));

    // provide Encapsulated Pixel Data Value Total Length
    obj.put(DataElement::new(
        tags::ENCAPSULATED_PIXEL_DATA_VALUE_TOTAL_LENGTH,
        VR::UV,
        PrimitiveValue::from(total_pixeldata_len),
    ));

    // try to apply operations
    for (n, op) in ops.into_iter().enumerate() {
        match obj.apply(op) {
            Ok(_) => (),
            Err(e) => {
                tracing::warn!("Could not apply transcoding step #{}: {}", n, e)
            }
        }
    }

    // change transfer syntax
    obj.update_meta(|meta| meta.set_transfer_syntax(ts));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_dictionary_std::uids;
    use dicom_object::open_file;
    #[cfg(feature = "native")]
    use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;
    use dicom_transfer_syntax_registry::entries::{
        ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN, JPEG_EXTENDED,
    };

    #[cfg(feature = "native")]
    #[test]
    fn test_transcode_from_jpeg_lossless_to_native_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_jpeg_gdcm.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data conversion is needed here
        assert_eq!(obj.meta().transfer_syntax(), uids::JPEG_LOSSLESS_SV1);

        // transcode to explicit VR little endian
        obj.transcode(&EXPLICIT_VR_LITTLE_ENDIAN.erased())
            .expect("Should have transcoded successfully");

        // check transfer syntax
        assert_eq!(
            obj.meta().transfer_syntax(),
            EXPLICIT_VR_LITTLE_ENDIAN.uid()
        );

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

    #[cfg(feature = "native")]
    #[test]
    fn test_transcode_from_native_to_jpeg_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb.dcm").unwrap();
        let mut obj = open_file(&test_file).unwrap();

        // pre-condition check: pixel data is native
        assert_eq!(
            obj.meta().transfer_syntax(),
            uids::EXPLICIT_VR_LITTLE_ENDIAN
        );

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
        assert_eq!(
            obj.meta().transfer_syntax(),
            uids::EXPLICIT_VR_LITTLE_ENDIAN
        );

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
        assert!(
            size_2 < size_1,
            "expected smaller size for lower quality, but {} => {}",
            size_2,
            size_1
        );
    }

    #[cfg(feature = "native")]
    #[test]
    // Note: Test ignored until 12-bit JPEG decoding is supported
    #[ignore]
    fn test_transcode_from_jpeg_to_native_16bit() {
        let test_file = dicom_test_files::path("pydicom/JPEG-lossy.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data conversion is needed here
        assert_eq!(obj.meta().transfer_syntax(), uids::JPEG_EXTENDED12_BIT);

        // transcode to explicit VR little endian
        obj.transcode(&EXPLICIT_VR_LITTLE_ENDIAN.erased())
            .expect("Should have transcoded successfully");

        // check transfer syntax
        assert_eq!(
            obj.meta().transfer_syntax(),
            EXPLICIT_VR_LITTLE_ENDIAN.uid()
        );

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

    /// can transcode native multi-frame pixel data
    #[cfg(feature = "native")]
    #[test]
    fn test_transcode_2frames_to_jpeg() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_2frame.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data conversion is needed here
        assert_eq!(
            obj.meta().transfer_syntax(),
            uids::EXPLICIT_VR_LITTLE_ENDIAN
        );

        // transcode to JPEG baseline
        obj.transcode(&JPEG_BASELINE.erased())
            .expect("Should have transcoded successfully");

        // check transfer syntax
        assert_eq!(obj.meta().transfer_syntax(), JPEG_BASELINE.uid());

        // check that the number of frames stayed the same
        let num_frames = obj.get(tags::NUMBER_OF_FRAMES).unwrap();
        assert_eq!(num_frames.to_int::<u32>().unwrap(), 2);

        // check that the pixel data is encapsulated
        let pixel_data = obj.element(tags::PIXEL_DATA).unwrap();

        let fragments = pixel_data
            .fragments()
            .expect("Pixel Data should be in encapsulated fragments");

        // two frames, two fragments (as required by JPEG baseline)
        assert_eq!(fragments.len(), 2);

        // each frame has some data
        assert!(fragments[0].len() > 4);
        assert!(fragments[1].len() > 4);
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

            assert_eq!(
                obj.meta().transfer_syntax(),
                EXPLICIT_VR_LITTLE_ENDIAN.uid()
            );
            // pixel data is still native
            let pixel_data = obj.get(tags::PIXEL_DATA).unwrap().to_bytes().unwrap();
            assert_eq!(pixel_data.len(), 100 * 100 * 3);
        }
        {
            let test_file = dicom_test_files::path("pydicom/JPEG-lossy.dcm").unwrap();
            let mut obj = open_file(test_file).unwrap();

            assert_eq!(obj.meta().transfer_syntax(), uids::JPEG_EXTENDED12_BIT);

            // transcode to the same TS
            obj.transcode(&JPEG_EXTENDED.erased())
                .expect("Should have transcoded successfully");

            assert_eq!(obj.meta().transfer_syntax(), uids::JPEG_EXTENDED12_BIT);
            // pixel data is still encapsulated
            let fragments = obj.get(tags::PIXEL_DATA).unwrap().fragments().unwrap();
            assert_eq!(fragments.len(), 1);
        }
    }

    /// converting to Encapsulated Uncompressed Explicit VR Little Endian
    /// should split each frame into separate fragments in native form
    #[test]
    fn test_transcode_encapsulated_uncompressed() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_2frame.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // transcode to the same TS
        obj.transcode(&ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN.erased())
            .expect("Should have transcoded successfully");

        assert_eq!(
            obj.meta().transfer_syntax(),
            ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN.uid()
        );
        // pixel data is encapsulated, but in native form
        let pixel_data = obj.get(tags::PIXEL_DATA).unwrap();
        let fragments = pixel_data.fragments().unwrap();
        assert_eq!(fragments.len(), 2);
        // each frame should have native pixel data (100x100 RGB)
        assert_eq!(fragments[0].len(), 100 * 100 * 3);
        assert_eq!(fragments[1].len(), 100 * 100 * 3);
    }
}
