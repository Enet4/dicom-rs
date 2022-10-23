# DICOM-rs `storescp`

[![CratesIO](https://img.shields.io/crates/v/dicom-storescp.svg)](https://crates.io/crates/dicom-storescp)

This is an implementation of the DICOM Storage SCP (C-STORE),
which can be used for receiving DICOM files from other DICOM devices.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

```none
dicom-storescp [-p tcp_port] [-o dicom_storage_dir] [OPTIONS]
```

Note that this tool is not necessarily a drop-in replacement
for `storescp` tools in other DICOM software projects.
Run `dicom-storescp --help` for more details.
