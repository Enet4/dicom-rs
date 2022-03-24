# DICOM-rs `dump`

[![CratesIO](https://img.shields.io/crates/v/dicom-dump.svg)](https://crates.io/crates/dicom-dump)
[![Documentation](https://docs.rs/dicom-dump/badge.svg)](https://docs.rs/dicom-dump)

A command line utility for inspecting the contents of DICOM files
by printing them in a human readable format.

A programmatic API for dumping DICOM objects is also available.
If you intend to use `dicom-dump` exclusively as a library,
you can disable the `cli` Cargo feature.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
    dicom-dump [FLAGS] [OPTIONS] <file>

FLAGS:
    -h, --help             Prints help information
        --no-text-limit    whether text value width limit is disabled (limited to `width` by default)
    -V, --version          Prints version information

OPTIONS:
        --color <color>    color mode [default: auto]
    -w, --width <width>    the width of the display (default is to check automatically)

ARGS:
    <file>    The DICOM file to read
```
