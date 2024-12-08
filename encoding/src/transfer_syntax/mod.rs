//! Module containing the DICOM Transfer Syntax data structure and related methods.
//! Similar to the DcmCodec in DCMTK, the `TransferSyntax` contains all of the necessary
//! algorithms for decoding and encoding DICOM data in a certain transfer syntax.
//!
//! This crate does not host specific transfer syntaxes. Instead, they are created in
//! other crates and registered in the global transfer syntax registry,
//! which implements [`TransferSyntaxIndex`].
//! For more information, please see the [`dicom-transfer-syntax-registry`] crate,
//! which provides built-in implementations.
//!
//! This module allows you to register your own transfer syntaxes.
//! With the `inventory-registry` Cargo feature,
//! you can use the macro [`submit_transfer_syntax`](crate::submit_transfer_syntax)
//! or [`submit_ele_transfer_syntax`](crate::submit_ele_transfer_syntax)
//! to instruct the compiler to include your implementation in the registry.
//! Without the `inventory`-based registry
//! (in case your environment does not support it),
//! you can still roll your own [transfer syntax index][1].
//!
//! [1]: TransferSyntaxIndex
//! [`dicom-transfer-syntax-registry`]: https://docs.rs/dicom-transfer-syntax-registry

use crate::adapters::{
    DynPixelDataReader, DynPixelDataWriter, NeverPixelAdapter, PixelDataReader, PixelDataWriter,
};
use crate::decode::{
    basic::BasicDecoder, explicit_be::ExplicitVRBigEndianDecoder,
    explicit_le::ExplicitVRLittleEndianDecoder, implicit_le::ImplicitVRLittleEndianDecoder,
    DecodeFrom,
};
use crate::encode::{
    explicit_be::ExplicitVRBigEndianEncoder, explicit_le::ExplicitVRLittleEndianEncoder,
    implicit_le::ImplicitVRLittleEndianEncoder, EncodeTo, EncoderFor,
};
use std::io::{Read, Write};

pub use byteordered::Endianness;

/// A decoder with its type erased.
pub type DynDecoder<S> = Box<dyn DecodeFrom<S>>;

/// An encoder with its type erased.
pub type DynEncoder<'w, W> = Box<dyn EncodeTo<W> + 'w>;

/// A DICOM transfer syntax specifier.
///
/// Custom encoding and decoding capabilities
/// are defined via the parameter types `D` and `P`,
/// The type parameter `D` specifies
/// an adapter for reading and writing data sets,
/// whereas `P` specifies the encoder and decoder of encapsulated pixel data.
///
/// This type is usually consumed in its "type erased" form,
/// with its default parameter types.
/// On the other hand, implementers of `TransferSyntax` will typically specify
/// concrete types for `D` and `P`,
/// which are type-erased before registration.
/// If the transfer syntax requires no data set codec,
/// `D` can be assigned to the utility type [`NeverAdapter`].
/// If pixel data encoding/decoding is not needed or not supported,
/// you can assign `P` to [`NeverPixelAdapter`].
#[derive(Debug)]
pub struct TransferSyntax<D = DynDataRWAdapter, R = DynPixelDataReader, W = DynPixelDataWriter> {
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
    codec: Codec<D, R, W>,
}

/// Wrapper type for a provider of transfer syntax descriptors.
///
/// This is a piece of the plugin interface for
/// registering and collecting transfer syntaxes.
/// Implementers and consumers of transfer syntaxes
/// will usually not interact with it directly.
/// In order to register a new transfer syntax,
/// see the macro [`submit_transfer_syntax`](crate::submit_transfer_syntax).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TransferSyntaxFactory(pub fn() -> TransferSyntax);

#[cfg(feature = "inventory-registry")]
// Collect transfer syntax specifiers from other crates.
inventory::collect!(TransferSyntaxFactory);

/// Trait for a container/repository of transfer syntax specifiers.
///
/// Types implementing this trait are held responsible for populating
/// themselves with a set of transfer syntaxes, which can be fully supported,
/// partially supported, or not supported. Usually, only one implementation
/// of this trait is used for the entire program,
/// the most common one being the `TransferSyntaxRegistry` type
/// from [`transfer-syntax-registry`].
///
/// [`transfer-syntax-registry`]: https://docs.rs/dicom-transfer-syntax-registry
pub trait TransferSyntaxIndex {
    /// Obtain a DICOM transfer syntax by its respective UID.
    ///
    /// Implementations of this method should be robust to the possible
    /// presence of trailing null characters (`\0`) in `uid`.
    fn get(&self, uid: &str) -> Option<&TransferSyntax>;
}

impl<T: ?Sized> TransferSyntaxIndex for &T
where
    T: TransferSyntaxIndex,
{
    fn get(&self, uid: &str) -> Option<&TransferSyntax> {
        (**self).get(uid)
    }
}

#[cfg(feature = "inventory-registry")]
#[macro_export]
/// Submit a transfer syntax specifier to be supported by the
/// program's runtime. This is to be used by crates wishing to provide
/// additional support for a certain transfer syntax using the
/// main transfer syntax registry.
///
/// This macro does not actually "run" anything, so place it outside of a
/// function body at the root of the crate.
/// The expression is evaluated when the transfer syntax registry is populated
/// upon the first request,
/// and must resolve to a value of type [`TransferSyntax<D, P>`],
/// for valid definitions of the parameter types `D` and `P`.
/// The macro will type-erase these parameters automatically.
///
/// # Example
///
/// One common use case is wanting to read data sets
/// of DICOM objects in a private transfer syntax,
/// even when a decoder for that pixel data is not available.
/// By writing a simple stub at your project's root,
/// the rest of the ecosystem will know
/// how to read and write data sets in that transfer syntax.
///
/// ```
/// use dicom_encoding::{
///     submit_transfer_syntax, AdapterFreeTransferSyntax, Codec, Endianness,
/// };
///
/// submit_transfer_syntax!(AdapterFreeTransferSyntax::new(
///     // Transfer Syntax UID
///     "1.3.46.670589.33.1.4.1",
///     // Name/alias
///     "CT Private ELE",
///     // Data set byte order
///     Endianness::Little,
///     // Explicit VR (true) or Implicit VR (false)
///     true,
///     Codec::EncapsulatedPixelData(None, None),  // pixel data codec
/// ));
/// ```
///
/// With [`Codec::EncapsulatedPixelData(None, None)`][1],
/// we are indicating that the transfer syntax uses encapsulated pixel data.
/// albeit without the means to decode or encode it.
/// See the [`adapters`](crate::adapters) module
/// to know how to write pixel data encoders and decoders.
///
/// [1]: Codec::EncapsulatedPixelData
macro_rules! submit_transfer_syntax {
    ($ts: expr) => {
        $crate::inventory::submit! {
            $crate::transfer_syntax::TransferSyntaxFactory(|| ($ts).erased())
        }
    };
}

#[cfg(not(feature = "inventory-registry"))]
#[macro_export]
/// Submit a transfer syntax specifier to be supported by the
/// program's runtime. This is to be used by crates wishing to provide
/// additional support for a certain transfer syntax using the
/// main transfer syntax registry.
///
/// This macro does actually "run" anything, so place it outside of a
/// function body at the root of the crate.
///
/// Without the `inventory-registry` feature, this request is ignored.
macro_rules! submit_transfer_syntax {
    ($ts: expr) => {
        // ignore request
    };
}

#[cfg(feature = "inventory-registry")]
#[macro_export]
/// Submit an explicit VR little endian transfer syntax specifier
/// to be supported by the program's runtime.
///
/// This macro is equivalent in behavior as [`submit_transfer_syntax`](crate::submit_transfer_syntax),
/// but it is easier to use when
/// writing support for compressed pixel data formats,
/// which are usually in explicit VR little endian.
///
/// This macro does not actually "run" anything, so place it outside of a
/// function body at the root of the crate.
/// The expression is evaluated when the transfer syntax registry is populated
/// upon the first request,
/// and must resolve to a value of type [`Codec<D, R, W>`],
/// for valid definitions of the parameter types `D`, `R`, and `W`.
/// The macro will type-erase these parameters automatically.
///
/// # Example
///
/// One common use case is wanting to read data sets
/// of DICOM objects in a private transfer syntax,
/// even when a decoder for that pixel data is not available.
/// By writing a simple stub at your project's root,
/// the rest of the ecosystem will know
/// how to read and write data sets in that transfer syntax.
///
/// ```
/// use dicom_encoding::{submit_ele_transfer_syntax, Codec};
///
/// submit_ele_transfer_syntax!(
///     // Transfer Syntax UID
///     "1.3.46.670589.33.1.4.1",
///     // Name/alias
///     "CT Private ELE",
///     // pixel data codec
///     Codec::EncapsulatedPixelData(None, None)
/// );
/// ```
///
/// With [`Codec::EncapsulatedPixelData(None, None)`][1],
/// we are indicating that the transfer syntax uses encapsulated pixel data.
/// albeit without the means to decode or encode it.
/// See the [`adapters`](crate::adapters) module
/// to know how to write pixel data encoders and decoders.
///
/// [1]: Codec::EncapsulatedPixelData
macro_rules! submit_ele_transfer_syntax {
    ($uid: expr, $name: expr, $codec: expr) => {
        $crate::submit_transfer_syntax! {
            $crate::AdapterFreeTransferSyntax::new_ele(
                $uid,
                $name,
                $codec
            )
        }
    };
}

#[cfg(not(feature = "inventory-registry"))]
#[macro_export]
/// Submit an explicit VR little endian transfer syntax specifier
/// to be supported by the program's runtime.
///
/// This macro is equivalent in behavior as [`submit_transfer_syntax`],
/// but it is easier to use when
/// writing support for compressed pixel data formats,
/// which are usually in explicit VR little endian.
///
/// This macro does actually "run" anything, so place it outside of a
/// function body at the root of the crate.
///
/// Without the `inventory-registry` feature, this request is ignored.
macro_rules! submit_ele_transfer_syntax {
    ($uid: literal, $name: literal, $codec: expr) => {
        // ignore request
    };
}

/// A description and possible implementation regarding
/// the encoding and decoding requirements of a transfer syntax.
/// This is also used as a means to describe whether pixel data is encapsulated
/// and whether this implementation supports decoding and/or encoding it.
///
/// ### Type parameters
///
/// - `D` should implement [`DataRWAdapter`]
///   and defines how one should read and write DICOM data sets,
///   such as in the case for deflated data.
///   When no special considerations for data set reading and writing
///   are necessary, this can be set to [`NeverAdapter`].
/// - `R` should implement [`PixelDataReader`],
///   and enables programs to convert encapsulated pixel data fragments
///   into native pixel data.
/// - `W` should implement [`PixelDataWriter`],
///   and enables programs to convert native pixel data
///   into encapsulated pixel data.
///
#[derive(Debug, Clone, PartialEq)]
pub enum Codec<D, R, W> {
    /// No codec is required for this transfer syntax.
    ///
    /// Pixel data, if any, should be in its _native_, unencapsulated format.
    None,
    /// Pixel data for this transfer syntax is encapsulated
    /// and likely subjected to a specific encoding process.
    /// The first part of the tuple struct contains the pixel data decoder,
    /// whereas the second item is for the pixel data encoder.
    ///
    /// Decoding of the pixel data is not supported
    /// if the decoder is `None`.
    /// In this case, the program should still be able to
    /// parse DICOM data sets
    /// and fetch the pixel data in its encapsulated form.
    EncapsulatedPixelData(Option<R>, Option<W>),
    /// A custom data set codec is required for reading and writing data sets.
    ///
    /// If the item in the tuple struct is `None`,
    /// then no reading and writing whatsoever is supported.
    /// This could be used by a stub of
    /// _Deflated Explicit VR Little Endian_, for example.
    Dataset(Option<D>),
}

/// An alias for a transfer syntax specifier with no pixel data encapsulation
/// nor data set deflating.
pub type AdapterFreeTransferSyntax =
    TransferSyntax<NeverAdapter, NeverPixelAdapter, NeverPixelAdapter>;

/// A fully dynamic adapter of byte read and write streams.
pub trait DataRWAdapter {
    /// Adapt a byte reader.
    fn adapt_reader<'r>(&self, reader: Box<dyn Read + 'r>) -> Box<dyn Read + 'r>;

    /// Adapt a byte writer.
    fn adapt_writer<'w>(&self, writer: Box<dyn Write + 'w>) -> Box<dyn Write + 'w>;
}

/// Alias type for a dynamically dispatched data adapter.
pub type DynDataRWAdapter = Box<dyn DataRWAdapter + Send + Sync>;

impl<T> DataRWAdapter for &'_ T
where
    T: DataRWAdapter,
{
    /// Adapt a byte reader.
    fn adapt_reader<'r>(&self, reader: Box<dyn Read + 'r>) -> Box<dyn Read + 'r> {
        (**self).adapt_reader(reader)
    }

    /// Adapt a byte writer.
    fn adapt_writer<'w>(&self, writer: Box<dyn Write + 'w>) -> Box<dyn Write + 'w> {
        (**self).adapt_writer(writer)
    }
}

/// An immaterial type representing a data set adapter which is never required,
/// and as such is never instantiated.
/// Most transfer syntaxes use this,
/// as they do not have to adapt readers and writers
/// for encoding and decoding data sets.
/// The main exception is in the family of
/// _Deflated Explicit VR Little Endian_ transfer syntaxes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NeverAdapter {}

impl DataRWAdapter for NeverAdapter {

    fn adapt_reader<'r>(&self, _reader: Box<dyn Read + 'r>) -> Box<dyn Read + 'r> {
        unreachable!()
    }

    fn adapt_writer<'w>(&self, _writer: Box<dyn Write + 'w>) -> Box<dyn Write + 'w> {
        unreachable!()
    }
}

impl<D, R, W> TransferSyntax<D, R, W> {
    /// Create a new transfer syntax descriptor.
    ///
    /// Note that only transfer syntax implementers are expected to
    /// construct TS descriptors from scratch.
    /// For a practical usage of transfer syntaxes,
    /// one should look up an existing transfer syntax registry by UID.
    ///
    /// # Example
    ///
    /// To register a private transfer syntax in your program,
    /// use [`submit_transfer_syntax`](crate::submit_transfer_syntax)
    /// outside of a function body:
    ///  
    /// ```no_run
    /// # use dicom_encoding::{
    /// #     submit_transfer_syntax, Codec, Endianness,
    /// #     NeverAdapter, NeverPixelAdapter, TransferSyntax,
    /// # };
    /// submit_transfer_syntax! {
    ///     TransferSyntax::<NeverAdapter, NeverPixelAdapter, NeverPixelAdapter>::new(
    ///         "1.3.46.670589.33.1.4.1",
    ///         "CT-Private-ELE",
    ///         Endianness::Little,
    ///         true,
    ///         Codec::EncapsulatedPixelData(None, None),
    ///     )
    /// }
    /// ```
    pub const fn new(
        uid: &'static str,
        name: &'static str,
        byte_order: Endianness,
        explicit_vr: bool,
        codec: Codec<D, R, W>,
    ) -> Self {
        TransferSyntax {
            uid,
            name,
            byte_order,
            explicit_vr,
            codec,
        }
    }

    /// Create a new descriptor
    /// for a transfer syntax in explicit VR little endian.
    ///
    /// Note that only transfer syntax implementers are expected to
    /// construct TS descriptors from scratch.
    /// For a practical usage of transfer syntaxes,
    /// one should look up an existing transfer syntax registry by UID.
    ///
    /// # Example
    ///
    /// To register a private transfer syntax in your program,
    /// use [`submit_transfer_syntax`](crate::submit_transfer_syntax)
    /// outside of a function body:
    ///  
    /// ```no_run
    /// # use dicom_encoding::{
    /// #     submit_transfer_syntax, Codec,
    /// #     NeverAdapter, NeverPixelAdapter, TransferSyntax,
    /// # };
    /// submit_transfer_syntax! {
    ///     TransferSyntax::<NeverAdapter, NeverPixelAdapter, NeverPixelAdapter>::new_ele(
    ///         "1.3.46.670589.33.1.4.1",
    ///         "CT-Private-ELE",
    ///         Codec::EncapsulatedPixelData(None, None),
    ///     )
    /// }
    /// ```
    ///
    /// See [`submit_ele_transfer_syntax`](crate::submit_ele_transfer_syntax)
    /// for an alternative.
    pub const fn new_ele(uid: &'static str, name: &'static str, codec: Codec<D, R, W>) -> Self {
        TransferSyntax {
            uid,
            name,
            byte_order: Endianness::Little,
            explicit_vr: true,
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
    pub fn codec(&self) -> &Codec<D, R, W> {
        &self.codec
    }

    /// Check whether this transfer syntax specifier provides a complete
    /// implementation,
    /// meaning that it can both decode and encode in this transfer syntax.
    pub fn is_fully_supported(&self) -> bool {
        matches!(
            self.codec,
            Codec::None | Codec::Dataset(Some(_)) | Codec::EncapsulatedPixelData(Some(_), Some(_)),
        )
    }

    /// Check whether no codecs are required for this transfer syntax,
    /// meaning that a complete implementation is available
    /// and no pixel data conversion is required.
    pub fn is_codec_free(&self) -> bool {
        matches!(self.codec, Codec::None)
    }

    /// Check whether neither reading nor writing of data sets is supported.
    /// If this is `true`, encoding and decoding will not be available.
    pub fn is_unsupported(&self) -> bool {
        matches!(self.codec, Codec::Dataset(None))
    }

    /// Check whether this transfer syntax expects pixel data to be encapsulated.
    ///
    /// This does not imply that the pixel data can be decoded. 
    pub fn is_encapsulated_pixel_data(&self) -> bool {
        matches!(self.codec, Codec::EncapsulatedPixelData(..))
    }

    /// Check whether reading and writing the pixel data is unsupported.
    /// If this is `true`, encoding and decoding of the data set may still
    /// be possible, but the pixel data will only be available in its
    /// encapsulated form.
    pub fn is_unsupported_pixel_encapsulation(&self) -> bool {
        matches!(
            self.codec,
            Codec::Dataset(None) | Codec::EncapsulatedPixelData(None, None)
        )
    }

    /// Check whether this codec can fully decode
    /// both data sets and pixel data.
    pub fn can_decode_all(&self) -> bool {
        matches!(
            self.codec,
            Codec::None | Codec::Dataset(Some(_)) | Codec::EncapsulatedPixelData(Some(_), _)
        )
    }

    /// Check whether this codec can decode the data set.
    pub fn can_decode_dataset(&self) -> bool {
        matches!(
            self.codec,
            Codec::None | Codec::Dataset(Some(_)) | Codec::EncapsulatedPixelData(..)
        )
    }

    /// Retrieve the appropriate data element decoder for this transfer syntax.
    /// Can yield none if decoding is not supported.
    ///
    /// The resulting decoder does not consider pixel data encapsulation or
    /// data set compression rules. This means that the consumer of this method
    /// needs to adapt the reader before using the decoder.
    pub fn decoder<'s>(&self) -> Option<DynDecoder<dyn Read + 's>> {
        self.decoder_for()
    }

    /// Retrieve the appropriate data element decoder for this transfer syntax
    /// and given reader type (this method is not object safe).
    /// Can yield none if decoding is not supported.
    ///
    /// The resulting decoder does not consider pixel data encapsulation or
    /// data set compression rules. This means that the consumer of this method
    /// needs to adapt the reader before using the decoder.
    pub fn decoder_for<S>(&self) -> Option<DynDecoder<S>>
    where
        Self: Sized,
        S: ?Sized + Read,
    {
        match (self.byte_order, self.explicit_vr) {
            (Endianness::Little, false) => Some(Box::<ImplicitVRLittleEndianDecoder<_>>::default()),
            (Endianness::Little, true) => Some(Box::<ExplicitVRLittleEndianDecoder>::default()),
            (Endianness::Big, true) => Some(Box::<ExplicitVRBigEndianDecoder>::default()),
            _ => None,
        }
    }

    /// Retrieve the appropriate data element encoder for this transfer syntax.
    /// Can yield none if encoding is not supported. The resulting encoder does not
    /// consider pixel data encapsulation or data set compression rules.
    pub fn encoder<'w>(&self) -> Option<DynEncoder<'w, dyn Write + 'w>> {
        self.encoder_for()
    }

    /// Retrieve the appropriate data element encoder for this transfer syntax
    /// and the given writer type (this method is not object safe).
    /// Can yield none if encoding is not supported. The resulting encoder does not
    /// consider pixel data encapsulation or data set compression rules.
    pub fn encoder_for<'w, T>(&self) -> Option<DynEncoder<'w, T>>
    where
        Self: Sized,
        T: ?Sized + Write + 'w,
    {
        match (self.byte_order, self.explicit_vr) {
            (Endianness::Little, false) => Some(Box::new(EncoderFor::new(
                ImplicitVRLittleEndianEncoder::default(),
            ))),
            (Endianness::Little, true) => Some(Box::new(EncoderFor::new(
                ExplicitVRLittleEndianEncoder::default(),
            ))),
            (Endianness::Big, true) => Some(Box::new(EncoderFor::new(
                ExplicitVRBigEndianEncoder::default(),
            ))),
            _ => None,
        }
    }

    /// Obtain a dynamic basic decoder, based on this transfer syntax' expected endianness.
    pub fn basic_decoder(&self) -> BasicDecoder {
        BasicDecoder::from(self.endianness())
    }

    /// Type-erase the pixel data or data set codec.
    pub fn erased(self) -> TransferSyntax
    where
        D: Send + Sync + 'static,
        D: DataRWAdapter,
        R: Send + Sync + 'static,
        R: PixelDataReader,
        W: Send + Sync + 'static,
        W: PixelDataWriter,
    {
        let codec = match self.codec {
            Codec::Dataset(d) => Codec::Dataset(d.map(|d| Box::new(d) as _)),
            Codec::EncapsulatedPixelData(r, w) => Codec::EncapsulatedPixelData(
                r.map(|r| Box::new(r) as _),
                w.map(|w| Box::new(w) as _),
            ),
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
