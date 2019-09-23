//! This crate contains the DICOM transfer syntax registry.
//! The transfer syntax registry maps a DICOM UID of a transfer syntax into the
//! respective transfer syntax specifier. The container of transfer syntaxes is
//! populated before-main through the [inventory] pattern, which makes it
//! readily available through the [`TransferSyntaxRegistry`] type.
//!
//! This registry should not have to be used directly, except when developing
//! higher level APIs, which should learn to negotiate and resolve the expected
//! transfer syntax automatically.
//!
//! [inventory]: https://docs.rs/inventory/0.1.4/inventory

use byteordered::Endianness;
use dicom_encoding::submit_transfer_syntax;
use dicom_encoding::transfer_syntax::{
    AdapterFreeTransferSyntax as Ts, Codec, TransferSyntaxIndex,
};
use lazy_static::lazy_static;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

pub use dicom_encoding::TransferSyntax;

/// Data type for a registry of DICOM.
pub struct TransferSyntaxRegistryImpl {
    m: HashMap<&'static str, &'static TransferSyntax>,
}

impl fmt::Debug for TransferSyntaxRegistryImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entries: HashMap<&str, &str> =
            self.m.iter().map(|(uid, ts)| (*uid, ts.name())).collect();
        f.debug_struct("TransferSyntaxRegistryImpl")
            .field("m", &entries)
            .finish()
    }
}

impl TransferSyntaxRegistryImpl {
    /// Obtain a DICOM codec by transfer syntax UID.
    fn get<U: AsRef<str>>(&self, uid: U) -> Option<&'static TransferSyntax> {
        let ts_uid = {
            let uid = uid.as_ref();
            if uid.as_bytes().last().cloned() == Some(b'\0') {
                &uid[..uid.len() - 1]
            } else {
                &uid
            }
        };
        self.m.get(ts_uid).copied()
    }

    /// Register the given transfer syntax (TS) to the system. It can override
    /// another TS with the same UID, in the only case that the TS requires
    /// certain codecs which are not supported by the previously registered
    /// TS. If no such requirements are imposed, this function returns `false`
    /// and no changes are made.
    fn register(&mut self, ts: &'static TransferSyntax) -> bool {
        match self.m.entry(&ts.uid()) {
            Entry::Occupied(mut e) => {
                let replace = match (&e.get().codec(), ts.codec()) {
                    (Codec::Unsupported, Codec::Dataset(_))
                    | (Codec::EncapsulatedPixelData, Codec::PixelData(_)) => true,
                    // weird one ahead: the two specifiers do not agree on
                    // requirements, better keep it as a separate match arm for
                    // debugging purposes
                    (Codec::Unsupported, Codec::PixelData(_)) => {
                        eprintln!("Inconsistent requirements for transfer syntax {}: `Unsupported` cannot be replaced with `PixelData`", ts.uid());
                        false
                    }
                    // ignoring TS with less or equal implementation
                    _ => false,
                };

                if replace {
                    e.insert(ts);
                    true
                } else {
                    false
                }
            }
            Entry::Vacant(e) => {
                e.insert(ts);
                true
            }
        }
    }
}

impl TransferSyntaxIndex for TransferSyntaxRegistryImpl {
    fn get(&self, uid: &str) -> Option<&TransferSyntax> {
        Self::get(self, uid)
    }
}

/// Zero-sized representative of the main transfer syntax registry.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub struct TransferSyntaxRegistry;

impl TransferSyntaxIndex for TransferSyntaxRegistry {
    fn get(&self, uid: &str) -> Option<&TransferSyntax> {
        get_registry().get(uid)
    }
}

lazy_static! {
    static ref REGISTRY: TransferSyntaxRegistryImpl = {
        let mut registry = TransferSyntaxRegistryImpl {
            m: HashMap::with_capacity(32),
        };

        for ts in inventory::iter::<TransferSyntax> {
            registry.register(ts);
        }

        registry
    };
}

/// Retrieve a reference to the global codec registry.
pub(crate) fn get_registry() -> &'static TransferSyntaxRegistryImpl {
    &REGISTRY
}

/// create a TS with an unsupported pixel encapsulation
pub(crate) const fn create_ts_stub(uid: &'static str, name: &'static str) -> Ts {
    TransferSyntax::new(
        uid,
        name,
        Endianness::Little,
        true,
        Codec::EncapsulatedPixelData,
    )
}

/// Retrieve the default transfer syntax.
pub fn default() -> Ts {
    IMPLICIT_VR_LITTLE_ENDIAN
}

// included verbatim instead of placed in a module because inventory
// value submission only works at the crate's root at the moment.
include!("entries.rs");
