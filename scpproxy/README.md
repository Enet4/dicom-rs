# DICOM-rs `scpproxy`

[![CratesIO](https://img.shields.io/crates/v/dicom-scpproxy.svg)](https://crates.io/crates/dicom-scpproxy)
[![Documentation](https://docs.rs/dicom-scpproxy/badge.svg)](https://docs.rs/dicom-scpproxy)

This is an implementation of the Proxy SCP, which can be used for logging and debugging purposes. 

## Usage

```
    scpproxy [OPTIONS] <destination-host> <destination-port>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --listen-port <listen-port>    The port that we will listen for SCU connections on [default: 3333]

ARGS:
    <destination-host>    The destination host name (SCP)
    <destination-port>    The destination host port (SCP)
```
