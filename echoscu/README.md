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
    echoscu [FLAGS] <addr> [message-id]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v               verbose mode

ARGS:
    <addr>          socket address to SCP (example: "127.0.0.1:104")
    <message-id>    the C-ECHO message ID [default: 1]
```
