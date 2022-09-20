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
DICOM dictionary builder

USAGE:
    dicom-dictionary-builder <SUBCOMMAND>

OPTIONS:
    -h, --help          Print help information

SUBCOMMANDS:
    data-element       Fetch and build a dictionary of DICOM data elements (tags)
    help               Print this message or the help of the given subcommand(s)
```

Fetching a data (tags) dictionary:

```text
USAGE:
    dicom-dictionary-builder data-element [OPTIONS] [FROM]

ARGS:
    <FROM>    Where to fetch the data element dictionary from [default:
              https://raw.githubusercontent.com/DCMTK/dcmtk/master/dcmdata/data/dicom.dic]

OPTIONS:
        --deprecate_retired    Mark retired DICOM tags as deprecated
    -h, --help                 Print help information
        --ignore_retired       Ignore retired DICOM tags
    -o <OUTPUT>                The output file [default: tags.rs]
```
