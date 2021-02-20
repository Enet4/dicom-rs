# DICOM-rs `echoscu`

[![CratesIO](https://img.shields.io/crates/v/dicom-echoscu.svg)](https://crates.io/crates/dicom-echoscu)
[![Documentation](https://docs.rs/dicom-echoscu/badge.svg)](https://docs.rs/dicom-echoscu)

This is an implementation of the DICOM Verification C-ECHO SCU,
which can be used for verifying DICOM nodes.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `echoscu` tools in other DICOM software projects.

```none
DICOM C-ECHO SCU

USAGE:
    dicom-echoscu [FLAGS] [OPTIONS] <addr>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    verbose mode

OPTIONS:
        --called-ae-title <called-ae-title>      the called AE title [default: ANY-SCP]
        --calling-ae-title <calling-ae-title>    the calling AE title [default: ECHOSCU]
    -m, --message-id <message-id>                the C-ECHO message ID [default: 1]

ARGS:
    <addr>    socket address to SCP (example: "127.0.0.1:104")
```
