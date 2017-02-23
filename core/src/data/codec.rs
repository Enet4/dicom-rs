#![allow(unsafe_code)]
//! This module contains the concept of a DICOM codec registry.
//!

extern crate lazy_static;

use std::sync::Mutex;
use std::ops::Deref;
use std::collections::HashMap;
use transfer_syntax::TransferSyntax;

/// Data type for a registry of DICOM codecs.
#[derive(Debug)]
pub struct CodecRegistry<'ts> {
    m: Mutex<HashMap<&'static str, Box<(TransferSyntax + Send + 'ts)>>>,
}

impl<'ts> CodecRegistry<'ts> {

    /// Obtain a DICOM codec by transfer syntax UID.
    pub fn get<UID: Deref<Target = str>>(&'ts self, uid: UID) -> Option<&'ts (TransferSyntax + Send + 'ts)> {
        unimplemented!()
    }

    /// Register a DICOM codec.
    fn register<T: TransferSyntax + Send + 'ts>(&mut self, ts: T) {
        self.m.get_mut().unwrap().insert(ts.uid(), Box::new(ts));
    }
}

lazy_static! {
    static ref REGISTRY: CodecRegistry<'static> = {
        CodecRegistry { m: Mutex::new(HashMap::new()) }
    };
}

/// Retrieve the global codec registry.
pub fn get_registry() -> &'static CodecRegistry<'static> {
    &REGISTRY
}

//pub fn register<T: TransferSyntax + 'static>(ts: T) {
//    REGISTRY.register(ts);
//}
