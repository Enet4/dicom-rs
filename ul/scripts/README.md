# Scripts for dicom-ul


## Generate DIMSE messages

Install `uv` and run `uv run python generate_command_structs.py`

This will populate `src/pdu/generated.rs`

The script parses the tables in the HTML dicom standard which has current
been tested with 2025c.

If there are changes to the tables in terms of header names, table locations, etc.
the script will need to be updated.
