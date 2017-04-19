#![allow(unsafe_code)]
//! This module contains the concept of a DICOM codec registry.
//!

extern crate lazy_static;

use std::collections::HashMap;
use transfer_syntax;
use transfer_syntax::TransferSyntax;

type DynTransferSyntax<'ts> = Box<(TransferSyntax + Send + 'ts)>;

/// Data type for a registry of DICOM codecs.
#[derive(Debug)]
pub struct CodecRegistry<'ts> {
    m: HashMap<&'static str, DynTransferSyntax<'ts>>,
}

impl<'ts> CodecRegistry<'ts> {
    /// Obtain a DICOM codec by transfer syntax UID.
    pub fn get<UID: AsRef<str>>(&'ts self, uid: UID) -> Option<&'ts (TransferSyntax + Send + 'ts)> {
        self.m.get(uid.as_ref()).map(|b| b.as_ref())
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

//pub fn register<T: TransferSyntax + 'static>(ts: T) {
//    REGISTRY.register(ts);
//}


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
