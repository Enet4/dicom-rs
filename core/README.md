# DICOM-rs core library

This sub-project implements the essential data structures and mechanisms for dealing with DICOM information and communication formats. The current aim of this crate is to provide reading and writing capabilities of DICOM data, either from files or other sources.

## Building

```sh
cargo build --release
```

## Roadmap (-ish)

Although no timeline is expected, and the list of features to be included here is uncertain, my intent is to publish the first version of this crate when the DICOM object abstraction becomes sufficiently functional. That is, the user should be capable of:

 - Reading DICOM objects from files, and retrieving elements without eagerly keeping the whole file in memory, in one of these transfer syntaxes: _ExplicitVRLittleEndian_, _ImplicitVRLittleEndian_ and _ExplicitVRBigEndian_;
 - Creating and writing DICOM objects;
 - Fetching an object's pixel data as an n-dimensional array;
 - All of this using a practical DICOM object abstraction.
