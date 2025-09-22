# DIMSE Message Builder

A Rust tool that generates DICOM command structs from the DICOM standard specification.

## Features

- Fetches the latest DICOM Part 7 specification from DICOM.nema.org
- Parses XML tables containing command definitions
- Generates Rust structs with the `Builder` pattern using `bon`
- Implements the `Command` trait for each generated struct
- Supports marker traits for dataset requirements
- Uses the `quote` macro for clean, maintainable code generation

## Usage

Run the tool to generate the DICOM command structs:

```bash
cargo run
```

By default, this will generate `ul/src/pdu/generated.rs`. In relation to the workspace root
You can specify a different output file:

```bash
cargo run -- --output path/to/output.rs
```

## Generated Structs

The tool generates structs for the following DICOM message types:

- C-STORE (Request, Response)
- C-FIND (Request, Response, Cancel)
- C-GET (Request, Response, Cancel)
- C-MOVE (Request, Response, Cancel)
- C-ECHO (Request, Response)

Each struct includes:
- Fields based on the DICOM standard tables
- Builder pattern support via `bon`
- `Command` trait implementation
- Appropriate marker traits (`DatasetRequiredCommand`, `DatasetConditionalCommand`, `DatasetForbiddenCommand`)