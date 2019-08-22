//! This module contains the DICOM transfer syntax registry.
//! The transfer syntax registry maps a DICOM UID of a transfer syntax into the
//! respective transfer syntax specifier.

use std::collections::HashMap;
use std::fmt;
use crate::transfer_syntax;
use crate::transfer_syntax::TransferSyntax;
use lazy_static::lazy_static;


/// Data type for a registry of DICOM.
pub struct TransferSyntaxRegistry {
    m: HashMap<&'static str, TransferSyntax>,
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
    pub fn get<U: AsRef<str>>(&self, uid: U) -> Option<&TransferSyntax> {
        let ts_uid = {
            let uid = uid.as_ref();
            if uid.chars().rev().next() == Some('\0') {
                &uid[..uid.len() - 1]
            } else {
                &uid
            }
        };
        self.m.get(ts_uid)
    }
}

lazy_static! {
    static ref REGISTRY: TransferSyntaxRegistry = {
        TransferSyntaxRegistry { m: initialize_codecs() }
    };
}

/// Retrieve the global codec registry.
pub fn get_registry() -> &'static TransferSyntaxRegistry {
    &REGISTRY
}

fn initialize_codecs() -> HashMap<&'static str, TransferSyntax> {
    let mut m = HashMap::<&'static str, TransferSyntax>::new();

    let ts = transfer_syntax::EXPLICIT_VR_LITTLE_ENDIAN;
    m.insert(ts.uid(), ts.erased());
    let ts = transfer_syntax::IMPLICIT_VR_LITTLE_ENDIAN;
    m.insert(ts.uid(), ts.erased());
    let ts = transfer_syntax::EXPLICIT_VR_BIG_ENDIAN;
    m.insert(ts.uid(), ts.erased());

    m
}
