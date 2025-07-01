# DICOM-rs `movescu`

This is an implementation of the DICOM Move SCU (C-MOVE),
which can be used to transfer images to an C-STORE archive.
For ease of use it automatically starts a STORE-SCP when performing 
a move operation to itself (c-move destination identical to calling-aet) 
and stores the received files in the current directory

This tool is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project.

## Usage

Note that this tool is not necessarily a drop-in replacement
for `movescu` tools in other DICOM software toolkits.
Run `dicom-movescu --help`  for more details.

The following query/retrieve information models are supported at the moment:

- **`-S`**: Study Root Query/Retrieve Information Model – MOVE (default)
- **`-P`**: Patient Root Query/Retrieve Information Model - MOVE

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
dicom-movescu PACS@pacs.example.com:1045 --study query.dcm
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
QueryRetrieveLevel=SERIES
StudyInstanceUID=1.3.46.670589.5.2.10.2156913941.892665384.993397
SeriesInstanceUID=1.3.46.670589.5.2.10.2156913941.892665339.718742
```

You can do:

```sh
dicom-movescu PACS@pacs.example.com:1045 -S --query-file query.txt
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
or a tag keyword name such as `StudyInstanceUID`.

#### Examples

```sh
# retrieve study from application entity PACS with StudyInstanceUID
dicom-movescu PACS@pacs.example.com:1045 -q QueryRetrieveLevel=STUDY -q StudyInstanceUID=1.3.46.670589.5.2.10.2156913941.892665384.993397 --move-destination=STORE-SCP

# retrieve series from application entity PACS with StudyInstanceUID and SeriesInstanceUID
dicom-movescu PACS@pacs.example.com:1045 -q QueryRetrieveLevel=SERIES -q StudyInstanceUID=1.3.46.670589.5.2.10.2156913941.892665384.993397 -q SeriesInstanceUID=1.3.46.670589.5.2.10.2156913941.892665384.993397 --move-destination=STORE-SCP

# transfer study from application entity PACS to AE STORAGE
dicom-movescu PACS@pacs.example.com:1045 -q QueryRetrieveLevel=STUDY -q StudyInstanceUID=1.3.46.670589.5.2.10.2156913941.892665384.993397 --move-destination=STORAGE
```

#### Limitations

Currently Instance (image) level C-Move does not work for local store-scp but should work when performing a C-MOVE to other archives
The movescu tool makes no attempt to prevent incorrect queries. In particular, the query keys of a C-MOVE request should only contain the QueryRetrieveLevel attribute and one or more of the so-called "unique key attributes" (PatientID, StudyInstanceUID, SeriesInstanceUID and SOPInstanceUID).

