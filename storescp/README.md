# DICOM-rs `storescp`

[![CratesIO](https://img.shields.io/crates/v/dicom-storescp.svg)](https://crates.io/crates/dicom-storescp)
[![Documentation](https://docs.rs/dicom-storescp/badge.svg)](https://docs.rs/dicom-storescp)

This is an implementation of the DICOM Storage SCP (C-STORE),
which can be used for receiving DICOM files from other DICOM devices.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `storescp` tools in other DICOM software projects.

```none
DICOM C-STORE SCP

USAGE:
    dicom-storescp [FLAGS] [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    verbose mode

OPTIONS:
        --max-pdu-length <max-pdu-length>        the maximum PDU length [default: 16384]
    -p               The port to listen on

ARGS:
```
