# DICOM-rs `fromimage`

[![CratesIO](https://img.shields.io/crates/v/dicom-fromimage.svg)](https://crates.io/crates/dicom-fromimage)
[![Documentation](https://docs.rs/dicom-fromimage/badge.svg)](https://docs.rs/dicom-fromimage)

This command line tool takes a base DICOM file of the image module
and replaces the various DICOM attributes with those of another file.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
dicom-fromimage 0.1.0
Convert and replace a DICOM file's image with another image

USAGE:
    dicom-fromimage.exe [FLAGS] [OPTIONS] <dcm-file> <img-file>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Print more information about the image and the output file

OPTIONS:
    -o, --out <output>    Path to the output image (default is to replace input extension with `.new.dcm`)

ARGS:
    <dcm-file>    Path to the base DICOM file to read
    <img-file>    Path to the image file to replace the DICOM file
```
