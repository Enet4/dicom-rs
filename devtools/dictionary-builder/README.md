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
    sop                Fetch and build a dictionary of DICOM SOP classes
    help               Print this message or the help of the given subcommand(s)
```

After specifying which dictionary is intended,
the next argument is usually its source,
which can be either a file or a hyperlink.

Fetching a data element (tags) dictionary:

```text
USAGE:
    dicom-dictionary-builder data-element [OPTIONS] [FROM]

ARGS:
    <FROM>    Path or URL to the data element dictionary [default:
              https://raw.githubusercontent.com/DCMTK/dcmtk/master/dcmdata/data/dicom.dic]

OPTIONS:
        --deprecate-retired    Mark retired DICOM tags as deprecated
    -h, --help                 Print help information
        --ignore-retired       Ignore retired DICOM tags
    -o <OUTPUT>                The output file [default: tags.rs]
```

Fetching an SOP class dictionary:

```text
USAGE:
    dicom-dictionary-builder sop [OPTIONS] [FROM]

ARGS:
    <FROM>    Path or URL to the SOP class dictionary from [default:
              https://dicom.nema.org/medical/dicom/current/source/docbook/part06/part06.xml]

OPTIONS:
        --deprecate-retired    Mark retired SOP classes as deprecated
    -h, --help                 Print help information
        --ignore-retired       Ignore retired SOP classes
    -o <OUTPUT>                The output file [default: sop.rs]
```

**Note:** If retrieving part06.xml from the official DICOM server
fails due to the TLS connection not initializing,
try downloading the file with another software
and passing the path to the file manually.
