# DICOM-rs `dcmdump`

[![CratesIO](https://img.shields.io/crates/v/dcmdump.svg)](https://crates.io/crates/dcmdump)
[![Documentation](https://docs.rs/dcmdump/badge.svg)](https://docs.rs/dcmdump)

**Warning:** This tool is deprecated in favor of [`dicom-dump`](../dump).

A command line utility for inspecting DICOM files.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
    dcmdump [FLAGS] [OPTIONS] <file>

FLAGS:
    -h, --help             Prints help information
        --no-text-limit    whether text value width limit is disabled (limited to `width` by default)
    -V, --version          Prints version information

OPTIONS:
    -w, --width <width>    the width of the display (default is to check automatically)

ARGS:
    <file>    The DICOM file to read
```
