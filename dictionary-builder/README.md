# DICOM-rs `dictionary-builder`

[![CratesIO](https://img.shields.io/crates/v/dicom-dictionary-builder.svg)](https://crates.io/crates/dicom-dictionary-builder)
[![Documentation](https://docs.rs/dicom-dictionary-builder/badge.svg)](https://docs.rs/dicom-dictionary-builder)

This sub-project is a tool for generating machine readable attribute dictionaries from the DICOM standard.
At the moment, the tool is capable of parsing .dic files from the DCMTK project.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Building

```bash
cargo build --release
```

## Usage

```text
DICOM Dictionary Builder 0.1.0

USAGE:
    dicom-dictionary-builder [FLAGS] [OPTIONS] [FROM]

FLAGS:
    -h, --help          Prints help information
        --no-retired    Whether to ignore retired tags
    -V, --version       Prints version information

OPTIONS:
    -o <OUTPUT>        The path to the output file [default: tags.rs]

ARGS:
    <FROM>    Where to fetch the dictionary from [default:
              https://raw.githubusercontent.com/DCMTK/dcmtk/master/dcmdata/data/dicom.dic]
```
