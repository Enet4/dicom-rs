//! Module containing the DICOM Transfer Syntax data structure and related methods.
//! Similar to the DcmCodec in DCMTK, the `TransferSyntax` contains all of the necessary
//! algorithms for decoding and encoding DICOM data in a certain transfer syntax.
//! 
//! This crate does not host specific transfer syntaxes. Instead, they are created in
//! other crates and registered in the global transfer syntax registry. For more
//! information, please see the `dicom-transfer-syntax-registry` crate.

pub mod explicit_le;
pub mod explicit_be;
pub mod implicit_le;

use byteordered::Endianness;
use std::io::{Read, Write};
use crate::decode::basic::BasicDecoder;
use crate::decode::Decode;
use crate::encode::Encode;

/// A decoder with its type erased.
pub type DynamicDecoder = Box<dyn Decode<Source = dyn Read>>;

/// An encoder with its type erased.
pub type DynamicEncoder = Box<dyn Encode<Writer = dyn Write>>;

/// A DICOM transfer syntax specifier. The data RW adapter `A` specifies
/// custom codec capabilities when required.
#[derive(Debug)]
pub struct TransferSyntax<A = DynDataRWAdapter> {
    /// The unique identifier of the transfer syntax.
    uid: &'static str,
    /// The name of the transfer syntax.
    name: &'static str,
    /// The byte order of data.
    byte_order: Endianness,
    /// Whether the transfer syntax mandates an explicit value representation,
    /// or the VR is implicit.
    explicit_vr: bool,
    /// The transfer syntax' requirements and implemented capabilities.
    codec: Codec<A>,
}

/// Description regarding the encoding and decoding requirements of a transfer
/// syntax. This is also used as a means to describe whether pixel data is
/// encapsulated and whether this implementation supports it.
#[derive(Debug, Clone, PartialEq)]
pub enum Codec<A> {
    /// No codec is given, nor is it required.
    None,
    /// Custom encoding and decoding of the entire data set is required, but
    /// not supported. This could be used by a stub of
    /// _Deflated Explicit VR Little Endian_, for example.
    Unsupported,
    /// Custom encoding and decoding of the pixel data set is required, but
    /// not supported. The program should still be able to parse DICOM
    /// data sets and fetch the pixel data in its encapsulated form.
    EncapsulatedPixelData,
    /// A pixel data encapsulation codec is required and provided for reading
    /// and writing pixel data
    PixelData(A),
    /// A full, custom data set codec is required and provided.
    Dataset(A),
}

/// An alias for a transfer syntax specifier with no pixel data encapsulation
/// nor data set deflating.
pub type AdapterFreeTransferSyntax = TransferSyntax<NeverAdapter>;

/// An adapter of byte read and write streams.
pub trait DataRWAdapter<R, W> {
    type Reader: Read;
    type Writer: Write;

    /// Adapt a byte reader.
    fn adapt_reader(&self, reader: R) -> Self::Reader
    where
        R: Read;

    /// Adapt a byte writer.
    fn adapt_writer(&self, writer: W) -> Self::Writer
    where
        W: Write;
}

pub type DynDataRWAdapter = Box<dyn DataRWAdapter<Box<dyn Read>, Box<dyn Write>, Reader = Box<dyn Read>, Writer = Box<dyn Write>> + Send + Sync>;

impl<'a, T, R, W> DataRWAdapter<R, W> for &'a T
where
    T: DataRWAdapter<R, W>,
    R: Read,
    W: Write,
{
    type Reader = <T as DataRWAdapter<R, W>>::Reader;
    type Writer = <T as DataRWAdapter<R, W>>::Writer;

    /// Adapt a byte reader.
    fn adapt_reader(&self, reader: R) -> Self::Reader
    where
        R: Read
    {
        (**self).adapt_reader(reader)
    }

    /// Adapt a byte writer.
    fn adapt_writer(&self, writer: W) -> Self::Writer
    where
        W: Write
    {
        (**self).adapt_writer(writer)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NeverAdapter {}

impl<R, W> DataRWAdapter<R, W> for NeverAdapter {
    type Reader = Box<dyn Read>;
    type Writer = Box<dyn Write>;

    fn adapt_reader(&self, _reader: R) -> Self::Reader
    where
        R: Read,
    {
        unreachable!()
    }

    fn adapt_writer(&self, _writer: W) -> Self::Writer
    where
        W: Write,
    {
        unreachable!()
    }
}

impl<A> TransferSyntax<A> {
    pub const fn new(uid: &'static str, name: &'static str, byte_order: Endianness, explicit_vr: bool, codec: Codec<A>) -> Self {
        TransferSyntax {
            uid,
            name,
            byte_order,
            explicit_vr,
            codec,
        }
    }

    /// Obtain this transfer syntax' unique identifier.
    pub const fn uid(&self) -> &'static str {
        self.uid
    }

    /// Obtain the name of this transfer syntax.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Obtain this transfer syntax' expected endianness.
    pub const fn endianness(&self) -> Endianness {
        self.byte_order
    }

    /// Obtain this transfer syntax' codec specification.
    pub fn codec(&self) -> &Codec<A> {
        &self.codec
    }

    /// Check whether reading and writing of data sets is unsupported.
    pub fn unsupported(&self) -> bool {
        match self.codec {
            Codec::Unsupported => true,
            _ => false,
        }
    }

    /// Check whether reading and writing the pixel data is unsupported.
    pub fn unsupported_pixel_encapsulation(&self) -> bool {
        match self.codec {
            Codec::Unsupported | Codec::EncapsulatedPixelData => true,
            _ => false,
        }
    }

    /// Retrieve the appropriate data element decoder for this transfer syntax.
    /// Can yield none if decoding is not supported.
    /// 
    /// The resulting decoder does not consider pixel data encapsulation or
    /// data set compression rules. This means that the consumer of this method
    /// needs to adapt the reader before using the decoder.
    pub fn get_decoder(&self) -> Option<DynamicDecoder>
    {
        match (self.byte_order, self.explicit_vr) {
            (Endianness::Little, false) => {
                Some(Box::new(implicit_le::ImplicitVRLittleEndianDecoder::default()))
            },
            (Endianness::Little, true) => {
                Some(Box::new(explicit_le::ExplicitVRLittleEndianDecoder::default()))
            },
            (Endianness::Big, true) => {
                Some(Box::new(explicit_be::ExplicitVRBigEndianDecoder::default()))
            },
            _ => {
                None
            }
        }
    }

    /// Retrieve the appropriate data element encoder for this transfer syntax.
    /// Can yield none if encoding is not supported. The resulting encoder does not
    /// consider pixel data encapsulation or data set compression rules.
    pub fn get_encoder(&self) -> Option<DynamicEncoder> {
        match (self.byte_order, self.explicit_vr) {
            (Endianness::Little, false) => {
                Some(Box::new(implicit_le::ImplicitVRLittleEndianEncoder::default()))
            },
            (Endianness::Little, true) => {
                Some(Box::new(explicit_le::ExplicitVRLittleEndianEncoder::default()))
            },
            (Endianness::Big, true) => {
                Some(Box::new(explicit_be::ExplicitVRBigEndianEncoder::default()))
            },
            _ => {
                None
            }
        }
    }

    /// Obtain a dynamic basic decoder, based on this transfer syntax' expected endianness.
    pub fn get_basic_decoder(&self) -> BasicDecoder {
        BasicDecoder::from(self.endianness())
    }

    /// Type-erase the pixel data or data set codec.
    pub fn erased(self) -> TransferSyntax
    where
        A: Send + Sync + 'static,
        A: DataRWAdapter<Box<dyn Read>, Box<dyn Write>, Reader = Box<dyn Read>, Writer = Box<dyn Write>>,
    {
        let codec = match self.codec {
            Codec::Dataset(a) => Codec::Dataset(Box::new(a) as DynDataRWAdapter),
            Codec::PixelData(a) => Codec::PixelData(Box::new(a) as DynDataRWAdapter),
            Codec::EncapsulatedPixelData => Codec::EncapsulatedPixelData,
            Codec::Unsupported => Codec::Unsupported,
            Codec::None => Codec::None,
        };

        TransferSyntax {
            uid: self.uid,
            name: self.name,
            byte_order: self.byte_order,
            explicit_vr: self.explicit_vr,
            codec,
        }
    }
}
