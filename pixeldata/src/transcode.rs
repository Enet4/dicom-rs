use dicom_core::{
    smallvec::smallvec, DataDictionary, DataElement, DicomValue, Length, PrimitiveValue, Tag, VR,
};
use dicom_dictionary_std::tags;
use dicom_encoding::{adapters::EncodeOptions, Codec, TransferSyntax, TransferSyntaxIndex};
use dicom_object::{mem::InMemFragment, FileDicomObject, InMemDicomObject};
use dicom_transfer_syntax_registry::{entries::EXPLICIT_VR_LITTLE_ENDIAN, TransferSyntaxRegistry};
use snafu::{ensure, OptionExt, ResultExt, Snafu};

use crate::PixelDecoder;

#[derive(Debug, Snafu)]
pub struct Error(InnerError);

/// An error occurred during the object transcoding process.
#[derive(Debug, Snafu)]
pub(crate) enum InnerError {
    /// Unrecognized transfer syntax of receiving object ({ts})
    UnknownSrcTransferSyntax { ts: String },

    /// Unsupported target transfer syntax
    UnsupportedTransferSyntax,

    /// Could not decode pixel data of receiving object  
    DecodePixelData { source: crate::Error },

    /// Could not read receiving object
    ReadObject { source: dicom_object::Error },

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
pub trait Transcode {
    /// Convert the receiving object's transfer syntax
    /// to the one specified in `ts`,
    /// replacing one or more attributes to fit the intended transfer syntax,
    /// including the meta group specifying the transfer syntax.
    ///
    /// If the receiving object's pixel data is encapsulated,
    /// the object is first decoded into native pixel data.
    /// In case of an encoding error,
    /// the object may be left with this intermediate state.
    fn transcode(&mut self, ts: &TransferSyntax) -> Result<()>;
}

impl<D> Transcode for FileDicomObject<InMemDicomObject<D>>
where
    D: Clone + DataDictionary,
{
    fn transcode(&mut self, ts: &TransferSyntax) -> Result<()> {
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

        if ts.is_codec_free() {
            if current_ts.is_codec_free() {
                // no pixel data conversion is necessary:
                // change transfer syntax and return
                self.meta_mut().set_transfer_syntax(ts);
                Ok(())
            } else {
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
                };

                // change transfer syntax and return
                self.meta_mut().set_transfer_syntax(ts);
                Ok(())
            }
        } else {
            // must decode then encode
            let adapter = match ts.codec() {
                Codec::PixelData(adapter) => adapter,
                Codec::EncapsulatedPixelData => return UnsupportedTransferSyntaxSnafu.fail()?,
                Codec::None | Codec::Unsupported | Codec::Dataset(_) => {
                    unreachable!("Unexpected codec from transfer syntax")
                }
            };

            // decode pixel data
            let decoded_pixeldata = self.decode_pixel_data().context(DecodePixelDataSnafu)?;
            let bits_allocated = decoded_pixeldata.bits_allocated();
            let number_of_frames = decoded_pixeldata.number_of_frames();

            // note: there currently not a clear way
            // to encode multiple fragments,
            // so we stop if the image has more than one frame
            ensure!(number_of_frames == 1, MultiFrameEncodingNotImplementedSnafu);

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
                .set_transfer_syntax(&EXPLICIT_VR_LITTLE_ENDIAN);

            // use RWPixel adapter API
            let mut pixeldata = Vec::new();
            let options = EncodeOptions::new();
            adapter
                .encode(&*self, options, &mut pixeldata)
                .context(EncodePixelDataSnafu)?;

            let total_pixeldata_len = pixeldata.len() as u64;

            // put everything in a single fragment
            let pixel_seq = DicomValue::<InMemDicomObject<D>, InMemFragment>::PixelSequence {
                offset_table: smallvec![],
                fragments: smallvec![pixeldata],
            };

            self.put(DataElement::new_with_len(
                tags::PIXEL_DATA,
                VR::OB,
                Length::UNDEFINED,
                pixel_seq,
            ));

            self.put(DataElement::new(
                tags::NUMBER_OF_FRAMES,
                VR::IS,
                PrimitiveValue::from("1"),
            ));

            // replace Encapsulated Pixel Data Value Total Length
            // if it is present
            if self
                .element_opt(Tag(0x7FE0, 0x0003))
                .context(ReadObjectSnafu)?
                .is_some()
            {
                self.put(DataElement::new(
                    Tag(0x7FE0, 0x0003),
                    VR::UV,
                    PrimitiveValue::from(total_pixeldata_len),
                ));
            }

            // change transfer syntax
            self.meta_mut().set_transfer_syntax(ts);

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_object::open_file;
    use dicom_test_files;

    #[test]
    fn test_transcode_from_jpeg_to_native_rgb() {
        let test_file = dicom_test_files::path("pydicom/SC_rgb_jpeg_gdcm.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data conversion is needed here
        assert_ne!(&obj.meta().transfer_syntax, EXPLICIT_VR_LITTLE_ENDIAN.uid());

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

        let rows = 100;
        let cols = 100;
        let spp = 3;

        assert_eq!(pixels.len(), rows * cols * spp);
    }

    #[test]
    // Test ignored until 12-bit JPEG decoding is supported
    #[ignore]
    fn test_transcode_from_jpeg_to_native_16bit() {
        let test_file = dicom_test_files::path("pydicom/JPEG-lossy.dcm").unwrap();
        let mut obj = open_file(test_file).unwrap();

        // pre-condition check: pixel data conversion is needed here
        assert_ne!(&obj.meta().transfer_syntax, EXPLICIT_VR_LITTLE_ENDIAN.uid());

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
}
