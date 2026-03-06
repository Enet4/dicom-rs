DICOM-rs is a Rust ecosystem providing libraries and CLI tools for working with the DICOM medical imaging standard. It is a Cargo workspace containing multiple crates (core, object, pixeldata, parser, encoding, transfer-syntax-registry, dictionary-std, json, ul, and several CLI tools like dump, storescu, storescp, echoscu, findscu, toimage, fromimage, scpproxy, etc.).

Purpose: Provide a pure-Rust stack to read, write, transcode, and network DICOM objects and to decode/handle imaging pixel data.

Tech stack: Rust >= 1.72.0 (MSRV for libraries), Cargo workspace, uses crates like `chrono`, `smallvec`, `snafu`, `itertools`, `jpeg-decoder`, `flate2`, and others across subcrates.

Code layout: Top-level workspace with member crates listed in `Cargo.toml`. Each crate follows standard Cargo layout (`src/lib.rs`, optional `src/main.rs` for binaries). `core` crate contains core traits and types.

Conventions: Uses Rust 2018/2021 editions; style follows idiomatic Rust. Error handling commonly via `snafu`. MSRV is documented in README.

Important files: `README.md`, `Cargo.toml` (workspace), per-crate `Cargo.toml`, `core/README.md`.

Useful commands (high level): `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`, `cargo +stable build` (or specific toolchain).