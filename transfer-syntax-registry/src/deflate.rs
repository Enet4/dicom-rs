//! Implementation of Deflated Explicit VR Little Endian.
use std::io::{Read, Write};

use byteordered::Endianness;
use dicom_encoding::{Codec, TransferSyntax, transfer_syntax::DataRWAdapter};
use flate2;
use flate2::Compression;

/// Immaterial type representing an adapter for deflated data.
#[derive(Debug)]
pub struct FlateAdapter;

/// **Fully implemented**: Deflated Explicit VR Little Endian
pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: TransferSyntax<FlateAdapter> = TransferSyntax::new(
    "1.2.840.10008.1.2.1.99",
    "Deflated Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::Dataset(FlateAdapter),
);

impl<R: 'static, W: 'static> DataRWAdapter<R, W> for FlateAdapter
where
    R: Read,
    W: Write,
{
    type Reader = flate2::read::DeflateDecoder<R>;
    type Writer = flate2::write::DeflateEncoder<W>;

    fn adapt_reader(&self, reader: R) -> Self::Reader
    where
        R: Read,
    {
        flate2::read::DeflateDecoder::new(reader)
    }

    fn adapt_writer(&self, writer: W) -> Self::Writer
    where
        W: Write,
    {
        flate2::write::DeflateEncoder::new(writer, Compression::fast())
    }
}
