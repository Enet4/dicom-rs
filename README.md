# DICOM-rs

[![DICOM-rs on crates.io](https://img.shields.io/crates/v/dicom.svg)](https://crates.io/crates/dicom)
[![continuous integration](https://github.com/Enet4/dicom-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/Enet4/dicom-rs/actions/workflows/rust.yml)
![Minimum Rust Version Stable](https://img.shields.io/badge/Minimum%20Rust%20Version-stable-green.svg)
[![dependency status](https://deps.rs/repo/github/Enet4/dicom-rs/status.svg)](https://deps.rs/repo/github/Enet4/dicom-rs)
[![DICOM-rs chat](https://img.shields.io/badge/zulip-join_chat-brightgreen.svg)](https://dicom-rs.zulipchat.com/)
[![Documentation](https://docs.rs/dicom/badge.svg)](https://docs.rs/dicom)

An ecosystem of library and tools for [DICOM](https://dicomstandard.org) compliant systems.

This collection provides a pure Rust implementation of the DICOM standard,
allowing users to work with DICOM objects
and interact with DICOM applications,
while aiming to be fast, safe, and intuitive to use.

## Components

### Library

The following library packages are designed to be used in
other Rust libraries and applications.

- [`object`](object) provides a high-level abstraction of DICOM objects
  and functions for reading and writing DICOM files.
- [`pixeldata`](pixeldata) enables the decoding and conversion of DICOM objects
  into usable imaging data structures,
  such as images and multidimensional arrays.
- [`dump`](dump) provides helpful routines for
  dumping the contents of DICOM objects.
- [`json`](json) provides serialization and deserialization to DICOM JSON.
- [`ul`](ul) implements the DICOM upper layer protocol.
- [`dictionary-std`](dictionary-std) contains a Rust definition of
  the standard data dictionary.
- [`transfer-syntax-registry`](transfer-syntax-registry) contains a registry of
  transfer syntax specifications.
- [`parser`](parser) provides a middle-level abstraction
  for the parsing and printing of DICOM data sets.
- [`encoding`](encoding) contains DICOM data encoding and decoding primitives.
- [`core`](core) represents all of the base traits,
  data structures and functions related to DICOM content.

#### Using as a library

The parent crate [`dicom`](parent) aggregates the key components of the full library,
so it can be added to a project as an alternative to
selectively grabbing the components that you need.

Generally, most projects would add [`dicom_object`](object),
which is the most usable crate for reading DICOM objects from a file or a similar source.
This crate is available in `dicom::object`.
For working with the imaging data of a DICOM object,
add [`pixeldata`](pixeldata).
Network capabilities may be constructed on top of [`ul`](ul).

A simple example of use follows.
For more details,
please visit the [`dicom` documentation](https://docs.rs/dicom).

```rust
use dicom::object::open_file;
use dicom::dictionary_std::tags;

let obj = open_file("0001.dcm")?;
let patient_name = obj.element(tags::PATIENT_NAME)?.to_str()?;
let modality = obj.element(tags::MODALITY)?.to_str()?;
```

### Tools

The project also comprises an assortment of command line tools.

- [`dump`](dump), aside from being a library,
  is also a command-line application for inspecting DICOM files.
- [`scpproxy`](scpproxy) implements a Proxy service class provider.
- [`echoscu`](echoscu) implements a Verification service class user.
- [`findscu`](findscu) implements a Find service class user.
- [`storescu`](storescu) implements a Storage service class user.
- [`storescp`](storescp) implements a Storage service class provider.
- [`toimage`](toimage) lets you convert a DICOM file into an image file.
- [`fromimage`](fromimage) lets you replace the imaging data of a DICOM file
  with one from an image file.
- [`pixeldata`](pixeldata) also includes `dicom-transcode`,
  which lets you transcode DICOM files to other transfer syntaxes.

### Development tools

- [`dictionary-builder`](dictionary-builder) is an independent application that
  generates code and other data structures for a DICOM standard dictionary.

## Building

You can use Cargo to build all crates in the repository.

```sh
cargo build
```

Other than the parts needed to build a pure Rust project,
no other development dependencies are necessary
unless certain extensions are included via Cargo features.
Consult each crate for guidelines on selecting features to suit your needs.

### Minimum Supported Rust version

DICOM-rs currently provides partial MSRV guarantees,
defined by the following rules:

- The minimum supported Rust version required to build _library crates_ with default features is **1.72.0**.
- For any other cases, namely building the tool/binary crates or including extra features,
  only the latest stable toolchain is guaranteed to build.

At of 2025-10-04, version 1.80.0 can build the full project,
though this is not prescriptive and is subject to change over time.

## Roadmap & Contributing

This project is under active development.

Your feedback during the development of these solutions is welcome. Please see the [wiki](https://github.com/Enet4/dicom-rs/wiki)
for additional guidelines related to the project's roadmap.
See also the [contributor guidelines](CONTRIBUTING.md) and the project's [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
