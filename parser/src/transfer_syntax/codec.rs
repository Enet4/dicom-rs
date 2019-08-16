//! This module contains the concept of a DICOM codec registry.
//! A codec registry maps a DICOM UID of a transfer syntax into the respective
//! transfer syntax' encoding and decoding component.
//! 

use std::collections::HashMap;
use std::fmt;
use crate::transfer_syntax;
use crate::transfer_syntax::{TransferSyntax, Codec};
use lazy_static::lazy_static;

type DynTransferSyntax<'ts> = Box<(dyn Codec + Send + Sync + 'ts)>;
type DynTransferSyntaxRef<'ts> = &'ts (dyn Codec + Send + Sync);

/// Data type for a registry of DICOM codecs.
pub struct CodecRegistry<'ts> {
    m: HashMap<&'static str, DynTransferSyntax<'ts>>,
}

impl<'ts> fmt::Debug for CodecRegistry<'ts> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entries: HashMap<&str, &str> = self.m.iter()
            .map(|(uid, ts)| (*uid, ts.name()))
            .collect();
        f.debug_struct("CodecRegistry")
            .field("m", &entries)
            .finish()
    }
}

impl<'ts> CodecRegistry<'ts> {
    /// Obtain a DICOM codec by transfer syntax UID.
    pub fn get<U: AsRef<str>>(&'ts self, uid: U) -> Option<DynTransferSyntaxRef<'ts>> {
        let ts_uid = {
            let uid = uid.as_ref();
            if uid.chars().rev().next() == Some('\0') {
                &uid[..uid.len() - 1]
            } else {
                &uid
            }
        };
        self.m.get(ts_uid).map(|b| b.as_ref())
    }
}

lazy_static! {
    static ref REGISTRY: CodecRegistry<'static> = {
        CodecRegistry { m: initialize_codecs() }
    };
}

/// Retrieve the global codec registry.
pub fn get_registry() -> &'static CodecRegistry<'static> {
    &REGISTRY
}

fn initialize_codecs() -> HashMap<&'static str, DynTransferSyntax<'static>> {
    let mut m = HashMap::<&'static str, DynTransferSyntax<'static>>::new();

    let ts = transfer_syntax::ExplicitVRLittleEndian;
    m.insert(ts.uid(), Box::from(ts));
    let ts = transfer_syntax::ImplicitVRLittleEndian;
    m.insert(ts.uid(), Box::from(ts));
    let ts = transfer_syntax::ExplicitVRBigEndian;
    m.insert(ts.uid(), Box::from(ts));
    let ts = transfer_syntax::DeflatedExplicitVRLittleEndian;
    m.insert(ts.uid(), Box::from(ts));
    let ts = transfer_syntax::JPEGBaseline;
    m.insert(ts.uid(), Box::from(ts));

    m
}
