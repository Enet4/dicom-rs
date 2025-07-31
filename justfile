check:
    cargo fmt -- --check
    cargo clippy

fix:
    cargo fmt
    cargo clippy --fix --allow-dirty --allow-staged

fix_nightly:
    cargo +nightly fmt -- --config-path .rustfmt_nightly.toml
    #cargo +nightly clippy --fix --allow-dirty --allow-staged # To enable, after running format on nightly