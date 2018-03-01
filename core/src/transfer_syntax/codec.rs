#![allow(unsafe_code)]
//! This module contains the concept of a DICOM codec registry.
//! A codec registry maps a DICOM UID of a transfer syntax
//! into the respective transfer syntax object.
//! 

extern crate lazy_static;

use std::collections::HashMap;
use std::fmt;
use transfer_syntax;
use transfer_syntax::TransferSyntax;

type DynTransferSyntax<'ts> = Box<(TransferSyntax + Send + Sync + 'ts)>;
type DynTransferSyntaxRef<'ts> = &'ts (TransferSyntax + Send + Sync);

/// Data type for a registry of DICOM codecs.
pub struct CodecRegistry<'ts> {
    m: HashMap<&'static str, DynTransferSyntax<'ts>>,
}

impl<'ts> fmt::Debug for CodecRegistry<'ts> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let keys: Vec<_> = self.m.keys().map(|x| *x).collect();
        f.debug_struct("CodecRegistry")
            .field("m", &format!("{:?}", keys))
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
