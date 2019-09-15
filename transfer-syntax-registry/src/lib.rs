//! This crate contains the DICOM transfer syntax registry.
//! The transfer syntax registry maps a DICOM UID of a transfer syntax into the
//! respective transfer syntax specifier.
//!
//! This registry should not have to be used directly, except when developing
//! higher level APIs, which should learn to negotiate and resolve the expected
//! transfer syntax automatically.

use byteordered::Endianness;
use dicom_encoding::submit_transfer_syntax;
use dicom_encoding::transfer_syntax::{AdapterFreeTransferSyntax as Ts, Codec};
use lazy_static::lazy_static;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

pub use dicom_encoding::TransferSyntax;

/// Data type for a registry of DICOM.
pub struct TransferSyntaxRegistry {
    m: HashMap<&'static str, &'static TransferSyntax>,
}

impl fmt::Debug for TransferSyntaxRegistry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entries: HashMap<&str, &str> =
            self.m.iter().map(|(uid, ts)| (*uid, ts.name())).collect();
        f.debug_struct("TransferSyntaxRegistry")
            .field("m", &entries)
            .finish()
    }
}

impl TransferSyntaxRegistry {
    /// Obtain a DICOM codec by transfer syntax UID.
    pub fn get<U: AsRef<str>>(&self, uid: U) -> Option<&'static TransferSyntax> {
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

lazy_static! {
    static ref REGISTRY: TransferSyntaxRegistry = {
        let mut registry = TransferSyntaxRegistry {
            m: HashMap::with_capacity(32),
        };

        for ts in inventory::iter::<TransferSyntax> {
            registry.register(ts);
        }

        registry
    };
}

/// Retrieve the global codec registry.
pub fn get_registry() -> &'static TransferSyntaxRegistry {
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
