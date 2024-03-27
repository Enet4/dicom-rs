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
    dicom-storescu [FLAGS] [OPTIONS] <addr> [files]...

FLAGS:
        --fail-first    fail if not all DICOM files can be transferred
    -h, --help          Prints help information
    -V, --version       Prints version information
    -v, --verbose       verbose mode

OPTIONS:
        --called-ae-title <called-ae-title>
            the called Application Entity title, overrides AE title in address if present [default: ANY-SCP]

        --calling-ae-title <calling-ae-title>    the calling Application Entity title [default: STORE-SCU]
        --max-pdu-length <max-pdu-length>        the maximum PDU length accepted by the SCU [default: 16384]
    -m, --message-id <message-id>                the C-STORE message ID [default: 1]
        --username <username>                    user identity username
        --password <password>                    user identity password
        --kerberos-service-ticket <ticket>       user identity Kerberos service ticket
        --saml-assertion <assertion>             user identity SAML assertion
        --jwt <jwt>                              user identity JWT

ARGS:
    <addr>        socket address to Store SCP, optionally with AE title (example: "STORE-SCP@127.0.0.1:104")
    <files>...    the DICOM file(s) to store
```

Example:

```sh
dicom-storescu MAIN-STORAGE@192.168.1.99:104 xray1.dcm xray2.dcm
```
