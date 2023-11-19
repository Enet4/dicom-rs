//! Implementation of Deflated Explicit VR Little Endian.
use std::io::{Read, Write};

use dicom_encoding::transfer_syntax::DataRWAdapter;
use flate2;
use flate2::Compression;

/// Immaterial type representing an adapter for deflated data.
#[derive(Debug)]
pub struct FlateAdapter;

impl<R: 'static, W: 'static> DataRWAdapter<R, W> for FlateAdapter
where
    R: Read,
    W: Write,
{
    // type Reader = Box<flate2::read::DeflateDecoder<R>>;
    // type Writer = Box<flate2::write::DeflateEncoder<W>>;
    type Reader = Box<dyn Read>;
    type Writer = Box<dyn Write>;

    fn adapt_reader(&self, reader: R) -> Self::Reader
    where
        R: Read,
    {
        Box::new(flate2::read::DeflateDecoder::new(reader))
    }

    fn adapt_writer(&self, writer: W) -> Self::Writer
    where
        W: Write,
    {
        Box::new(flate2::write::DeflateEncoder::new(writer, Compression::fast()))
    }
}
