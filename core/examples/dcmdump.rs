//! A reimplementation of dcmdump in Rust
//! WIP
extern crate dicom_core;

use dicom_core::{DicomElement, DicomObject};
use std::fs::File;
use std::path::Path;

fn main() {
    println!("Sorry, this tool is not implemented yet.");
}

fn dump<P: AsRef<Path>>(path: P) {
    let file = File::open(path).unwrap();
    //let obj = DicomLoader.load(file).unwrap();

    //for elem in obj {
    //    dump_element(&elem);
    //}
}

fn dump_element(elem: &DicomElement) {}
