#!/bin/sh

env ASAN_OPTIONS=allocator_may_return_null=1:max_allocation_size_mb=40 cargo +nightly fuzz run open_file
