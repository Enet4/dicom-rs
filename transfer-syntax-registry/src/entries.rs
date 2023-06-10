//! A list of compiled transfer syntax specifiers.
//!
//! The constants exported here refer to the library's built-in support
//! for DICOM transfer syntaxes.
//!
//! - **Fully implemented** means that the default transfer syntax registry
//!   provides built-in support for reading and writing data sets,
//!   as well as for encoding and decoding encapsulated pixel data,
//!   if applicable.
//! - When specified as **Implemented**,
//!   the transfer syntax is supported to some extent
//!   (usually decoding is supported but not encoding).
//! - **Stub descriptors** serve to provide information about
//!   the transfer syntax,
//!   and may provide partial support.
//!   In most cases it will be possible to read and write data sets,
//!   but not encode or decode encapsulated pixel data.
//!
//! With the `inventory-registry` feature,
//! stubs can be replaced by independently developed crates,
//! hence expanding support for those transfer syntaxes
//! to the registry.

use crate::create_ts_stub;
use byteordered::Endianness;
use dicom_encoding::{transfer_syntax::{AdapterFreeTransferSyntax as Ts, Codec}, NeverPixelAdapter};

#[cfg(any(feature = "jpeg", feature = "rle"))]
use dicom_encoding::transfer_syntax::{NeverAdapter, TransferSyntax};

#[cfg(feature = "jpeg")]
use crate::adapters::jpeg::JpegAdapter;
#[cfg(feature = "rle")]
use crate::adapters::rle_lossless::RleLosslessAdapter;

// -- the three base transfer syntaxes, fully supported --

/// **Fully implemented:** Implicit VR Little Endian: Default Transfer Syntax for DICOM
pub const IMPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2",
    "Implicit VR Little Endian",
    Endianness::Little,
    false,
    Codec::None,
);

/// **Fully implemented:** Explicit VR Little Endian
pub const EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.1",
    "Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::None,
);

/// **Fully implemented:** Explicit VR Big Endian
pub const EXPLICIT_VR_BIG_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.2",
    "Explicit VR Big Endian",
    Endianness::Big,
    true,
    Codec::None,
);

// -- transfer syntaxes with pixel data adapters, fully supported --

/// **Implemented:** RLE Lossless
#[cfg(feature = "rle")]
pub const RLE_LOSSLESS: TransferSyntax<NeverAdapter, RleLosslessAdapter, NeverPixelAdapter> = TransferSyntax::new(
    "1.2.840.10008.1.2.5",
    "RLE Lossless",
    Endianness::Little,
    true,
    Codec::EncapsulatedPixelData(Some(RleLosslessAdapter), None),
);
/// **Stub:** RLE Lossless
///
/// A native implementation is available
/// by enabling the `rle` Cargo feature.
#[cfg(not(feature = "rle"))]
pub const RLE_LOSSLESS: Ts = create_ts_stub("1.2.840.10008.1.2.5", "RLE Lossless");

// JPEG encoded pixel data

/// An alias for a transfer syntax specifier with `JpegPixelAdapter`
/// (note that only decoding is supported at the moment).
#[cfg(feature = "jpeg")]
type JpegTs<R = JpegAdapter, W = NeverPixelAdapter> = TransferSyntax<NeverAdapter, R, W>;

/// Create a transfer syntax with JPEG encapsulated pixel data
#[cfg(feature = "jpeg")]
const fn create_ts_jpeg(uid: &'static str, name: &'static str) -> JpegTs {
    TransferSyntax::new(
        uid,
        name,
        Endianness::Little,
        true,
        Codec::EncapsulatedPixelData(Some(JpegAdapter), None),
    )
}

/// **Implemented:** JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression
#[cfg(feature = "jpeg")]
pub const JPEG_BASELINE: JpegTs =
    create_ts_jpeg("1.2.840.10008.1.2.4.50", "JPEG Baseline (Process 1)");
/// **Implemented:** JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression
///
/// A native implementation is available
/// by enabling the `jpeg` Cargo feature.
#[cfg(not(feature = "jpeg"))]
pub const JPEG_BASELINE: Ts = create_ts_stub("1.2.840.10008.1.2.4.50", "JPEG Baseline (Process 1)");

/// **Implemented:** JPEG Extended (Process 2 & 4): Default Transfer Syntax for Lossy JPEG 12 Bit Image Compression (Process 4 only)
#[cfg(feature = "jpeg")]
pub const JPEG_EXTENDED: JpegTs =
    create_ts_jpeg("1.2.840.10008.1.2.4.51", "JPEG Extended (Process 2 & 4)");
/// **Stub descriptor:** JPEG Extended (Process 2 & 4): Default Transfer Syntax for Lossy JPEG 12 Bit Image Compression (Process 4 only)
///
/// A native implementation is available
/// by enabling the `jpeg` Cargo feature.
#[cfg(not(feature = "jpeg"))]
pub const JPEG_EXTENDED: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.51", "JPEG Extended (Process 2 & 4)");

/// **Implemented:** JPEG Lossless, Non-Hierarchical (Process 14)
#[cfg(feature = "jpeg")]
pub const JPEG_LOSSLESS_NON_HIERARCHICAL: JpegTs = create_ts_jpeg(
    "1.2.840.10008.1.2.4.57",
    "JPEG Lossless, Non-Hierarchical (Process 14)",
);
/// **Stub descriptor:** JPEG Lossless, Non-Hierarchical (Process 14)
///
/// A native implementation is available
/// by enabling the `jpeg` Cargo feature.
#[cfg(not(feature = "jpeg"))]
pub const JPEG_LOSSLESS_NON_HIERARCHICAL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.57",
    "JPEG Lossless, Non-Hierarchical (Process 14)",
);

/// **Implemented:** JPEG Lossless, Non-Hierarchical, First-Order Prediction
/// (Process 14 [Selection Value 1]):
/// Default Transfer Syntax for Lossless JPEG Image Compression
#[cfg(feature = "jpeg")]
pub const JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION: JpegTs = create_ts_jpeg(
    "1.2.840.10008.1.2.4.70",
    "JPEG Lossless, Non-Hierarchical, First-Order Prediction",
);
/// **Stub descriptor:** JPEG Lossless, Non-Hierarchical, First-Order Prediction
/// (Process 14 [Selection Value 1]):
/// Default Transfer Syntax for Lossless JPEG Image Compression
///
/// A native implementation is available
/// by enabling the `jpeg` Cargo feature.
#[cfg(not(feature = "jpeg"))]
pub const JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.70",
    "JPEG Lossless, Non-Hierarchical, First-Order Prediction",
);

// --- stub transfer syntaxes, known but not supported ---

/// **Stub descriptor:** Deflated Explicit VR Little Endian
pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.1.99",
    "Deflated Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::Dataset(None),
);

/// **Stub descriptor:** JPIP Referenced Deflate
pub const JPIP_REFERENCED_DEFLATE: Ts = Ts::new(
    "1.2.840.10008.1.2.4.95",
    "JPIP Referenced Deflate",
    Endianness::Little,
    true,
    Codec::Dataset(None),
);

// --- partially supported transfer syntaxes, pixel data encapsulation not supported ---

/// **Stub descriptor:** JPEG-LS Lossless Image Compression
pub const JPEG_LS_LOSSLESS_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.80",
    "JPEG-LS Lossless Image Compression",
);
/// **Stub descriptor:** JPEG-LS Lossy (Near-Lossless) Image Compression
pub const JPEG_LS_LOSSY_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.81",
    "JPEG-LS Lossy (Near-Lossless) Image Compression",
);

/// **Stub descriptor:** JPEG 2000 Image Compression (Lossless Only)
pub const JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.90",
    "JPEG 2000 Image Compression (Lossless Only)",
);
/// **Stub descriptor:** JPEG 2000 Image Compression
pub const JPEG_2000_IMAGE_COMPRESSION: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.91", "JPEG 2000 Image Compression");
/// **Stub descriptor:** JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.92",
    "JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)",
);
/// **Stub descriptor:** JPEG 2000 Part 2 Multi-component Image Compression
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.93",
    "JPEG 2000 Part 2 Multi-component Image Compression",
);

/// **Stub descriptor:** JPIP Referenced
pub const JPIP_REFERENCED: Ts = create_ts_stub("1.2.840.10008.1.2.4.94", "JPIP Referenced");

/// **Stub descriptor:** MPEG2 Main Profile / Main Level
pub const MPEG2_MAIN_PROFILE_MAIN_LEVEL: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.100", "MPEG2 Main Profile / Main Level");
/// **Stub descriptor:** Fragmentable MPEG2 Main Profile / Main Level
pub const FRAGMENTABLE_MPEG2_MAIN_PROFILE_MAIN_LEVEL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.100.1",
    "Fragmentable MPEG2 Main Profile / Main Level",
);
/// **Stub descriptor:** MPEG2 Main Profile / High Level
pub const MPEG2_MAIN_PROFILE_HIGH_LEVEL: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.101", "MPEG2 Main Profile / High Level");
/// **Stub descriptor:** Fragmentable MPEG2 Main Profile / High Level
pub const FRAGMENTABLE_MPEG2_MAIN_PROFILE_HIGH_LEVEL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.101.1",
    "Fragmentable MPEG2 Main Profile / High Level",
);
/// **Stub descriptor:** MPEG-4 AVC/H.264 High Profile / Level 4.1
pub const MPEG4_AVC_H264_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.102",
    "MPEG-4 AVC/H.264 High Profile / Level 4.1",
);
/// **Stub descriptor:** Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.1
pub const FRAGMENTABLE_MPEG4_AVC_H264_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.102.1",
    "Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.1",
);
/// **Stub descriptor:** MPEG-4 AVC/H.264 BD-Compatible High Profile / Level 4.1
pub const MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.103",
    "MPEG-4 AVC/H.264 BD-Compatible High Profile / Level 4.1",
);
/// **Stub descriptor:** Fragmentable MPEG-4 AVC/H.264 BD-Compatible High Profile / Level 4.1
pub const FRAGMENTABLE_MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.103.1",
    "Fragmentable MPEG-4 AVC/H.264 BD-Compatible High Profile / Level 4.1",
);
/// **Stub descriptor:** MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video
pub const MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.104",
    "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video",
);
/// **Stub descriptor:** Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video
pub const FRAGMENTABLE_MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.104.1",
    "Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video",
);
/// **Stub descriptor:** MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video
pub const MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.105",
    "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video",
);
/// **Stub descriptor:** Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video
pub const FRAGMENTABLE_MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.105.1",
    "Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video",
);
/// **Stub descriptor:** MPEG-4 AVC/H.264 High Profile / Level 4.2
pub const MPEG4_AVC_H264_STEREO_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.106",
    "MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2",
);
/// **Stub descriptor:** Fragmentable MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2
pub const FRAGMENTABLE_MPEG4_AVC_H264_STEREO_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.106.1",
    "Fragmentable MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2",
);
/// **Stub descriptor:** HEVC/H.265 Main Profile / Level 5.1
pub const HEVC_H265_MAIN_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.107",
    "HEVC/H.265 Main Profile / Level 5.1",
);
/// **Stub descriptor:** HEVC/H.265 Main 10 Profile / Level 5.1
pub const HEVC_H265_MAIN_10_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.108",
    "HEVC/H.265 Main 10 Profile / Level 5.1",
);
/// **Stub descriptor:** SMPTE ST 2110-20 Uncompressed Progressive Active Video
pub const SMPTE_ST_2110_20_UNCOMPRESSED_PROGRESSIVE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.7.1",
    "SMPTE ST 2110-20 Uncompressed Progressive Active Video",
);
/// **Stub descriptor:** SMPTE ST 2110-20 Uncompressed Interlaced Active Video
pub const SMPTE_ST_2110_20_UNCOMPRESSED_INTERLACED: Ts = create_ts_stub(
    "1.2.840.10008.1.2.7.2",
    "SMPTE ST 2110-20 Uncompressed Interlaced Active Video",
);
/// **Stub descriptor:** SMPTE ST 2110-30 PCM Digital Audio
pub const SMPTE_ST_2110_30_PCM: Ts = create_ts_stub(
    "1.2.840.10008.1.2.7.3",
    "SMPTE ST 2110-30 PCM Digital Audio",
);
