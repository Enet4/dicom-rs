# DICOM-rs `findscu`

[![CratesIO](https://img.shields.io/crates/v/dicom-findscu.svg)](https://crates.io/crates/dicom-findscu)
[![Documentation](https://docs.rs/dicom-findscu/badge.svg)](https://docs.rs/dicom-findscu)

This is an implementation of the DICOM Find SCU (C-Find),
which can be used to search for study and patient records in a DICOM archive.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `findscu` tools in other DICOM software projects.
Run `dicom-findscu --help`  for more details.

There are two exclusive ways to specify a DICOM query:

### Using the multi-value `-q` option

Each value is a text of the form `«field_name»=«field_value»`, where:

- `field_name` is either a standard DICOM tag
  (in the form `(gggg,eeee)` or `gggg,eeee`)
  or a tag keyword name such as `PatientName`.
- `field_value` is the respective value or pattern to match
  against the value of the specified DICOM attribute.
  It can be empty, which in that case the `=` may also be left out.

Basic usage includes searching for a study or patient by a certain attribute.
Only patient level and study level queries are supported.

```sh
# query application entity STORAGE for a study with the accession number A123
dicom-findscu STORAGE@pacs.example.com:1045 --study -q AccessionNumber=A123

# query application entity PACS for patients born in 1990-12-25
dicom-findscu PACS@pacs.example.com:1045 --patient -q PatientBirthDate=19901225

# wild-card query: grab a list of all study instance UIDs
dicom-findscu PACS@pacs.example.com:1045 -S -q "StudyInstanceUID=*"
```

### Passing a query object file

As an alternative to term queries,
you can also provide a DICOM query object
as a file.
There are currently no tools in DICOM-rs
to assist in the process of creating these objects,
but one can convert DCMTK DICOM data dumps
into compatible DICOM query objects.

```sh
# query is defined in file
dicom-findscu PACS@pacs.example.com:1045 --study query.dcm
```
