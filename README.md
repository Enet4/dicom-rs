# DICOM-rs

[![DICOM-rs on crates.io](https://img.shields.io/crates/v/dicom.svg)](https://crates.io/crates/dicom)
[![continuous integration](https://github.com/Enet4/dicom-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/Enet4/dicom-rs/actions/workflows/rust.yml)
![Minimum Rust Version Stable](https://img.shields.io/badge/Minimum%20Rust%20Version-stable-green.svg)
[![dependency status](https://deps.rs/repo/github/Enet4/dicom-rs/status.svg)](https://deps.rs/repo/github/Enet4/dicom-rs)
[![Gitter](https://badges.gitter.im/dicom-rs/community.svg)](https://gitter.im/dicom-rs/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)
[![Documentation](https://docs.rs/dicom/badge.svg)](https://docs.rs/dicom)

An ecosystem of library and tools for [DICOM](https://dicomstandard.org) compliant systems.

This collection provides a pure Rust implementation of the DICOM standard,
allowing users to read and write DICOM objects over files and other sources, while
remaining intrinsically fast and safe to use.

## Components

### Library:

- [`core`](core) represents all of the base traits, data structures and functions related to DICOM content.
- [`encoding`](encoding) contains DICOM data encoding and decoding primitives.
- [`parser`](parser) provides a middle-level abstraction for the parsing and printing of DICOM data sets.
- [`object`](object) provides a high-level abstraction of DICOM objects and functions for reading and writing DICOM files.
- [`pixeldata`](pixeldata) enables the conversion of DICOM objects into images and multidimensional arrays.
- [`dictionary-std`](dictionary-std) contains a Rust definition of the standard data dictionary.
- [`transfer-syntax-registry`](transfer-syntax-registry) contains a registry of transfer syntax specifications.
- [`ul`](ul) implements the DICOM upper layer protocol.

### Tools:

- [`dump`](dump) is a command-line application for inspecting DICOM files.
- [`scpproxy`](scpproxy) implements the Proxy service class provider.
- [`echoscu`](echoscu) implements a Verification service class user.
- [`storescu`](storescu) implements a Storage service class user.

### Development tools:

- [`dictionary-builder`](dictionary-builder) is a Rust application that generates code and
  other data structures for a DICOM standard dictionary.

## Using as a library

The parent crate [`dicom`](parent) aggregates the key components of the full library.
[`dicom::object`](object) is currently the most usable crate for reading DICOM objects from a file or a similar source.
As an alternative, [`dicom_object`](object) can be used directly.

An example of use follows. For more details, please visit the [`dicom-object` documentation](https://docs.rs/dicom-object).

```rust
use dicom::object::open_file;

let obj = open_file("0001.dcm")?;
let patient_name = obj.element_by_name("PatientName")?.to_str()?;
let modality = obj.element_by_name("Modality")?.to_str()?;
```

## Building

You can use Cargo to build all crates in the repository.

```sh
cargo build
```

## Roadmap & Contributing

This project is under active development.

Your feedback during the development of these solutions is welcome. Please see the [wiki](https://github.com/Enet4/dicom-rs/wiki)
for additional guidelines related to the project's roadmap.
See also the [contributor guidelines](CONTRIBUTING.md) and the project's [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
