Common commands for development in this repository (run from project root):

- Build the whole workspace: `cargo build`
- Run tests (workspace): `cargo test` or `cargo test -p <crate>`
- Check compilation without producing artifacts: `cargo check`
- Run with a specific toolchain (if needed): `rustup run stable cargo build` or `cargo +stable build`
- Format code: `cargo fmt --all` (requires `rustfmt`)
- Lint suggestions: `cargo clippy --all-targets --all-features -- -D warnings` (requires `clippy`)
- Run a specific binary: `cargo run -p <crate> --bin <name>` or `cargo run -p dump -- <args>`
- Run a crate's tests: `cargo test -p core` or from inside crate: `cargo test`
- Build in release: `cargo build --release`
- Clean: `cargo clean`
- Inspect workspace members: open `Cargo.toml` top-level `members` list
- Git utilities: `git status`, `git branch`, `git checkout -b <branch>`, `git commit -m "..."`, `git push`.

Notes: Some crates have additional tools or fuzz targets under `fuzz/`. The repository uses Rust features selectively per crate; consult crate `Cargo.toml` for feature flags.