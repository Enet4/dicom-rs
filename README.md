# DICOM-rs

[![Build Status](https://travis-ci.org/Enet4/dicom-rs.svg?branch=master)](https://travis-ci.org/Enet4/dicom-rs) [![dependency status](https://deps.rs/repo/github/Enet4/dicom-rs/status.svg)](https://deps.rs/repo/github/Enet4/dicom-rs)


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
- [`dictionary_builder`](dictionary_builder) is a Rust application that generates code and
  other data structures for a DICOM standard dictionary using entries from the official website.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
