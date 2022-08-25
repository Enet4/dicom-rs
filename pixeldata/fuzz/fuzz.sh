#!/usr/bin/env sh
set -eu
# pick fuzz target binary
FUZZ_TARGET="${1-decode_image_file}"
env ASAN_OPTIONS=allocator_may_return_null=1:max_allocation_size_mb=1024 cargo +nightly fuzz run "$FUZZ_TARGET" -- -max_len=16777216
