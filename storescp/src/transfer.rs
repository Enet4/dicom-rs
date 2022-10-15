//! Accepted storage transfer options

use dicom_transfer_syntax_registry::entries;

/// A list of supported abstract syntaxes for storage services
pub static ABSTRACT_SYNTAXES: &[&str] = &[
    "1.2.840.10008.5.1.4.1.1.2",
    "1.2.840.10008.5.1.4.1.1.2.1",
    "1.2.840.10008.5.1.4.1.1.9",
    "1.2.840.10008.5.1.4.1.1.8",
    "1.2.840.10008.5.1.4.1.1.7",
    "1.2.840.10008.5.1.4.1.1.6",
    "1.2.840.10008.5.1.4.1.1.5",
    "1.2.840.10008.5.1.4.1.1.4",
    "1.2.840.10008.5.1.4.1.1.4.1",
    "1.2.840.10008.5.1.4.1.1.4.2",
    "1.2.840.10008.5.1.4.1.1.4.3",
    "1.2.840.10008.5.1.4.1.1.3",
    "1.2.840.10008.5.1.4.1.1.2",
    "1.2.840.10008.5.1.4.1.1.1",
    "1.2.840.10008.5.1.4.1.1.1.1",
    "1.2.840.10008.5.1.4.1.1.1.1.1",
    "1.2.840.10008.5.1.4.1.1.104.1",
    "1.2.840.10008.5.1.4.1.1.104.2",
    "1.2.840.10008.5.1.4.1.1.104.3",
    "1.2.840.10008.5.1.4.1.1.11.1",
    "1.2.840.10008.5.1.4.1.1.128",
    "1.2.840.10008.5.1.4.1.1.13.1.3",
    "1.2.840.10008.5.1.4.1.1.13.1.4",
    "1.2.840.10008.5.1.4.1.1.13.1.5",
    "1.2.840.10008.5.1.4.1.1.130",
    "1.2.840.10008.5.1.4.1.1.481.1",
    "1.2.840.10008.5.1.4.1.1.20",
    "1.2.840.10008.5.1.4.1.1.3.1",
    "1.2.840.10008.5.1.4.1.1.7",
    "1.2.840.10008.5.1.4.1.1.7.1",
    "1.2.840.10008.5.1.4.1.1.7.2",
    "1.2.840.10008.5.1.4.1.1.7.3",
    "1.2.840.10008.5.1.4.1.1.7.4",
    "1.2.840.10008.5.1.4.1.1.88.11",
    "1.2.840.10008.5.1.4.1.1.88.22",
    "1.2.840.10008.5.1.4.1.1.88.33",
];

/// List of base, uncompressed transfer syntaxes with native pixel data
pub static NATIVE_TRANSFER_SYNTAXES: &[&str] = &[
    "1.2.840.10008.1.2",
    "1.2.840.10008.1.2.1",
];

/// List of accepted transfer syntaxes
pub static TRANSFER_SYNTAXES: &[&str] = &[
    "1.2.840.10008.1.2",
    "1.2.840.10008.1.2.1",
    entries::RLE_LOSSLESS.uid(),
    entries::JPEG_BASELINE.uid(),
    entries::JPEG_EXTENDED.uid(),
    entries::JPEG_LOSSLESS_NON_HIERARCHICAL.uid(),
    entries::JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.uid(),
    entries::JPEG_LS_LOSSLESS_IMAGE_COMPRESSION.uid(),
    entries::JPEG_LS_LOSSY_IMAGE_COMPRESSION.uid(),
    entries::JPEG_2000_IMAGE_COMPRESSION.uid(),
    entries::JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.uid(),
    entries::JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION.uid(),
    entries::JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY.uid(),
    entries::MPEG2_MAIN_PROFILE_HIGH_LEVEL.uid(),
    entries::MPEG2_MAIN_PROFILE_MAIN_LEVEL.uid(),
    entries::MPEG4_AVC_H264_BD_COMPATIBLE_HIGH_PROFILE.uid(),
    entries::MPEG4_AVC_H264_HIGH_PROFILE.uid(),
    entries::MPEG4_AVC_H264_HIGH_PROFILE_FOR_2D_VIDEO.uid(),
    entries::MPEG4_AVC_H264_HIGH_PROFILE_FOR_3D_VIDEO.uid(),
    entries::MPEG4_AVC_H264_STEREO_HIGH_PROFILE.uid(),
    entries::HEVC_H265_MAIN_10_PROFILE.uid(),
    entries::HEVC_H265_MAIN_PROFILE.uid(),
    entries::SMPTE_ST_2110_20_UNCOMPRESSED_INTERLACED.uid(),
    entries::SMPTE_ST_2110_20_UNCOMPRESSED_PROGRESSIVE.uid(),
    entries::SMPTE_ST_2110_30_PCM.uid(),
];
