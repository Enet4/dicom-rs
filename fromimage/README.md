# DICOM-rs `fromimage`

[![CratesIO](https://img.shields.io/crates/v/dicom-fromimage.svg)](https://crates.io/crates/dicom-fromimage)
[![Documentation](https://docs.rs/dicom-fromimage/badge.svg)](https://docs.rs/dicom-fromimage)

This command line tool takes a base DICOM file of the image module
and replaces the various DICOM attributes with those of another file.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
Usage: dicom-fromimage [OPTIONS] <DCM_FILE> <IMG_FILE>

Arguments:
  <DCM_FILE>  Path to the base DICOM file to read
  <IMG_FILE>  Path to the image file to replace the DICOM file

Options:
  -o, --out <OUTPUT>
          Path to the output image (default is to replace input extension with `.new.dcm`)
      --transfer-syntax <TRANSFER_SYNTAX>
          Override the transfer syntax UID
      --encapsulate
          Encapsulate the image file raw data in a fragment sequence instead of writing native pixel data
      --retain-implementation
          Retain the implementation class UID and version name from base DICOM
  -v, --verbose
          Print more information about the image and the output file
  -h, --help
          Print help
  -V, --version
          Print version
```

### Example

Given a template DICOM file `base.dcm`,
replace the image data with the image in `image.png`:

```none
dicom-fromimage base.dcm image.png -o image.dcm
```

This will read the image file in the second argument
and save it as native pixel data in Explicit VR Little Endian to `image.dcm`.

You can also encapsulate the image file into a pixel data fragment,
without converting to native pixel data.
This allows you to create a DICOM file in JPEG baseline:

```none
dicom-fromimage base.dcm image.jpg --transfer-syntax 1.2.840.10008.1.2.4.50 --encapsulate -o image.dcm
```

**Note:** `--transfer-syntax` is just a UID override,
it will not automatically transcode the pixel data
to conform to the given transfer syntax. 
To transcode files between transfer syntaxes,
see [`dicom-transcode`](https://github.com/Enet4/dicom-rs/tree/master/pixeldata).
