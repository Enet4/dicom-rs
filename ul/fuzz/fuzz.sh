#!/usr/bin/env sh
RUST_TOOLCHAIN=${RUST_TOOLCHAIN:-nightly}
env ASAN_OPTIONS=allocator_may_return_null=1:max_allocation_size_mb=40 cargo +${RUST_TOOLCHAIN} fuzz run pdu_roundtrip
