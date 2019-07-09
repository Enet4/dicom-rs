# DICOM-rs

[![Build Status](https://travis-ci.org/Enet4/dicom-rs.svg?branch=master)](https://travis-ci.org/Enet4/dicom-rs) ![Minimum Rust Version Stable](https://img.shields.io/badge/Minimum%20Rust%20Version-stable-green.svg) [![dependency status](https://deps.rs/repo/github/Enet4/dicom-rs/status.svg)](https://deps.rs/repo/github/Enet4/dicom-rs) [![Gitter](https://badges.gitter.im/dicom-rs/community.svg)](https://gitter.im/dicom-rs/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)


An efficient and practical library for DICOM compliant systems.

This collection provides a pure Rust implementation of the DICOM standard,
allowing users to read and write DICOM objects over files and other sources, while
remaining intrinsically fast and safe to use.

This project is a work in progress. Nevertheless, the first usable version may hopefully arrive
in the next few months. Any feedback during the development of these solutions is welcome.

## Components

- [`core`](core) represents all of the base traits, data structures and functions related to DICOM content.
- [`parser`](parser) contains DICOM data encoding and decoding constructs, as well as a parser of DICOM data sets.
- [`object`](object) provides a high-level abstraction of DICOM objects and functions for reading and writing DICOM files.
- [`dictionary-std`](dictionary-std) contains a Rust definition of the standard data dictionary.
- [`dictionary-builder`](dictionary-builder) is a Rust application that generates code and
  other data structures for a DICOM standard dictionary using entries from the official website.

## Using as a library

[`dicom-object`](object) is currently the most usable crate for reading DICOM objects. An example of use follows. For more, please visit the respective documentation.

```rust
use dicom_object::open_file;
use dicom_object::Result;

let obj = open_file("0001.dcm")?;
let patient_name = obj.element_by_name("PatientName")?.to_str()?;
let modality = obj.element_by_name("Modality")?.to_str()?;
```

## Building

You can use Cargo to build all crates in the repository.

```sh
cargo build --release
```

## Roadmap

Although no particular timeline is expected, and the list of features to be included here is uncertain, my intent is to publish the first version of this crate when the DICOM object abstraction becomes sufficiently functional. That is, the user should be capable of:

 - Reading DICOM objects from files in one of these transfer syntaxes: _ExplicitVRLittleEndian_, _ImplicitVRLittleEndian_ and _ExplicitVRBigEndian_;
 - Creating and writing DICOM objects;
 - Fetching an object's pixel data as an n-dimensional array;

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
