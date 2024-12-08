#![deny(trivial_numeric_casts, unsafe_code, unstable_features)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    unused_qualifications,
    unused_import_braces
)]
//! This crate contains the DICOM transfer syntax registry.
//!
//! The transfer syntax registry maps a DICOM UID of a transfer syntax (TS)
//! into the respective transfer syntax specifier.
//! This specifier defines:
//!
//! 1. how to read and write DICOM data sets;
//! 2. how to decode and encode pixel data.
//!
//! Support may be partial, in which case the data set can be retrieved
//! but the pixel data may not be decoded through the DICOM-rs ecosystem.
//! By default, adapters for encapsulated pixel data
//! need to be explicitly added by dependent projects,
//! such as `dicom-pixeldata`.
//! When adding `dicom-transfer-syntax-registry` yourself,
//! to include support for some transfer syntaxes with encapsulated pixel data,
//! add the **`native`** Cargo feature
//! or one of the other image encoding features available.
//!
//! By default, a fixed known set of transfer syntaxes are provided as built in.
//! Moreover, support for more TSes can be extended by other crates
//! through the [inventory] pattern,
//! in which the registry is automatically populated before main.
//! This is done by enabling the Cargo feature **`inventory-registry`**.
//! The feature can be left disabled
//! for environments which do not support `inventory`,
//! with the downside of only providing the built-in transfer syntaxes.
//!
//! All registered TSes will be readily available
//! through the [`TransferSyntaxRegistry`] type.
//!
//! This registry is intended to be used in the development of higher level APIs,
//! which should learn to negotiate and resolve the expected
//! transfer syntax automatically.
//!
//! ## Transfer Syntaxes
//!
//! This crate encompasses basic DICOM level of conformance,
//! plus support for some transfer syntaxes with compressed pixel data.
//! _Implicit VR Little Endian_,
//! _Explicit VR Little Endian_,
//! and _Explicit VR Big Endian_
//! are fully supported.
//! Support may vary for transfer syntaxes which rely on encapsulated pixel data.
//!
//! | transfer syntax               | decoding support     | encoding support |
//! |-------------------------------|----------------------|------------------|
//! | JPEG Baseline (Process 1)     | Cargo feature `jpeg` | ✓ |
//! | JPEG Extended (Process 2 & 4) | Cargo feature `jpeg` | x |
//! | JPEG Lossless, Non-Hierarchical (Process 14) | Cargo feature `jpeg` | x |
//! | JPEG Lossless, Non-Hierarchical, First-Order Prediction (Process 14 [Selection Value 1]) | Cargo feature `jpeg` | x |
//! | JPEG-LS Lossless              | Cargo feature `charls` | ✓ |
//! | JPEG-LS Lossy (Near-Lossless) | Cargo feature `charls` | ✓ |
//! | JPEG 2000 (Lossless Only)     | Cargo feature `openjp2` or `openjpeg-sys` | x |
//! | JPEG 2000                     | Cargo feature `openjp2` or `openjpeg-sys` | x |
//! | JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only) | Cargo feature `openjp2` or `openjpeg-sys` | x |
//! | JPEG 2000 Part 2 Multi-component Image Compression | Cargo feature `openjp2` or `openjpeg-sys` | x |
//! | High-Throughput JPEG 2000 (Lossless Only) | Cargo feature `openjp2` or `openjpeg-sys` | x |
//! | High-Throughput JPEG 2000 with RPCL Options (Lossless Only) | Cargo feature `openjp2` or `openjpeg-sys` | x |
//! | High-Throughput JPEG 2000     | Cargo feature `openjp2` or `openjpeg-sys` | x |
//! | JPEG XL Lossless              | Cargo feature `jpegxl` | ✓ |
//! | JPEG XL Recompression         | Cargo feature `jpegxl` | x |
//! | JPEG XL                       | Cargo feature `jpegxl` | ✓ |
//! | RLE Lossless                  | Cargo feature `rle` | x |
//!
//! Cargo features behind `native` (`jpeg`, `rle`) are added by default.
//! They provide implementations that are written in pure Rust
//! and are likely available in all supported platforms without issues.
//! Additional codecs are opt-in by enabling Cargo features,
//! for scenarios where a native implementation is not available,
//! or alternative implementations are available.
//!
//! - `charls` provides support for JPEG-LS
//!   by linking to the CharLS reference implementation,
//!   which is written in C++.
//!   No alternative JPEG-LS implementations are available at the moment. 
//! - `openjpeg-sys` provides a binding to the OpenJPEG reference implementation,
//!   which is written in C and is statically linked.
//!   It may offer better performance than the pure Rust implementation,
//!   but cannot be used in WebAssembly.
//!   Include `openjpeg-sys-threads` to build OpenJPEG with multithreading.
//! - `openjp2` provides a binding to a computer-translated Rust port of OpenJPEG.
//!   Due to the nature of this crate,
//!   it might not work on all modern platforms.
//! - `jpegxl` adds JPEG XL support using `jxl-oxide` for decoding
//!   and `zune-jpegxl` for encoding.
//!
//! Transfer syntaxes which are not supported,
//! either due to being unable to read the data set
//! or decode encapsulated pixel data,
//! are listed as _stubs_ for partial support.
//! The full list is available in the [`entries`] module.
//! These stubs may also be replaced by separate libraries
//! if using the inventory-based registry.
//!
//! [inventory]: https://docs.rs/inventory/0.3.15/inventory

use dicom_encoding::transfer_syntax::{
    AdapterFreeTransferSyntax as Ts, Codec, TransferSyntaxIndex,
};
use lazy_static::lazy_static;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

pub use dicom_encoding::TransferSyntax;
pub mod entries;

mod adapters;
#[cfg(feature = "deflate")]
mod deflate;

#[cfg(feature = "inventory-registry")]
pub use dicom_encoding::inventory;

/// Main implementation of a registry of DICOM transfer syntaxes.
///
/// Consumers would generally use [`TransferSyntaxRegistry`] instead.
pub struct TransferSyntaxRegistryImpl {
    m: HashMap<&'static str, TransferSyntax>,
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
    /// Obtain an iterator of all registered transfer syntaxes.
    pub fn iter(&self) -> impl Iterator<Item = &TransferSyntax> {
        self.m.values()
    }

    /// Obtain a DICOM codec by transfer syntax UID.
    fn get<U: AsRef<str>>(&self, uid: U) -> Option<&TransferSyntax> {
        let ts_uid = uid
            .as_ref()
            .trim_end_matches(|c: char| c.is_whitespace() || c == '\0');
        self.m.get(ts_uid)
    }

    /// Register the given transfer syntax (TS) to the system. It can override
    /// another TS with the same UID, in the only case that the TS requires
    /// certain codecs which are not supported by the previously registered
    /// TS. If no such requirements are imposed, this function returns `false`
    /// and no changes are made.
    fn register(&mut self, ts: TransferSyntax) -> bool {
        match self.m.entry(ts.uid()) {
            Entry::Occupied(mut e) => {
                let replace = match (&e.get().codec(), ts.codec()) {
                    (Codec::Dataset(None), Codec::Dataset(Some(_)))
                    | (
                        Codec::EncapsulatedPixelData(None, None),
                        Codec::EncapsulatedPixelData(..),
                    )
                    | (
                        Codec::EncapsulatedPixelData(Some(_), None),
                        Codec::EncapsulatedPixelData(Some(_), Some(_)),
                    )
                    | (
                        Codec::EncapsulatedPixelData(None, Some(_)),
                        Codec::EncapsulatedPixelData(Some(_), Some(_)),
                    ) => true,
                    // weird one ahead: the two specifiers do not agree on
                    // requirements, better keep it as a separate match arm for
                    // debugging purposes
                    (Codec::Dataset(None), Codec::EncapsulatedPixelData(_, _)) => {
                        tracing::warn!("Inconsistent requirements for transfer syntax {}: `Dataset` cannot be replaced by `EncapsulatedPixelData`", ts.uid());
                        false
                    }
                    // another weird one:
                    // the two codecs do not agree on requirements
                    (Codec::EncapsulatedPixelData(_, _), Codec::Dataset(None)) => {
                        tracing::warn!("Inconsistent requirements for transfer syntax {}: `EncapsulatedPixelData` cannot be replaced by `Dataset`", ts.uid());
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

impl TransferSyntaxRegistry {
    /// Obtain an iterator of all registered transfer syntaxes.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &TransferSyntax> {
        get_registry().iter()
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

    static ref REGISTRY: TransferSyntaxRegistryImpl = {
        let mut registry = TransferSyntaxRegistryImpl {
            m: HashMap::with_capacity(32),
        };

        use self::entries::*;
        let built_in_ts: [TransferSyntax; 45] = [
            IMPLICIT_VR_LITTLE_ENDIAN.erased(),
            EXPLICIT_VR_LITTLE_ENDIAN.erased(),
            EXPLICIT_VR_BIG_ENDIAN.erased(),

            ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN.erased(),

            DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN.erased(),
            JPIP_REFERENCED_DEFLATE.erased(),
            JPIP_HTJ2K_REFERENCED_DEFLATE.erased(),

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
            HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.erased(),
            HIGH_THROUGHPUT_JPEG_2000_WITH_RPCL_OPTIONS_IMAGE_COMPRESSION_LOSSLESS_ONLY.erased(),
            HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION.erased(),
            JPEG_XL_LOSSLESS.erased(),
            JPEG_XL_RECOMPRESSION.erased(),
            JPEG_XL.erased(),
            JPIP_REFERENCED.erased(),
            JPIP_HTJ2K_REFERENCED.erased(),
            MPEG2_MAIN_PROFILE_MAIN_LEVEL.erased(),
            FRAGMENTABLE_MPEG2_MAIN_PROFILE_MAIN_LEVEL.erased(),
            MPEG2_MAIN_PROFILE_HIGH_LEVEL.erased(),
            FRAGMENTABLE_MPEG2_MAIN_PROFILE_HIGH_LEVEL.erased(),
            MPEG4_AVC_H264_HIGH_PROFILE.erased(),
            FRAGMENTABLE_MPEG4_AVC_H264_HIGH_PROFILE.erased(),
            MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE.erased(),
            FRAGMENTABLE_MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE.erased(),
            MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO.erased(),
            FRAGMENTABLE_MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO.erased(),
            MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO.erased(),
            FRAGMENTABLE_MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO.erased(),
            MPEG4_AVC_H264_STEREO_HIGH_PROFILE.erased(),
            FRAGMENTABLE_MPEG4_AVC_H264_STEREO_HIGH_PROFILE.erased(),
            HEVC_H265_MAIN_PROFILE.erased(),
            HEVC_H265_MAIN_10_PROFILE.erased(),
            RLE_LOSSLESS.erased(),
            SMPTE_ST_2110_20_UNCOMPRESSED_PROGRESSIVE.erased(),
            SMPTE_ST_2110_20_UNCOMPRESSED_INTERLACED.erased(),
            SMPTE_ST_2110_30_PCM.erased(),
        ];

        // add built-in TSes manually
        for ts in built_in_ts {
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
    use dicom_encoding::transfer_syntax::TransferSyntaxFactory;

    for TransferSyntaxFactory(tsf) in inventory::iter::<TransferSyntaxFactory> {
        let ts = tsf();
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
    TransferSyntax::new_ele(uid, name, Codec::EncapsulatedPixelData(None, None))
}

/// Retrieve the default transfer syntax.
pub fn default() -> Ts {
    entries::IMPLICIT_VR_LITTLE_ENDIAN
}

#[cfg(test)]
mod tests {
    use dicom_encoding::TransferSyntaxIndex;

    use crate::TransferSyntaxRegistry;

    #[test]
    fn has_mandatory_tss() {
        let implicit_vr_le = TransferSyntaxRegistry
            .get("1.2.840.10008.1.2")
            .expect("transfer syntax registry should provide Implicit VR Little Endian");
        assert_eq!(implicit_vr_le.uid(), "1.2.840.10008.1.2");
        assert!(implicit_vr_le.is_fully_supported());

        // should also work with trailing null character
        let implicit_vr_le_2 = TransferSyntaxRegistry.get("1.2.840.10008.1.2\0").expect(
            "transfer syntax registry should provide Implicit VR Little Endian with padded TS UID",
        );

        assert_eq!(implicit_vr_le_2.uid(), implicit_vr_le.uid());

        let explicit_vr_le = TransferSyntaxRegistry
            .get("1.2.840.10008.1.2.1")
            .expect("transfer syntax registry should provide Explicit VR Little Endian");
        assert_eq!(explicit_vr_le.uid(), "1.2.840.10008.1.2.1");
        assert!(explicit_vr_le.is_fully_supported());

        // should also work with trailing null character
        let explicit_vr_le_2 = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1\0").expect(
            "transfer syntax registry should provide Explicit VR Little Endian with padded TS UID",
        );

        assert_eq!(explicit_vr_le_2.uid(), explicit_vr_le.uid());
    }

    #[test]
    fn provides_iter() {
        let all_tss: Vec<_> = TransferSyntaxRegistry.iter().collect();

        assert!(all_tss.len() >= 2);

        // contains at least Implicit VR Little Endian and Explicit VR Little Endian
        assert!(all_tss.iter().any(|ts| ts.uid() == "1.2.840.10008.1.2"));
        assert!(all_tss.iter().any(|ts| ts.uid() == "1.2.840.10008.1.2.1"));
    }
}
