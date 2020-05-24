# DICOM-rs `dictionary-builder`

[![CratesIO](https://img.shields.io/crates/v/dicom-dictionary-builder.svg)](https://crates.io/crates/dicom-dictionary-builder)
[![Documentation](https://docs.rs/dicom-dictionary-builder/badge.svg)](https://docs.rs/dicom-dictionary-builder)

This sub-project is a tool for generating machine readable attribute dictionaries from the DICOM standard.

## Building

```bash
cargo build --release
```

## Usage

```text
    dictionary-builder [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f <FORMAT>        The output format [values: rs, json]
    -o <OUTPUT>        The path to the output file
```
