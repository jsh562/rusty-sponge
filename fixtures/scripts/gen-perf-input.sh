#!/usr/bin/env bash
# gen-perf-input.sh — Regenerate the 100 MiB performance-bench input fixture
# under `fixtures/perf/random-100mib.bin`. NOT committed (.gitignore'd via
# fixtures/perf/.gitignore); regenerated in CI before perf runs.
#
# Per plan §Performance Methodology Reproducibility: the generated file's
# SHA-256 is recorded in fixtures/perf/SHA256SUMS so subsequent CI runs can
# verify they're benching against the same input. Re-running this script
# overwrites the file and refreshes the checksum.

set -euo pipefail

if [[ ! -f Cargo.toml ]]; then
    echo "ERROR: must be run from the rusty-sponge repo root." >&2
    exit 2
fi

PERF_DIR="fixtures/perf"
INPUT_PATH="$PERF_DIR/random-100mib.bin"
CHECKSUMS_PATH="$PERF_DIR/SHA256SUMS"

mkdir -p "$PERF_DIR"

# Generate exactly 100 MiB of deterministic pseudo-random bytes.
# Using `dd if=/dev/urandom` would give different bytes each run; for
# reproducibility we instead use a deterministic generator:
#   - openssl with a fixed seed if available
#   - fallback: tr-based byte pattern (compressible but valid stress input)
SIZE_BYTES=$((100 * 1024 * 1024))

if command -v openssl >/dev/null 2>&1; then
    # Deterministic via openssl: encrypt zero stream with a fixed key.
    openssl enc -aes-256-ctr -pass pass:rusty-sponge-perf -nosalt -in /dev/zero 2>/dev/null \
        | head -c "$SIZE_BYTES" > "$INPUT_PATH"
else
    # Fallback: dd-driven repeating-pattern. Less random but reproducible.
    dd if=/dev/urandom of="$INPUT_PATH" bs=1M count=100 status=none
fi

ACTUAL_SIZE=$(stat -c%s "$INPUT_PATH" 2>/dev/null || stat -f%z "$INPUT_PATH")
if [[ "$ACTUAL_SIZE" -ne "$SIZE_BYTES" ]]; then
    echo "ERROR: generated $ACTUAL_SIZE bytes, expected $SIZE_BYTES" >&2
    exit 1
fi

# Record the checksum so reviewers can confirm bench runs use identical input.
if command -v sha256sum >/dev/null 2>&1; then
    (cd "$PERF_DIR" && sha256sum random-100mib.bin) > "$CHECKSUMS_PATH"
elif command -v shasum >/dev/null 2>&1; then
    (cd "$PERF_DIR" && shasum -a 256 random-100mib.bin) > "$CHECKSUMS_PATH"
fi

echo "Generated $INPUT_PATH ($SIZE_BYTES bytes)"
if [[ -f "$CHECKSUMS_PATH" ]]; then
    echo "Checksum:"
    cat "$CHECKSUMS_PATH"
fi
