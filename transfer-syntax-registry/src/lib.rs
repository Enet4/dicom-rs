//! This crate contains the DICOM transfer syntax registry.
//! The transfer syntax registry maps a DICOM UID of a transfer syntax into the
//! respective transfer syntax specifier.

mod entries;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use byteordered::Endianness;
use dicom_encoding::transfer_syntax::{Codec, TransferSyntax, AdapterFreeTransferSyntax};
use lazy_static::lazy_static;

/// Data type for a registry of DICOM.
pub struct TransferSyntaxRegistry {
    m: HashMap<&'static str, TransferSyntax>,
}

impl fmt::Debug for TransferSyntaxRegistry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entries: HashMap<&str, &str> = self.m.iter()
            .map(|(uid, ts)| (*uid, ts.name()))
            .collect();
        f.debug_struct("TransferSyntaxRegistry")
            .field("m", &entries)
            .finish()
    }
}

impl TransferSyntaxRegistry {
    /// Obtain a DICOM codec by transfer syntax UID.
    pub fn get<U: AsRef<str>>(&self, uid: U) -> Option<&TransferSyntax> {
        let ts_uid = {
            let uid = uid.as_ref();
            if uid.chars().rev().next() == Some('\0') {
                &uid[..uid.len() - 1]
            } else {
                &uid
            }
        };
        self.m.get(ts_uid)
    }

    /// Register the given transfer syntax (TS) to the system. It can override
    /// another TS with the same UID, in the only case that the TS requires
    /// certain codecs which are not supported by the previously registered
    /// TS. If no such requirements are imposed, this function returns `false`
    /// and no changes are made.
    fn register(&mut self, ts: TransferSyntax) -> bool {
        match self.m.entry(&ts.uid()) {
            Entry::Occupied(mut e) => {
                let replace = match (&e.get().codec(), &ts.codec()) {
                    (Codec::Unsupported, Codec::Dataset(_)) |
                    (Codec::EncapsulatedPixelData, Codec::PixelData(_)) => {
                        true
                    },
                    // weird one ahead: the two specifiers do not agree on
                    // requirements, better keep it as a separate match arm for
                    // debugging purposes
                    (Codec::Unsupported, Codec::PixelData(_)) => {
                        eprintln!("Inconsistent requirements for transfer syntax {}: `Unsupported` cannot be replaced with `PixelData`", ts.uid());
                        false
                    },
                    _ => false,
                };

                if replace {
                    e.insert(ts);
                    true
                } else {
                    false
                }
            },
            Entry::Vacant(e) => {
                e.insert(ts);
                true
            },
        }
    }
}

lazy_static! {
    static ref REGISTRY: TransferSyntaxRegistry = {
        TransferSyntaxRegistry { m: initialize_codecs() }
    };
}

/// Retrieve the default transfer syntax.
pub fn default() -> AdapterFreeTransferSyntax {
    entries::IMPLICIT_VR_LITTLE_ENDIAN
}

/// Retrieve the global codec registry.
pub fn get_registry() -> &'static TransferSyntaxRegistry {
    &REGISTRY
}

fn initialize_codecs() -> HashMap<&'static str, TransferSyntax> {
    let mut m = HashMap::<&'static str, TransferSyntax>::new();

    use crate::entries::*;

    // the three base transfer syntaxes, fully supported
    let ts = EXPLICIT_VR_LITTLE_ENDIAN;
    m.insert(ts.uid(), ts.erased());
    let ts = IMPLICIT_VR_LITTLE_ENDIAN;
    m.insert(ts.uid(), ts.erased());
    let ts = EXPLICIT_VR_BIG_ENDIAN;
    m.insert(ts.uid(), ts.erased());

    // stub transfer syntaxes, only partially supported due
    // to pixel data encapsulation
    let ts = JPEG_BASELINE;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_EXTENDED;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_LOSSLESS_NON_HIERARCHICAL;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_LS_LOSSLESS_IMAGE_COMPRESSION;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_LS_LOSSY_IMAGE_COMPRESSION;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_2000_IMAGE_COMPRESSION;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY;
    m.insert(ts.uid(), ts.erased());
    let ts = JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION;
    m.insert(ts.uid(), ts.erased());
    let ts = JPIP_REFERENCED;
    m.insert(ts.uid(), ts.erased());
    let ts = MPEG2_MAIN_PROFILE_MAIN_LEVEL;
    m.insert(ts.uid(), ts.erased());
    let ts = MPEG2_MAIN_PROFILE_HIGH_LEVEL;
    m.insert(ts.uid(), ts.erased());
    let ts = MPEG4_AVC_H264_HIGH_PROFILE;
    m.insert(ts.uid(), ts.erased());
    let ts = MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE;
    m.insert(ts.uid(), ts.erased());
    let ts = MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO;
    m.insert(ts.uid(), ts.erased());
    let ts = MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO;
    m.insert(ts.uid(), ts.erased());
    let ts = MPEG4_AVC_H264_STEREO_HIGH_PROFILE;
    m.insert(ts.uid(), ts.erased());
    let ts = HEVC_H265_MAIN_PROFILE;
    m.insert(ts.uid(), ts.erased());
    let ts = HEVC_H265_MAIN_10_PROFILE;
    m.insert(ts.uid(), ts.erased());
    let ts = RLE_LOSSLESS;
    m.insert(ts.uid(), ts.erased());

    // stub transfer syntaxes, known but not supported
    let ts = DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN;
    m.insert(ts.uid(), ts.erased());
    let ts = JPIP_DEREFERENCED_DEFLATE;
    m.insert(ts.uid(), ts.erased());

    m
}

/// create a TS with an unsupported pixel encapsulation
pub(crate) const fn create_ts_stub(uid: &'static str, name: &'static str) -> AdapterFreeTransferSyntax {
    TransferSyntax::new(
        uid,
        name,
        Endianness::Little,
        true,
        Codec::EncapsulatedPixelData,
    )
}
