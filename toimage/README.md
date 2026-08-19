# DICOM-rs `toimage`

[![CratesIO](https://img.shields.io/crates/v/dicom-toimage.svg)](https://crates.io/crates/dicom-toimage)
[![Documentation](https://docs.rs/dicom-toimage/badge.svg)](https://docs.rs/dicom-toimage)

A command line utility for converting DICOM image files
into general purpose image files (e.g. PNG).

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
dicom-toimage [OPTIONS] <FILES>...

Arguments:
  <FILES>...  Paths to the DICOM files (or directories) to convert

Options:
  -r, --recursive             Parse directory recursively
  -o, --out <OUTPUT>          Path to the output image, including file extension (replaces input extension with `.png` by default)
  -d, --outdir <OUTDIR>       Path to the output directory in bulk conversion mode, conflicts with `output`
  -e, --ext <EXT>             Extension when converting multiple files (default is to replace input extension with `.png`)
  -F, --frame <FRAME_NUMBER>  Frame number (0-indexed) [default: 0]
      --8bit                  Force output bit depth to 8 bits per sample
      --16bit                 Force output bit depth to 16 bits per sample
      --unwrap                Output the raw pixel data instead of decoding it
      --overlays              Render the overlay planes (groups 6000-601E) on top of the image (the output becomes RGB when an overlay applies to the frame)
      --overlay-color <OVERLAY_COLOR>  Color for overlay rendering as an RGB hex code (e.g. "ff0000" or "#00ff00"). May be given multiple times to color each overlay plane differently, assigned in ascending overlay group order and cycled if there are more planes than colors [default: ffffff]
      --overlay-file <OVERLAY_FILE>    Path to an additional DICOM file to take overlay planes from (e.g. a presentation state object referencing the image). May be given multiple times; planes are appended after those of the image itself
      --fail-first            Stop on the first failed conversion
  -v, --verbose               Print more information about the image and the output file
  -h, --help                  Print help
  -V, --version               Print version
```
