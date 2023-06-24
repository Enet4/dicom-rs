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

Usage: dicom-dictionary-builder <COMMAND>

Commands:
  data-element  Fetch and build a dictionary of DICOM data elements (tags)
  uids          Fetch and build a dictionary of DICOM unique identifiers
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

After specifying which dictionary is intended,
the next argument is usually its source,
which can be either a file or a hyperlink.

Fetching a data element (tags) dictionary:

```text
Fetch and build a dictionary of DICOM data elements (tags)

Usage: dicom-dictionary-builder data-element [OPTIONS] [FROM]

Arguments:
  [FROM]  Path or URL to the data element dictionary [default: https://raw.githubusercontent.com/DCMTK/dcmtk/master/dcmdata/data/dicom.dic]

Options:
  -o <OUTPUT>              The output file [default: tags.rs]
      --ignore-retired     Ignore retired DICOM tags
      --deprecate-retired  Mark retired DICOM tags as deprecated
  -h, --help               Print help
```

Fetching a UID dictionary:

```text
Usage: dicom-dictionary-builder uids [OPTIONS] [FROM]

Arguments:
  [FROM]  Path or URL to the XML file containing the UID values tables [default: https://dicom.nema.org/medical/dicom/current/source/docbook/part06/part06.xml]

Options:
  -o <OUTPUT>              The output file [default: uids.rs]
      --ignore-retired     Ignore retired UIDs
      --deprecate-retired  Mark retired UIDs as deprecated
      --feature-gate       Whether to gate different UID types on Cargo features
  -h, --help               Print help
```

**Note:** If retrieving part06.xml from the official DICOM server
fails due to the TLS connection not initializing,
try downloading the file with another software
and passing the path to the file manually.
