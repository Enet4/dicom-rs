#![deny(trivial_numeric_casts, unsafe_code, unstable_features)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    unused_qualifications,
    unused_import_braces
)]
//! This crate contains the DICOM transfer syntax registry.
//! The transfer syntax registry maps a DICOM UID of a transfer syntax into the
//! respective transfer syntax specifier. In the default implementation, the
//! container of transfer syntaxes is populated before-main through the
//! [inventory] pattern, then making all registerd TSes readily available
//! through the [`TransferSyntaxRegistry`] type.
//!
//! The default Cargo feature `inventory-registry` can be deactivated for
//! environments which do not support `inventory`, with the downside of only
//! providing the built-in transfer syntaxes.
//!
//! This registry should not have to be used directly, except when developing
//! higher level APIs, which should learn to negotiate and resolve the expected
//! transfer syntax automatically.
//!
//! ## Transfer Syntax descriptors
//!
//! This crate encompasses the basic DICOM level of conformance:
//! _Implicit VR Little Endian_,
//! _Explicit VR Little Endian_,
//! and _Explicit VR Big Endian_ are built-in.
//! Transfer syntaxes which are not supported,
//! or which rely on encapsulated pixel data,
//! are only listed as _stubs_ to be replaced by separate libraries.
//! The full list is available in the [`entries`](entries) module.
//!
//! [inventory]: https://docs.rs/inventory/0.1.4/inventory

use byteordered::Endianness;
use dicom_encoding::transfer_syntax::{
    AdapterFreeTransferSyntax as Ts, Codec, TransferSyntaxIndex,
};
use lazy_static::lazy_static;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

pub use dicom_encoding::TransferSyntax;
pub mod entries;

/// Data type for a registry of DICOM.
pub struct TransferSyntaxRegistryImpl {
    m: HashMap<&'static str, &'static TransferSyntax>,
}

impl fmt::Debug for TransferSyntaxRegistryImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entries: HashMap<&str, &str> =
            self.m.iter().map(|(uid, ts)| (*uid, ts.name())).collect();
        f.debug_struct("TransferSyntaxRegistryImpl")
            .field("m", &entries)
            .finish()
    }
}

impl TransferSyntaxRegistryImpl {
    /// Obtain a DICOM codec by transfer syntax UID.
    fn get<U: AsRef<str>>(&self, uid: U) -> Option<&'static TransferSyntax> {
        let ts_uid = uid
            .as_ref()
            .trim_end_matches(|c: char| c.is_whitespace() || c == '\0');
        self.m.get(ts_uid).copied()
    }

    /// Register the given transfer syntax (TS) to the system. It can override
    /// another TS with the same UID, in the only case that the TS requires
    /// certain codecs which are not supported by the previously registered
    /// TS. If no such requirements are imposed, this function returns `false`
    /// and no changes are made.
    fn register(&mut self, ts: &'static TransferSyntax) -> bool {
        match self.m.entry(&ts.uid()) {
            Entry::Occupied(mut e) => {
                let replace = match (&e.get().codec(), ts.codec()) {
                    (Codec::Unsupported, Codec::Dataset(_))
                    | (Codec::EncapsulatedPixelData, Codec::PixelData(_)) => true,
                    // weird one ahead: the two specifiers do not agree on
                    // requirements, better keep it as a separate match arm for
                    // debugging purposes
                    (Codec::Unsupported, Codec::PixelData(_)) => {
                        eprintln!("Inconsistent requirements for transfer syntax {}: `Unsupported` cannot be replaced with `PixelData`", ts.uid());
                        false
                    }
                    // ignoring TS with less or equal implementation
                    _ => false,
                };

                if replace {
                    e.insert(ts);
                    true
                } else {
                    false
                }
            }
            Entry::Vacant(e) => {
                e.insert(ts);
                true
            }
        }
    }
}

impl TransferSyntaxIndex for TransferSyntaxRegistryImpl {
    #[inline]
    fn get(&self, uid: &str) -> Option<&TransferSyntax> {
        Self::get(self, uid)
    }
}

/// Zero-sized representative of the main transfer syntax registry.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub struct TransferSyntaxRegistry;

impl TransferSyntaxIndex for TransferSyntaxRegistry {
    #[inline]
    fn get(&self, uid: &str) -> Option<&TransferSyntax> {
        get_registry().get(uid)
    }
}

lazy_static! {
    static ref BUILT_IN_TS: [TransferSyntax; 29] = {
        use self::entries::*;
        [
            IMPLICIT_VR_LITTLE_ENDIAN.erased(),
            EXPLICIT_VR_LITTLE_ENDIAN.erased(),
            EXPLICIT_VR_BIG_ENDIAN.erased(),

            DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN.erased(),
            JPIP_REFERENCED_DEFLATE.erased(),
            JPEG_BASELINE.erased(),
            JPEG_EXTENDED.erased(),
            JPEG_LOSSLESS_NON_HIERARCHICAL.erased(),
            JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.erased(),
            JPEG_LS_LOSSLESS_IMAGE_COMPRESSION.erased(),
            JPEG_LS_LOSSY_IMAGE_COMPRESSION.erased(),
            JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.erased(),
            JPEG_2000_IMAGE_COMPRESSION.erased(),
            JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY.erased(),
            JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION.erased(),
            JPIP_REFERENCED.erased(),
            MPEG2_MAIN_PROFILE_MAIN_LEVEL.erased(),
            MPEG2_MAIN_PROFILE_HIGH_LEVEL.erased(),
            MPEG4_AVC_H264_HIGH_PROFILE.erased(),
            MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE.erased(),
            MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO.erased(),
            MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO.erased(),
            MPEG4_AVC_H264_STEREO_HIGH_PROFILE.erased(),
            HEVC_H265_MAIN_PROFILE.erased(),
            HEVC_H265_MAIN_10_PROFILE.erased(),
            RLE_LOSSLESS.erased(),
            SMPTE_ST_2110_20_UNCOMPRESSED_PROGRESSIVE.erased(),
            SMPTE_ST_2110_20_UNCOMPRESSED_INTERLACED.erased(),
            SMPTE_ST_2110_30_PCM.erased(),
        ]
    };

    static ref REGISTRY: TransferSyntaxRegistryImpl = {
        let mut registry = TransferSyntaxRegistryImpl {
            m: HashMap::with_capacity(32),
        };

        // add built-in TSes manually
        for ts in BUILT_IN_TS.iter() {
            registry.register(ts);
        }
        // add TSes from inventory, if available
        inventory_populate(&mut registry);

        registry
    };
}

#[cfg(feature = "inventory-registry")]
#[inline]
fn inventory_populate(registry: &mut TransferSyntaxRegistryImpl) {
    for ts in inventory::iter::<TransferSyntax> {
        registry.register(ts);
    }
}

#[cfg(not(feature = "inventory-registry"))]
#[inline]
fn inventory_populate(_: &mut TransferSyntaxRegistryImpl) {
    // do nothing
}

/// Retrieve a reference to the global codec registry.
#[inline]
pub(crate) fn get_registry() -> &'static TransferSyntaxRegistryImpl {
    &REGISTRY
}

/// create a TS with an unsupported pixel encapsulation
pub(crate) const fn create_ts_stub(uid: &'static str, name: &'static str) -> Ts {
    TransferSyntax::new(
        uid,
        name,
        Endianness::Little,
        true,
        Codec::EncapsulatedPixelData,
    )
}

/// Retrieve the default transfer syntax.
pub fn default() -> Ts {
    entries::IMPLICIT_VR_LITTLE_ENDIAN
}
