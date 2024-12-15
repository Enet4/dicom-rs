//! Implementation of Deflated Explicit VR Little Endian.
use std::io::{Read, Write};

use dicom_encoding::transfer_syntax::DataRWAdapter;
use flate2::Compression;

/// An adapter for deflated data.
#[derive(Debug)]
pub struct FlateAdapter;

impl DataRWAdapter for FlateAdapter {
    fn adapt_reader<'r>(&self, reader: Box<dyn Read + 'r>) -> Box<dyn Read + 'r> {
        Box::new(flate2::read::DeflateDecoder::new(reader))
    }

    fn adapt_writer<'w>(&self, writer: Box<dyn Write + 'w>) -> Box<dyn Write + 'w> {
        Box::new(flate2::write::DeflateEncoder::new(writer, Compression::fast()))
    }
}
