fuzz crate target timeout="60":
    cd {{crate}} && cargo +nightly fuzz run {{target}} -- -max_total_time={{timeout}}

fuzz-build:
    cd object   && cargo +nightly fuzz build
    cd ul       && cargo +nightly fuzz build
    cd pixeldata && cargo +nightly fuzz build
    cd parser   && cargo +nightly fuzz build
    cd json     && cargo +nightly fuzz build
    cd core     && cargo +nightly fuzz build

fuzz-all timeout="60":
    -cd object    && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run open_file             -- -max_total_time={{timeout}}
    -cd ul        && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run pdu_roundtrip         -- -max_total_time={{timeout}}
    -cd pixeldata && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run decode_simple_image   -- -max_total_time={{timeout}}
    -cd pixeldata && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run decode_image_file     -- -max_total_time={{timeout}}
    -cd parser    && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run dataset_tokens        -- -max_total_time={{timeout}}
    -cd parser    && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run lazy_dataset_tokens   -- -max_total_time={{timeout}}
    -cd json      && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run json_roundtrip        -- -max_total_time={{timeout}}
    -cd core      && timeout -k 5 $(( {{timeout}} + 10 )) cargo +nightly fuzz run value_parse           -- -max_total_time={{timeout}}

fuzz-list:
    @echo "object:    open_file"
    @echo "ul:        pdu_roundtrip"
    @echo "pixeldata: decode_simple_image  decode_image_file"
    @echo "parser:    dataset_tokens  lazy_dataset_tokens"
    @echo "json:      json_roundtrip"
    @echo "core:      value_parse"

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
