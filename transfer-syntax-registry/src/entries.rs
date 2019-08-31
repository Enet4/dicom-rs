// Compiled transfer syntax specifiers.

// -- the three base transfer syntaxes, fully supported --

pub const IMPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2",
    "Implicit VR Little Endian",
    Endianness::Little,
    false,
    Codec::None,
);
submit_transfer_syntax!(IMPLICIT_VR_LITTLE_ENDIAN);

pub const EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.1",
    "Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::None,
);
submit_transfer_syntax!(EXPLICIT_VR_LITTLE_ENDIAN);

pub const EXPLICIT_VR_BIG_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.2",
    "Explicit VR Big Endian",
    Endianness::Big,
    true,
    Codec::None,
);
submit_transfer_syntax!(EXPLICIT_VR_BIG_ENDIAN);

// --- stub transfer syntaxes, known but not supported ---

pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new(
    "1.2.840.10008.1.2.1.99",
    "Deflated Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::Unsupported,
);
submit_transfer_syntax!(DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN);

pub const JPIP_DEREFERENCED_DEFLATE: Ts = Ts::new(
    "1.2.840.10008.1.2.4.95",
    "JPIP Referenced Deflate",
    Endianness::Little,
    true,
    Codec::Unsupported,
);
submit_transfer_syntax!(JPIP_DEREFERENCED_DEFLATE);

// --- partially supported transfer syntaxes, pixel data encapsulation not supported ---

pub const JPEG_BASELINE: Ts = create_ts_stub("1.2.840.10008.1.2.4.50", "JPEG Baseline (Process 1)");
submit_transfer_syntax!(JPEG_BASELINE);
pub const JPEG_EXTENDED: Ts = create_ts_stub("1.2.840.10008.1.2.4.51", "JPEG Extended (Process 2 & 4)");
submit_transfer_syntax!(JPEG_EXTENDED);
pub const JPEG_LOSSLESS_NON_HIERARCHICAL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.57", "JPEG Lossless, Non-Hierarchical (Process 14)");
submit_transfer_syntax!(JPEG_LOSSLESS_NON_HIERARCHICAL);
pub const JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.70", "JPEG Lossless, Non-Hierarchical, First-Order Prediction");
submit_transfer_syntax!(JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION);
pub const JPEG_LS_LOSSLESS_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.80", "JPEG-LS Lossless Image Compression");
submit_transfer_syntax!(JPEG_LS_LOSSLESS_IMAGE_COMPRESSION);
pub const JPEG_LS_LOSSY_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.81", "JPEG-LS Lossy (Near-Lossless) Image Compression");
submit_transfer_syntax!(JPEG_LS_LOSSY_IMAGE_COMPRESSION);
pub const JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.90", "JPEG 2000 Image Compression (Lossless Only)");
submit_transfer_syntax!(JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY);
pub const JPEG_2000_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.91", "JPEG 2000 Image Compression");
submit_transfer_syntax!(JPEG_2000_IMAGE_COMPRESSION);
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.92", "JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)");
submit_transfer_syntax!(JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY);
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.93", "JPEG 2000 Part 2 Multi-component Image Compression");
submit_transfer_syntax!(JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION);
pub const JPIP_REFERENCED: Ts = create_ts_stub("1.2.840.10008.1.2.4.94", "JPIP Referenced");
submit_transfer_syntax!(JPIP_REFERENCED);
pub const MPEG2_MAIN_PROFILE_MAIN_LEVEL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.100", "MPEG2 Main Profile / Main Level");
submit_transfer_syntax!(MPEG2_MAIN_PROFILE_MAIN_LEVEL);
pub const MPEG2_MAIN_PROFILE_HIGH_LEVEL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.101", "MPEG2 Main Profile / High Level");
submit_transfer_syntax!(MPEG2_MAIN_PROFILE_HIGH_LEVEL);
pub const MPEG4_AVC_H264_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.102", "MPEG-4 AVC/H.264 High Profile / Level 4.1");
submit_transfer_syntax!(MPEG4_AVC_H264_HIGH_PROFILE);
pub const MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.103", "MPEG-4 AVC/H.264 BD-Compatible High Profile / Level 4.1");
submit_transfer_syntax!(MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE);
pub const MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.104", "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video");
submit_transfer_syntax!(MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO);
pub const MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.105", "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video");
submit_transfer_syntax!(MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO);
pub const MPEG4_AVC_H264_STEREO_HIGH_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.106", "MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2");
submit_transfer_syntax!(MPEG4_AVC_H264_STEREO_HIGH_PROFILE);
pub const HEVC_H265_MAIN_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.107", "HEVC/H.265 Main Profile / Level 5.1");
submit_transfer_syntax!(HEVC_H265_MAIN_PROFILE);
pub const HEVC_H265_MAIN_10_PROFILE: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.108", "HEVC/H.265 Main 10 Profile / Level 5.1");
submit_transfer_syntax!(HEVC_H265_MAIN_10_PROFILE);
pub const RLE_LOSSLESS: Ts = create_ts_stub("1.2.840.10008.1.2.5", "RLE Lossless");
submit_transfer_syntax!(RLE_LOSSLESS);
