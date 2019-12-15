//! Compiled transfer syntax specifiers.

use crate::create_ts_stub;
use byteordered::Endianness;
use dicom_encoding::transfer_syntax::{AdapterFreeTransferSyntax as Ts, Codec};

// -- the three base transfer syntaxes, fully supported --

pub const IMPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2",
    "Implicit VR Little Endian",
    Endianness::Little,
    false,
    Codec::None,
);

pub const EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.1",
    "Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::None,
);

pub const EXPLICIT_VR_BIG_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.2",
    "Explicit VR Big Endian",
    Endianness::Big,
    true,
    Codec::None,
);

// --- stub transfer syntaxes, known but not supported ---

pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.1.99",
    "Deflated Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::Unsupported,
);

pub const JPIP_DEREFERENCED_DEFLATE: Ts = Ts::new(
    "1.2.840.10008.1.2.4.95",
    "JPIP Referenced Deflate",
    Endianness::Little,
    true,
    Codec::Unsupported,
);

// --- partially supported transfer syntaxes, pixel data encapsulation not supported ---

pub const JPEG_BASELINE: Ts = create_ts_stub("1.2.840.10008.1.2.4.50", "JPEG Baseline (Process 1)");
pub const JPEG_EXTENDED: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.51", "JPEG Extended (Process 2 & 4)");
pub const JPEG_LOSSLESS_NON_HIERARCHICAL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.57",
    "JPEG Lossless, Non-Hierarchical (Process 14)",
);
pub const JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.70",
    "JPEG Lossless, Non-Hierarchical, First-Order Prediction",
);
pub const JPEG_LS_LOSSLESS_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.80",
    "JPEG-LS Lossless Image Compression",
);
pub const JPEG_LS_LOSSY_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.81",
    "JPEG-LS Lossy (Near-Lossless) Image Compression",
);
pub const JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.90",
    "JPEG 2000 Image Compression (Lossless Only)",
);
pub const JPEG_2000_IMAGE_COMPRESSION: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.91", "JPEG 2000 Image Compression");
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.92",
    "JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)",
);
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.93",
    "JPEG 2000 Part 2 Multi-component Image Compression",
);
pub const JPIP_REFERENCED: Ts = create_ts_stub("1.2.840.10008.1.2.4.94", "JPIP Referenced");
pub const MPEG2_MAIN_PROFILE_MAIN_LEVEL: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.100", "MPEG2 Main Profile / Main Level");
pub const MPEG2_MAIN_PROFILE_HIGH_LEVEL: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.101", "MPEG2 Main Profile / High Level");
pub const MPEG4_AVC_H264_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.102",
    "MPEG-4 AVC/H.264 High Profile / Level 4.1",
);
pub const MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.103",
    "MPEG-4 AVC/H.264 BD-Compatible High Profile / Level 4.1",
);
pub const MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.104",
    "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video",
);
pub const MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.105",
    "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video",
);
pub const MPEG4_AVC_H264_STEREO_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.106",
    "MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2",
);
pub const HEVC_H265_MAIN_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.107",
    "HEVC/H.265 Main Profile / Level 5.1",
);
pub const HEVC_H265_MAIN_10_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.108",
    "HEVC/H.265 Main 10 Profile / Level 5.1",
);
pub const RLE_LOSSLESS: Ts = create_ts_stub("1.2.840.10008.1.2.5", "RLE Lossless");
