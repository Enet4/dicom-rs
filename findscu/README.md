# DICOM-rs `findscu`

[![CratesIO](https://img.shields.io/crates/v/dicom-findscu.svg)](https://crates.io/crates/dicom-findscu)
[![Documentation](https://docs.rs/dicom-findscu/badge.svg)](https://docs.rs/dicom-findscu)

This is an implementation of the DICOM Find SCU (C-FIND),
which can be used to search for study and patient records in a DICOM archive.

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `findscu` tools in other DICOM software toolkits.
Run `dicom-findscu --help`  for more details.

Basic usage includes searching for a study or patient by a certain attribute.
The following query/retrieve information models are supported at the moment:

- **`-S`**: Study Root Query/Retrieve Information Model – FIND (default)
- **`-P`**: Patient Root Query/Retrieve Information Model - FIND
- **`-W`**: Modality Worklist Information Model – FIND

There are two _non-exclusive_ ways to specify a DICOM query:

### Passing a DICOM query object file

You may optionally start by providing a DICOM query object as a file.
There are currently no tools in DICOM-rs
to assist in the process of creating these objects,
but one can convert DCMTK DICOM data dumps
into compatible DICOM query objects,
or write these tools yourself.

```sh
# query is defined in file
dicom-findscu PACS@pacs.example.com:1045 --study query.dcm
```

### Using the multi-value `-q` option

Each value is a text of the form `«field_path»=«field_value»`, where:

- `field_path` is a data element selector path;
- and `field_value` is the respective value or pattern to match
  against the value of the specified DICOM attribute.
  It can be empty, which in that case the `=` may also be left out.

If a path to a DICOM query file is passed,
these options will extend or override the query object from the file.

#### Selector syntax

Simple attribute selectors comprise a single data element key,
specified by a standard DICOM tag
(in one of the forms `(gggg,eeee)`, `gggg,eeee`, or `ggggeeee`)
or a tag keyword name such as `PatientName`.
To specify a sequence, use multiple of these separated by a dot
(e.g. `ScheduledProcedureStepSequence.0040,0020`).
When writing nested attributes,
you currently need to declare the sequence in a separate query option,
and _only then_ define the attribute inside it, in this order.
See the examples below for clarity.

#### Examples

```sh
# query application entity STORAGE for a study with the accession number A123
dicom-findscu STORAGE@pacs.example.com:1045 --study -q AccessionNumber=A123

# query application entity PACS for patients born in 1990-12-25
dicom-findscu PACS@pacs.example.com:1045 --patient -q PatientBirthDate=19901225

# wild-card query: grab a list of all study instance UIDs
dicom-findscu PACS@pacs.example.com:1045 -S -q "StudyInstanceUID=*"

# retrieve the modality worklist information
# for scheduled procedures where the patient has arrived
dicom-findscu INFO@pacs.example.com:1045 --mwl \
    -q ScheduledProcedureStepSequence \
    -q ScheduledProcedureStepSequence.ScheduledProcedureStepStatus=ARRIVED
```
