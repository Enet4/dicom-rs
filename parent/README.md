# DICOM-rs

This crate serves as a parent for library crates in the [DICOM-rs] project,
an ecosystem of library and tools for [DICOM] compliant systems.

This collection provides a pure Rust implementation of the DICOM standard,
allowing users to read and write DICOM objects over files and other sources, while
remaining intrinsically fast and safe to use.

## Using as a library

This crate exposes the [`dicom-object`] crate directly via the `object` module,
which is currently the go-to solution for reading DICOM objects
from a file or a similar source.
Other key components of the full library are available as well,
albeit representing different levels of abstraction.

An example of use follows.
For more details, please visit the [`dicom-object` documentation].

```rust
use dicom_object::open_file;
use dicom_object::Result;

let obj = open_file("0001.dcm")?;
let patient_name = obj.element_by_name("PatientName")?.to_str()?;
let modality = obj.element_by_name("Modality")?.to_str()?;
```

### Cargo features

This crate enables the inventory-based transfer syntax registry by default,
which allows for a seamless integration of additional transfer syntaxes
without changing the application.
In environments which do not support this, the feature can be disabled.
Please see the documentation of [`dicom-transfer-syntax-registry`]
for more information.

[DICOM]: https://dicomstandard.org
[DICOM-rs]: https://github.com/Enet4/dicom-rs
[`dicom-transfer-syntax-registry`]: https://docs.rs/dicom-transfer-syntax-registry
[`dicom-object`]: https://crates.io/crates/dicom-object
[`dicom-object` documentation]: https://docs.rs/dicom-object
