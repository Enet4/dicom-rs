# DICOM-rs `storescu`

[![CratesIO](https://img.shields.io/crates/v/dicom-storescu.svg)](https://crates.io/crates/dicom-storescu)
[![Documentation](https://docs.rs/dicom-storescu/badge.svg)](https://docs.rs/dicom-storescu)

This is an implementation of the DICOM Storage SCU (C-STORE),
which can be used for uploading DICOM files to other DICOM devices.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `storescu` tools in other DICOM software projects.

```none
DICOM C-STORE SCU

USAGE:
    dicom-storescu [FLAGS] [OPTIONS] <addr> <file>

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
    <addr>    socket address to STORE SCP (example: "127.0.0.1:104")
    <file>    the DICOM file to store
```
