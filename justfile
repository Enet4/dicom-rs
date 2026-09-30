toolchain := env_var_or_default("FUZZ_TOOLCHAIN", "nightly")

fuzz crate target timeout="60":
    cd {{crate}} && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= cargo +{{toolchain}} fuzz run {{target}} -- -max_total_time={{timeout}}

fuzz-build:
    cd object    && cargo +{{toolchain}} fuzz build
    cd ul        && cargo +{{toolchain}} fuzz build
    cd pixeldata && cargo +{{toolchain}} fuzz build
    cd parser    && cargo +{{toolchain}} fuzz build
    cd json      && cargo +{{toolchain}} fuzz build
    cd core      && cargo +{{toolchain}} fuzz build

fuzz-all timeout="60":
    cd object    && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run open_file           -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-open_file.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-open_file.log
    cd ul        && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run pdu_roundtrip       -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-pdu_roundtrip.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-pdu_roundtrip.log
    cd ul        && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run pdata_reader        -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-pdata_reader.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-pdata_reader.log
    cd ul        && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run pdata_writer        -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-pdata_writer.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-pdata_writer.log
    cd pixeldata && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run decode_simple_image -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-decode_simple_image.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-decode_simple_image.log
    cd pixeldata && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run decode_image_file   -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-decode_image_file.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-decode_image_file.log
    cd parser    && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run dataset_tokens      -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-dataset_tokens.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-dataset_tokens.log
    cd parser    && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run lazy_dataset_tokens -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-lazy_dataset_tokens.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-lazy_dataset_tokens.log
    cd parser    && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run encode_element      -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-encode_element.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-encode_element.log
    cd json      && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run json_roundtrip seeds/json_roundtrip -- -dict=dictionaries/json_roundtrip.dict -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-json_roundtrip.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-json_roundtrip.log
    cd core      && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run value_parse         -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-value_parse.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-value_parse.log
    cd core      && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run value_ops           -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-value_ops.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-value_ops.log

fuzz-external timeout="60":
    cd pixeldata && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run decode_image_file   --features gdcm   -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-decode_image_file-gdcm.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-decode_image_file-gdcm.log
    cd pixeldata && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run decode_simple_image --features gdcm   -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-decode_simple_image-gdcm.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-decode_simple_image-gdcm.log
    cd pixeldata && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run decode_image_file   --features charls -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-decode_image_file-charls.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-decode_image_file-charls.log
    cd pixeldata && ASAN_OPTIONS=symbolize=1 DEBUGINFOD_URLS= timeout --foreground -k 5 $(( {{timeout}} + 10 )) cargo +{{toolchain}} fuzz run decode_simple_image --features charls -- -max_total_time={{timeout}} 2>&1 | tee /tmp/dicom-fuzz-decode_simple_image-charls.log; ! grep -q 'exit status: 72' /tmp/dicom-fuzz-decode_simple_image-charls.log

fuzz-list:
    @echo "object:    open_file"
    @echo "ul:        pdu_roundtrip  pdata_reader  pdata_writer"
    @echo "pixeldata: decode_simple_image  decode_image_file"
    @echo "parser:    dataset_tokens  lazy_dataset_tokens  encode_element"
    @echo "json:      json_roundtrip"
    @echo "core:      value_parse  value_ops"
    @echo ""
    @echo "external (just fuzz-external, needs a C/C++ toolchain for GDCM/CharLS):"
    @echo "pixeldata: decode_simple_image  decode_image_file  (--features gdcm, run separately from --features charls)"

# Seed DICOM-file fuzzers with one smallest .dcm per source directory.
# Usage: just apply_dcm_corpus /path/to/dcm/files
apply_dcm_corpus dir=".":
    #!/usr/bin/env bash
    set -e
    corpuses=(
        object/fuzz/corpus/open_file
        parser/fuzz/corpus/dataset_tokens
        parser/fuzz/corpus/lazy_dataset_tokens
        pixeldata/fuzz/corpus/decode_image_file
    )
    for c in "${corpuses[@]}"; do mkdir -p "$c"; done
    count=0
    while IFS= read -r f; do
        hash=$(sha256sum "$f" | cut -c1-40)
        for c in "${corpuses[@]}"; do
            cp "$f" "$c/$hash"
        done
        echo "seeded: $(basename "$f")"
        count=$((count + 1))
    done < <(
        find "{{dir}}" -name "*.dcm" -printf "%s\t%h\t%p\n" 2>/dev/null \
            | sort -t$'\t' -k1,1n \
            | awk -F'\t' '!seen[$2]++ {print $3}'
    )
    echo "done: $count files -> ${#corpuses[@]} corpus dirs"

    json_corpus="json/fuzz/corpus/json_roundtrip"
    mkdir -p "$json_corpus"
    json_count=0
    while IFS= read -r f; do
        json=$(cargo run -q -p dicom-dump -- -f json "$f" 2>/dev/null) || continue
        hash=$(printf '%s' "$json" | sha256sum | cut -c1-40)
        printf '%s' "$json" > "$json_corpus/$hash"
        echo "seeded json: $(basename "$f")"
        json_count=$((json_count + 1))
    done < <(
        find "{{dir}}" -name "*.dcm" -printf "%s\t%h\t%p\n" 2>/dev/null \
            | sort -t$'\t' -k1,1n \
            | awk -F'\t' '!seen[$2]++ {print $3}'
    )
    echo "done json: $json_count files -> $json_corpus"
