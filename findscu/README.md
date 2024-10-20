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

There are three _non-exclusive_ ways to specify a DICOM query:

### Passing a DICOM query object file

You may optionally provide a path to a DICOM query object file
to bootstrap your query object,
otherwise you start with an empty one.
There are currently no tools in DICOM-rs
to assist in the process of creating these objects,
but one can convert DCMTK DICOM data dumps
into compatible DICOM query objects,
or write these tools yourself.

```sh
# query is defined in query.dcm
dicom-findscu PACS@pacs.example.com:1045 --study query.dcm
```

### Passing a query text file

An easier approach to specifying queries is
through the command line argument `--query-file «file»`.
The text file should contain a sequence of lines,
each of the form `«field_path»=«field_value»`, where:

- `field_path` is a data element selector path
  (see the element selector syntax below);
- and `field_value` is the respective value or pattern to match
  against the value of the specified DICOM attribute.
  It can be empty, which in that case the `=` may also be left out.

For example, given the file `query.txt`:

```none
# comments are supported
AccessionNumber
ScheduledProcedureStepSequence.Modality=MR
ScheduledProcedureStepSequence.ScheduledProcedureStepStartDate=20240703
```

You can do:

```sh
dicom-findscu PACS@pacs.example.com:1045 -W --query-file query.txt
```

### Using the multi-value `-q` option

Finally, the `-q` option accepts multiple query values
of the same form as in `--query-file`.
See more examples below.

Each of these forms will extend and override the query object in this order.

#### Selector syntax

Simple attribute selectors comprise a single data element key,
specified by a standard DICOM tag
(in one of the forms `(gggg,eeee)`, `gggg,eeee`, or `ggggeeee`)
or a tag keyword name such as `PatientName`.
To specify a sequence, use multiple of these separated by a dot
(e.g. `ScheduledProcedureStepSequence.0040,0020`).
Nested attributes will automatically construct intermediate sequences as needed.

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
    -q ScheduledProcedureStepSequence.ScheduledProcedureStepStatus=ARRIVED
```
