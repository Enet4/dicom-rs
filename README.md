# DICOM-rs

An efficient and practical base library for DICOM compliant systems.

At its core, this library is a pure Rust implementation of the DICOM representation format,
allowing users to read/write DICOM objects from/to files and other sources/destinations,
while remaining intrinsically fast and safe to use.

This project is a WIP. No crate has been published online to respect the fact that
the project does not yet meet the minimum features required to be usable.

## Components

 - "core" represents all of the base traits, data structures and functions for
 reading and writing DICOM content.
 - "dictionary_builder" is a Rust application that generates Rust code for
   a DICOM standard dictionary using entries from the web.

## Building

Enter the "core" directory and run:

```bash
cargo build --release
```
