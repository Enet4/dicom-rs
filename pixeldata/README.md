# DICOM-rs `pixeldata`

[![CratesIO](https://img.shields.io/crates/v/dicom-pixeldata.svg)](https://crates.io/crates/dicom-pixeldata)
[![Documentation](https://docs.rs/dicom-pixeldata/badge.svg)](https://docs.rs/dicom-pixeldata)

This sub-project is directed at users of the DICOM-rs ecosystem.
It provides constructs for handling DICOM pixel data
and is responsible for decoding pixel data elements
into images or multi-dimensional arrays.

This crate is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Binary

`dicom-pixeldata` also offers the `dicom-transcode` command-line tool
(enable Cargo feature `cli`).
You can use it to transcode a DICOM file to another transfer syntax,
transforming pixel data along the way.

```none
Transcode a DICOM file

Usage: dicom-transcode [OPTIONS] <--ts <TS>|--expl-vr-le|--impl-vr-le|--jpeg-baseline|--jpeg-ls-lossless|--jpeg-ls|--jpeg-xl-lossless|--jpeg-xl> <FILE>

Arguments:
  <FILE>  

Options:
  -o, --output <OUTPUT>        The output file (default is to change the extension to .new.dcm)
      --quality <QUALITY>      The encoding quality (from 0 to 100)
      --effort <EFFORT>        The encoding effort (from 0 to 100)
      --ts <TS>                Transcode to the Transfer Syntax indicated by UID
      --expl-vr-le             Transcode to Explicit VR Little Endian
      --impl-vr-le             Transcode to Implicit VR Little Endian
      --jpeg-baseline          Transcode to JPEG baseline (8-bit)
      --jpeg-ls-lossless       Transcode to JPEG-LS lossless
      --jpeg-ls                Transcode to JPEG-LS near-lossless
      --jpeg-xl-lossless       Transcode to JPEG XL lossless
      --jpeg-xl                Transcode to JPEG XL
      --retain-implementation  Retain the original implementation class UID and version name
  -v, --verbose                Verbose mode
  -h, --help                   Print help
  -V, --version                Print version
```
