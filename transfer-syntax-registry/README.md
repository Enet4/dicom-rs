# DICOM-rs `transfer-syntax-registry`

[![CratesIO](https://img.shields.io/crates/v/dicom-transfer-syntax-registry.svg)](https://crates.io/crates/dicom-transfer-syntax-registry)
[![Documentation](https://docs.rs/dicom-transfer-syntax-registry/badge.svg)](https://docs.rs/dicom-transfer-syntax-registry)

This sub-project implements a registry of DICOM transfer syntaxes,
which can be optionally extended.

An implementation based on [`inventory`] can be used through the Cargo feature
`inventory-registry`. `inventory` allows for users to register new transfer
syntax implementations in a compile time plugin-like fashion,
but not all environments support it (such as WebAssembly).

[`inventory`]: https://crates.io/crates/inventory
