# DICOM-rs

[![CratesIO](https://img.shields.io/crates/v/dicom.svg)](https://crates.io/crates/dicom)
[![Documentation](https://docs.rs/dicom/badge.svg)](https://docs.rs/dicom)

`dicom` is a library for the [DICOM] standard.
It is part of  the [DICOM-rs] project,
an ecosystem of modules and tools for DICOM compliant systems.

This collection provides a pure Rust implementation of the DICOM standard,
allowing users to read and write DICOM data over files and other sources,
while remaining intrinsically efficient, fast, intuitive, and safe to use.

## Using as a library

This crate exposes the [`dicom-object`] crate directly via the `object` module,
which has a high-level API for reading, writing, and manipulating DICOM objects.
Other key components of the full library are available in this one as well,
albeit representing different levels of abstraction.

An example of use follows.
For more details, please visit the [`dicom-object` documentation]
or the [full library documentation].

```rust
use dicom::core::Tag;
use dicom::object::{open_file, Result};

let obj = open_file("0001.dcm")?;
let patient_name = obj.element_by_name("PatientName")?.to_str()?;
let modality = obj.element_by_name("Modality")?.to_str()?;
let pixel_data_bytes = obj.element(Tag(0x7FE0, 0x0010))?.to_bytes()?;
```

### Cargo features

This crate enables the inventory-based transfer syntax registry by default,
which allows for a seamless integration of additional transfer syntaxes
without changing the application.
In environments which do not support this, the feature can be disabled.
Please see the documentation of [`dicom-transfer-syntax-registry`]
for more information.

The following root modules are behind Cargo features enabled by default:

- [`ul`]: the DICOM upper layer protocol library
- [`pixeldata`]: the pixel data abstraction library

If you do not intend to use these modules,
you can disable these features accordingly.

[DICOM]: https://dicomstandard.org
[DICOM-rs]: https://github.com/Enet4/dicom-rs
[`dicom-transfer-syntax-registry`]: https://docs.rs/dicom-transfer-syntax-registry
[`dicom-object`]: https://crates.io/crates/dicom-object
[`dicom-object` documentation]: https://docs.rs/dicom-object
[`ul`]: https://crates.io/crates/dicom-ul
[`pixeldata`]: https://crates.io/crates/dicom-pixeldata
[full library documentation]: https://docs.rs/dicom
