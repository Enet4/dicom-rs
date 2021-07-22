# DICOM-rs `findscu`

[![CratesIO](https://img.shields.io/crates/v/dicom-findscu.svg)](https://crates.io/crates/dicom-findscu)
[![Documentation](https://docs.rs/dicom-findscu/badge.svg)](https://docs.rs/dicom-findscu)

This is an implementation of the DICOM Find SCU (C-Find),
which can be used for uploading DICOM files to other DICOM devices.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `findscu` tools in other DICOM software projects.

```none
DICOM C-STORE SCU

USAGE:
    dicom-findscu [FLAGS] [OPTIONS] <addr> [file]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    verbose mode

OPTIONS:
        --called-ae-title <called-ae-title>      the called AE title [default: ANY-SCP]
        --calling-ae-title <calling-ae-title>    the calling AE title [default: STORE-SCU]
        --max-pdu-length <max-pdu-length>        the maximum PDU length [default: 16384]
    -m, --message-id <message-id>                the C-STORE message ID [default: 1]

ARGS:
    <addr>    socket address to FIND SCP (example: "127.0.0.1:1045")
    [file]    a bare DICOM file representing a C-FIND-RQ message
```
