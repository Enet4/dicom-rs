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

use crate::{adapters::uncompressed::UncompressedAdapter, create_ts_stub};
use byteordered::Endianness;
use dicom_encoding::transfer_syntax::{AdapterFreeTransferSyntax as Ts, Codec};

use dicom_encoding::transfer_syntax::{NeverAdapter, TransferSyntax};

#[cfg(any(feature = "rle", feature = "openjp2", feature = "openjpeg-sys"))]
use dicom_encoding::NeverPixelAdapter;

#[cfg(feature = "jpeg")]
use crate::adapters::jpeg::JpegAdapter;
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
use crate::adapters::jpeg2k::Jpeg2000Adapter;
#[cfg(feature = "charls")]
use crate::adapters::jpegls::{JpegLsAdapter, JpegLsLosslessWriter};
#[cfg(feature = "jpegxl")]
use crate::adapters::jpegxl::{JpegXlAdapter, JpegXlLosslessEncoder};
#[cfg(feature = "rle")]
use crate::adapters::rle_lossless::RleLosslessAdapter;
#[cfg(feature = "flate2")]
use crate::deflate::FlateAdapter;

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
pub const EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new_ele(
    "1.2.840.10008.1.2.1",
    "Explicit VR Little Endian",
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

/// **Fully implemented:** Encapsulated Uncompressed Explicit VR Little Endian
pub const ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN: TransferSyntax<
    NeverAdapter,
    UncompressedAdapter,
    UncompressedAdapter,
> = TransferSyntax::new_ele(
    "1.2.840.10008.1.2.1.98",
    "Encapsulated Uncompressed Explicit VR Little Endian",
    Codec::EncapsulatedPixelData(Some(UncompressedAdapter), Some(UncompressedAdapter)),
);

// -- transfer syntaxes with pixel data adapters, fully supported --

/// **Implemented:** RLE Lossless
#[cfg(feature = "rle")]
pub const RLE_LOSSLESS: TransferSyntax<NeverAdapter, RleLosslessAdapter, NeverPixelAdapter> =
    TransferSyntax::new_ele(
        "1.2.840.10008.1.2.5",
        "RLE Lossless",
        Codec::EncapsulatedPixelData(Some(RleLosslessAdapter), None),
    );
/// **Stub:** RLE Lossless
///
/// A native implementation is available
/// by enabling the `rle` Cargo feature.
#[cfg(not(feature = "rle"))]
pub const RLE_LOSSLESS: Ts = create_ts_stub("1.2.840.10008.1.2.5", "RLE Lossless");

// JPEG encoded pixel data

/// An alias for a transfer syntax specifier with [`JpegAdapter`]
/// (supports decoding and encoding to JPEG baseline,
/// support for JPEG extended and JPEG lossless may vary).
#[cfg(feature = "jpeg")]
type JpegTs<R = JpegAdapter, W = JpegAdapter> = TransferSyntax<NeverAdapter, R, W>;

/// Create a transfer syntax with JPEG encapsulated pixel data
#[cfg(feature = "jpeg")]
const fn create_ts_jpeg(uid: &'static str, name: &'static str, encoder: bool) -> JpegTs {
    TransferSyntax::new_ele(
        uid,
        name,
        Codec::EncapsulatedPixelData(
            Some(JpegAdapter),
            if encoder { Some(JpegAdapter) } else { None },
        ),
    )
}

/// **Implemented:** JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression
#[cfg(feature = "jpeg")]
pub const JPEG_BASELINE: JpegTs =
    create_ts_jpeg("1.2.840.10008.1.2.4.50", "JPEG Baseline (Process 1)", true);
/// **Implemented:** JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression
///
/// A native implementation is available
/// by enabling the `jpeg` Cargo feature.
#[cfg(not(feature = "jpeg"))]
pub const JPEG_BASELINE: Ts = create_ts_stub("1.2.840.10008.1.2.4.50", "JPEG Baseline (Process 1)");

/// **Implemented:** JPEG Extended (Process 2 & 4): Default Transfer Syntax for Lossy JPEG 12 Bit Image Compression (Process 4 only)
#[cfg(feature = "jpeg")]
pub const JPEG_EXTENDED: JpegTs = create_ts_jpeg(
    "1.2.840.10008.1.2.4.51",
    "JPEG Extended (Process 2 & 4)",
    false,
);
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
    false,
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
    false,
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

#[cfg(feature = "flate2")]
/// **Fully implemented**: Deflated Explicit VR Little Endian
pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: TransferSyntax<FlateAdapter, NeverAdapter, NeverAdapter> = TransferSyntax::new(
    "1.2.840.10008.1.2.1.99",
    "Deflated Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::Dataset(Some(FlateAdapter)),
);

#[cfg(not(feature = "flate2"))]
/// **Stub descriptor:** Deflated Explicit VR Little Endian
pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: Ts = Ts::new_ele(
    "1.2.840.10008.1.2.1.99",
    "Deflated Explicit VR Little Endian",
    Codec::Dataset(None),
);

// --- stub transfer syntaxes, known but not supported ---


/// **Stub descriptor:** JPIP Referenced Deflate
pub const JPIP_REFERENCED_DEFLATE: Ts = Ts::new_ele(
    "1.2.840.10008.1.2.4.95",
    "JPIP Referenced Deflate",
    Codec::Dataset(None),
);

/// **Stub descriptor:** JPIP Referenced Deflate
pub const JPIP_HTJ2K_REFERENCED_DEFLATE: Ts = Ts::new_ele(
    "1.2.840.10008.1.2.4.205",
    "JPIP HTJ2K Referenced Deflate",
    Codec::Dataset(None),
);

// --- JPEG 2000 support ---

/// An alias for a transfer syntax specifier with [`Jpeg2000Adapter`]
/// (supports decoding and encoding to JPEG baseline,
/// support for JPEG extended and JPEG lossless may vary).
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
type Jpeg2000Ts<R = Jpeg2000Adapter, W = NeverPixelAdapter> = TransferSyntax<NeverAdapter, R, W>;

/// Create a transfer syntax with JPEG 2000 encapsulated pixel data
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
const fn create_ts_jpeg2k(uid: &'static str, name: &'static str) -> Jpeg2000Ts {
    TransferSyntax::new_ele(
        uid,
        name,
        Codec::EncapsulatedPixelData(Some(Jpeg2000Adapter), None),
    )
}

/// **Decoder implementation:** JPEG 2000 Image Compression (Lossless Only)
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub const JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY: Jpeg2000Ts = create_ts_jpeg2k(
    "1.2.840.10008.1.2.4.90",
    "JPEG 2000 Image Compression (Lossless Only)",
);
/// **Stub descriptor:** JPEG 2000 Image Compression (Lossless Only)
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub const JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.90",
    "JPEG 2000 Image Compression (Lossless Only)",
);

/// **Decoder implementation:** JPEG 2000 Image Compression
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub const JPEG_2000_IMAGE_COMPRESSION: Jpeg2000Ts =
    create_ts_jpeg2k("1.2.840.10008.1.2.4.91", "JPEG 2000 Image Compression");
/// **Stub descriptor:** JPEG 2000 Image Compression
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub const JPEG_2000_IMAGE_COMPRESSION: Ts =
    create_ts_stub("1.2.840.10008.1.2.4.91", "JPEG 2000 Image Compression");

/// **Decoder implementation:** JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY: Jpeg2000Ts =
    create_ts_jpeg2k(
        "1.2.840.10008.1.2.4.92",
        "JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)",
    );
/// **Stub descriptor:** JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.92",
    "JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)",
);

/// **Decoder implementation:** JPEG 2000 Part 2 Multi-component Image Compression
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION: Jpeg2000Ts = create_ts_jpeg2k(
    "1.2.840.10008.1.2.4.93",
    "JPEG 2000 Part 2 Multi-component Image Compression",
);
/// **Stub descriptor:** JPEG 2000 Part 2 Multi-component Image Compression
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub const JPEG_2000_PART2_MULTI_COMPONENT_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.93",
    "JPEG 2000 Part 2 Multi-component Image Compression",
);

// --- HTJ2K ---

/// **Decoder implementation:** High-Throughput JPEG 2000 Image Compression (Lossless Only)
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub const HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY: Jpeg2000Ts = create_ts_jpeg2k(
    "1.2.840.10008.1.2.4.201",
    "High-Throughput JPEG 2000 Image Compression (Lossless Only)",
);
/// **Stub descriptor:** High-Throughput JPEG 2000 Image Compression (Lossless Only)
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub const HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.201",
    "High-Throughput JPEG 2000 Image Compression (Lossless Only)",
);

/// **Decoder implementation:** High-Throughput JPEG 2000 with RPCL Options Image Compression (Lossless Only)
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub const HIGH_THROUGHPUT_JPEG_2000_WITH_RPCL_OPTIONS_IMAGE_COMPRESSION_LOSSLESS_ONLY: Jpeg2000Ts = create_ts_jpeg2k(
    "1.2.840.10008.1.2.4.202",
    "High-Throughput JPEG 2000 with RPCL Options Image Compression (Lossless Only)",
);
/// **Stub descriptor:** High-Throughput JPEG 2000 Image Compression (Lossless Only)
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub const HIGH_THROUGHPUT_JPEG_2000_WITH_RPCL_OPTIONS_IMAGE_COMPRESSION_LOSSLESS_ONLY: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.202",
    "High-Throughput JPEG 2000 with RPCL Options Image Compression (Lossless Only)",
);

/// **Decoder implementation:** High-Throughput JPEG 2000 Image Compression
#[cfg(any(feature = "openjp2", feature = "openjpeg-sys"))]
pub const HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION: Jpeg2000Ts = create_ts_jpeg2k(
    "1.2.840.10008.1.2.4.203",
    "High-Throughput JPEG 2000 Image Compression",
);
/// **Stub descriptor:** High-Throughput JPEG 2000 Image Compression
#[cfg(not(any(feature = "openjp2", feature = "openjpeg-sys")))]
pub const HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.203",
    "High-Throughput JPEG 2000 Image Compression",
);


// --- JPEG-LS ---

/// An alias for a transfer syntax specifier with [`JpegLSAdapter`] as the decoder
/// and an arbitrary encoder (since two impls are available)
#[cfg(feature = "charls")]
type JpegLSTs<W> = TransferSyntax<NeverAdapter, JpegLsAdapter, W>;

/// **Decoder Implementation:** JPEG-LS Lossless Image Compression
#[cfg(feature = "charls")]
pub const JPEG_LS_LOSSLESS_IMAGE_COMPRESSION: JpegLSTs<JpegLsLosslessWriter> = TransferSyntax::new_ele(
    "1.2.840.10008.1.2.4.80",
    "JPEG-LS Lossless Image Compression",
    Codec::EncapsulatedPixelData(Some(JpegLsAdapter), Some(JpegLsLosslessWriter)),
);

/// **Stub descriptor:** JPEG-LS Lossless Image Compression
#[cfg(not(feature = "charls"))]
pub const JPEG_LS_LOSSLESS_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.80",
    "JPEG-LS Lossless Image Compression",
);

/// **Decoder Implementation:** JPEG-LS Lossy (Near-Lossless) Image Compression
#[cfg(feature = "charls")]
pub const JPEG_LS_LOSSY_IMAGE_COMPRESSION: JpegLSTs<JpegLsAdapter> = TransferSyntax::new_ele(
    "1.2.840.10008.1.2.4.81",
    "JPEG-LS Lossy (Near-Lossless) Image Compression",
    Codec::EncapsulatedPixelData(Some(JpegLsAdapter), Some(JpegLsAdapter)),
);

// --- JPEG XL support ---

/// An alias for a transfer syntax specifier with [`JpegXLAdapter`]
#[cfg(feature = "jpegxl")]
type JpegXlTs<R = JpegXlAdapter, W = JpegXlAdapter> = TransferSyntax<NeverAdapter, R, W>;

/// **Implemented:** JPEG XL Lossless
#[cfg(feature = "jpegxl")]
pub const JPEG_XL_LOSSLESS: JpegXlTs<JpegXlAdapter, JpegXlLosslessEncoder> = TransferSyntax::new_ele(
    "1.2.840.10008.1.2.4.110",
    "JPEG XL Lossless",
    Codec::EncapsulatedPixelData(Some(JpegXlAdapter), Some(JpegXlLosslessEncoder)),
);

/// **Stub descriptor:** JPEG XL Lossless
#[cfg(not(feature = "jpegxl"))]
pub const JPEG_XL_LOSSLESS: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.110",
    "JPEG XL Lossless"
);

/// **Decoder Implementation:** JPEG XL Recompression
#[cfg(feature = "jpegxl")]
pub const JPEG_XL_RECOMPRESSION: JpegXlTs = TransferSyntax::new_ele(
    "1.2.840.10008.1.2.4.111",
    "JPEG XL Recompression",
    Codec::EncapsulatedPixelData(Some(JpegXlAdapter), None),
);

/// **Stub descriptor:** JPEG XL Recompression
#[cfg(not(feature = "jpegxl"))]
pub const JPEG_XL_RECOMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.111",
    "JPEG XL Recompression"
);

/// **Implemented:** JPEG XL
#[cfg(feature = "jpegxl")]
pub const JPEG_XL: JpegXlTs = TransferSyntax::new_ele(
    "1.2.840.10008.1.2.4.112",
    "JPEG XL",
    Codec::EncapsulatedPixelData(Some(JpegXlAdapter), Some(JpegXlAdapter)),
);

/// **Stub descriptor:** JPEG XL
#[cfg(not(feature = "jpegxl"))]
pub const JPEG_XL: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.112",
    "JPEG XL"
);

/// **Stub descriptor:** JPEG-LS Lossy (Near-Lossless) Image Compression
#[cfg(not(feature = "charls"))]
pub const JPEG_LS_LOSSY_IMAGE_COMPRESSION: Ts = create_ts_stub(
    "1.2.840.10008.1.2.4.81",
    "JPEG-LS Lossy (Near-Lossless) Image Compression",
);

/// **Stub descriptor:** JPIP Referenced
pub const JPIP_REFERENCED: Ts = create_ts_stub("1.2.840.10008.1.2.4.94", "JPIP Referenced");

/// **Stub descriptor:** JPIP HT2JK Referenced
pub const JPIP_HTJ2K_REFERENCED: Ts = create_ts_stub("1.2.840.10008.1.2.4.204", "JPIP HTJ2K Referenced");

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
