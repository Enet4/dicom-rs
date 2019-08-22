//! Module containing the DICOM Transfer Syntax data structure and related methods.
//! Similar to the DcmCodec in DCMTK, the `TransferSyntax` contains all of the necessary
//! algorithms for decoding and encoding DICOM data in a certain transfer syntax.

pub mod registry;
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

/// A DICOM transfer syntax specifier.
#[derive(Debug)]
pub struct TransferSyntax<P = DynDataRWAdapter, A = DynDataRWAdapter> {
    /// The unique identifier of the transfer syntax.
    uid: &'static str,
    /// The name of the transfer syntax.
    name: &'static str,
    /// The byte order of data.
    byte_order: Endianness,
    /// Whether the transfer syntax mandates an explicit value representation,
    /// or the VR is implicit.
    explicit_vr: bool,
    /// The codec implementation for retrieving and writing encapsulated pixel
    /// data, if applicable.
    pixel_codec: Option<P>,
    /// The codec implementation for the full DICOM data set, if applicable.
    /// This would be used by _Deflated Explicit VR Little Endian_, for example.
    dataset_codec: Option<A>,
}

/// An alias for a transfer syntax specifier with no pixel data encapsulation
/// nor data set deflating.
pub type AdapterFreeTransferSyntax = TransferSyntax<NeverAdapter, NeverAdapter>;

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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct ErasedAdapter<T>(T);

impl<T: 'static, R: 'static, W: 'static> DataRWAdapter<R, W> for ErasedAdapter<T>
where
    T: DataRWAdapter<R, W>,
    R: Read + Send + Sync,
    W: Write + Send + Sync,
{
    type Reader = Box<dyn Read>;
    type Writer = Box<dyn Write>;

    fn adapt_reader(&self, reader: R) -> Self::Reader
    where
        R: Read,
    {
        Box::from(self.0.adapt_reader(reader)) as Box<_>
    }

    fn adapt_writer(&self, writer: W) -> Self::Writer
    where
        W: Write,
    {
        Box::from(self.0.adapt_writer(writer)) as Box<_>
    }
}

impl<P, A> TransferSyntax<P, A> {
    /// Obtain this transfer syntax' unique identifier.
    pub fn uid(&self) -> &'static str {
        self.uid
    }

    /// Obtain the name of this transfer syntax.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Obtain this transfer syntax' expected endianness.
    pub fn endianness(&self) -> Endianness {
        self.byte_order
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

    /// Type-erase the pixel codec and data set codec.
    pub fn erased(self) -> TransferSyntax
    where
        P: Send + Sync + 'static,
        A: Send + Sync + 'static,
        P: DataRWAdapter<Box<dyn Read>, Box<dyn Write>, Reader = Box<dyn Read>, Writer = Box<dyn Write>>,
        A: DataRWAdapter<Box<dyn Read>, Box<dyn Write>, Reader = Box<dyn Read>, Writer = Box<dyn Write>>,
    {
        TransferSyntax {
            uid: self.uid,
            name: self.name,
            byte_order: self.byte_order,
            explicit_vr: self.explicit_vr,
            pixel_codec: self.pixel_codec.map(|c| Box::new(c) as DynDataRWAdapter),
            dataset_codec: self.dataset_codec.map(|c| Box::new(c) as DynDataRWAdapter),
        }
    }
}

/// Retrieve the default transfer syntax.
pub fn default() -> AdapterFreeTransferSyntax {
    IMPLICIT_VR_LITTLE_ENDIAN
}

pub const IMPLICIT_VR_LITTLE_ENDIAN: AdapterFreeTransferSyntax = TransferSyntax {
    uid: "1.2.840.10008.1.2",
    name: "Implicit VR Little Endian",
    byte_order: Endianness::Little,
    explicit_vr: false,
    pixel_codec: None,
    dataset_codec: None,
};

pub const EXPLICIT_VR_LITTLE_ENDIAN: AdapterFreeTransferSyntax = TransferSyntax {
    uid: "1.2.840.10008.1.2.1",
    name: "Explicit VR Little Endian",
    byte_order: Endianness::Little,
    explicit_vr: true,
    pixel_codec: None,
    dataset_codec: None,
};

pub const EXPLICIT_VR_BIG_ENDIAN: AdapterFreeTransferSyntax = TransferSyntax {
    uid: "1.2.840.10008.1.2.2",
    name: "Explicit VR Big Endian",
    byte_order: Endianness::Big,
    explicit_vr: true,
    pixel_codec: None,
    dataset_codec: None,
};
