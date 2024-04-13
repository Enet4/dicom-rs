# DICOM-rs `toimage`

[![CratesIO](https://img.shields.io/crates/v/dicom-toimage.svg)](https://crates.io/crates/dicom-toimage)
[![Documentation](https://docs.rs/dicom-toimage/badge.svg)](https://docs.rs/dicom-toimage)

A command line utility for converting DICOM image files
into general purpose image files (e.g. PNG).

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
Usage: dicom-toimage [OPTIONS] <FILE>...

Arguments:
  <FILE>...  Paths to the DICOM files to convert

Options:
  -r, --recursive             Recursively parse sub folders if the given path is a directory
  -o, --out <OUTPUT>          Name of the output file(s) (default is the same as the input file with extension change)
  -d, --dir <OUTPUT_DIR>      Path to the output directory Directory will be created if it does not exist (default is `.`)
  -e, --ext <EXT>             File extension to use for output files [default: png]
  -F, --frame <FRAME_NUMBER>  Frame number (0-indexed) [default: 0]
      --8bit                  Force output bit depth to 8 bits per sample
      --16bit                 Force output bit depth to 16 bits per sample
      --unwrap                Output the raw pixel data instead of decoding it
  -v, --verbose               Print more information about the image and the output file
  -h, --help                  Print help
  -V, --version               Print version
```
