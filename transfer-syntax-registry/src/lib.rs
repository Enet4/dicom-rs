//! This crate contains the DICOM transfer syntax registry.
//! The transfer syntax registry maps a DICOM UID of a transfer syntax into the
//! respective transfer syntax specifier.

mod entries;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use byteordered::Endianness;
use dicom_encoding::transfer_syntax::{Codec, TransferSyntax, AdapterFreeTransferSyntax};
use lazy_static::lazy_static;

/// Data type for a registry of DICOM.
pub struct TransferSyntaxRegistry {
    m: HashMap<&'static str, &'static TransferSyntax>,
}

impl fmt::Debug for TransferSyntaxRegistry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entries: HashMap<&str, &str> = self.m.iter()
            .map(|(uid, ts)| (*uid, ts.name()))
            .collect();
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
            if uid.chars().rev().next() == Some('\0') {
                &uid[..uid.len() - 1]
            } else {
                &uid
            }
        };
        self.m.get(ts_uid).map(|ts| *ts)
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
                    (Codec::Unsupported, Codec::Dataset(_)) |
                    (Codec::EncapsulatedPixelData, Codec::PixelData(_)) => {
                        true
                    },
                    // weird one ahead: the two specifiers do not agree on
                    // requirements, better keep it as a separate match arm for
                    // debugging purposes
                    (Codec::Unsupported, Codec::PixelData(_)) => {
                        eprintln!("Inconsistent requirements for transfer syntax {}: `Unsupported` cannot be replaced with `PixelData`", ts.uid());
                        false
                    },
                    // ignoring TS with less or equal implementation 
                    _ => false,
                };

                if replace {
                    e.insert(ts);
                    true
                } else {
                    false
                }
            },
            Entry::Vacant(e) => {
                e.insert(ts);
                true
            },
        }
    }
}

lazy_static! {
    static ref REGISTRY: TransferSyntaxRegistry = {
        let mut registry = TransferSyntaxRegistry { m: HashMap::with_capacity(32) };

        for ts in inventory::iter::<TransferSyntax> {
            registry.register(ts);
        }

        registry
    };
}

/// Retrieve the default transfer syntax.
pub fn default() -> AdapterFreeTransferSyntax {
    entries::IMPLICIT_VR_LITTLE_ENDIAN
}

/// Retrieve the global codec registry.
pub fn get_registry() -> &'static TransferSyntaxRegistry {
    &REGISTRY
}

/// create a TS with an unsupported pixel encapsulation
pub(crate) const fn create_ts_stub(uid: &'static str, name: &'static str) -> AdapterFreeTransferSyntax {
    TransferSyntax::new(
        uid,
        name,
        Endianness::Little,
        true,
        Codec::EncapsulatedPixelData,
    )
}

#[cfg(test)]
mod tests {
    use super::{get_registry, TransferSyntaxRegistry};

    fn assert_fully_supported(registry: &TransferSyntaxRegistry, mut uid: &'static str) {
        let ts = registry.get(uid);
        assert!(ts.is_some());
        let ts = ts.unwrap();
        if uid.ends_with("\0") {
            uid = &uid[0..uid.len() - 1];
        }
        assert_eq!(ts.uid(), uid);
        assert!(ts.fully_supported());
    }

    #[test]
    fn contains_base_ts() {
        let registry = get_registry();

        // contains implicit VR little endian and is fully supported
        assert_fully_supported(&registry, "1.2.840.10008.1.2");

        // should work the same for trailing null characters
        assert_fully_supported(&registry, "1.2.840.10008.1.2\0");

        // contains explicit VR little endian and is fully supported
        assert_fully_supported(&registry, "1.2.840.10008.1.2.1");

        // contains explicit VR big endian and is fully supported
        assert_fully_supported(&registry, "1.2.840.10008.1.2.2");
    }
}
