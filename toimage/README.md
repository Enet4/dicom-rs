# DICOM-rs `toimage`

[![CratesIO](https://img.shields.io/crates/v/dicom-toimage.svg)](https://crates.io/crates/dicom-toimage)
[![Documentation](https://docs.rs/dicom-toimage/badge.svg)](https://docs.rs/dicom-toimage)

A command line utility for converting DICOM image files
into general purpose image files (e.g. PNG).

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
    dicom-toimage [FLAGS] [OPTIONS] <file>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Print more information about the image and the output file

OPTIONS:
    -F, --frame <frame-number>    Frame number (0-indexed) [default: 0]
    -o, --out <output>            Path to the output image (default is to replace input extension with `.png`)

ARGS:
    <file>    Path to the DICOM file to read
```
