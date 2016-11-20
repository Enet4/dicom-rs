# DICOM Dictionary Builder

This sub-project is a tool for generating machine readable attribute dictionaries from the DICOM standard.

## Building

```bash
cargo build --release
```
## Usage

```text
    dictionary_builder [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f <FORMAT>        The output format [values: rs, json]
    -o <OUTPUT>        The path to the output file
```