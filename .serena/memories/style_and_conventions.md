Style and conventions for DICOM-rs (collected from repo):

- Language: Rust (2018/2021 edition depending on crate). Use idiomatic Rust patterns.
- Error handling: `snafu` is commonly used for error definitions.
- MSRV: Libraries require Rust >= 1.72.0; other crates may require latest stable.
- Formatting: `rustfmt` (run via `cargo fmt --all`).
- Linting: `clippy` with `-D warnings` recommended for CI parity.
- Module layout: per-crate `src/lib.rs` for libraries; binaries in `src/main.rs` and `src/bin/`.
- Tests: Use `cargo test` (unit and integration tests). Integration tests in `tests/` directories of crates.
- Documentation: Use `rustdoc` comments (`///`); docs published on docs.rs.

Naming: follow Rust naming conventions (snake_case for functions/variables, CamelCase for types).

Design patterns: crates expose modular APIs; `parent` crate re-exports key components as `dicom` crate for downstream use.

If anything else is needed, ask the maintainers or consult `CONTRIBUTING.md`.