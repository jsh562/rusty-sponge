#!/usr/bin/env bash
# feature-lint: top-level runner shim.
#
# Invokes every sub-check defined in lint.sh in sequence, accumulates
# violations, and prints a human-readable summary.
#
# See tools/feature-lint/README.md for the invocation contract.

set -u

# Resolve the directory containing this script so we can source lint.sh
# regardless of the caller's working directory.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Sanity check.
if [[ ! -f "$SCRIPT_DIR/lint.sh" ]]; then
    printf 'feature-lint: cannot find lint.sh next to run.sh (looked at %s)\n' "$SCRIPT_DIR/lint.sh" >&2
    exit 2
fi

# Source lint.sh to import the check_* helper functions.
# (lint.sh's bottom-of-file dispatcher is guarded by [[ BASH_SOURCE == 0 ]],
# so sourcing does not trigger its own argument parsing.)
# shellcheck source=lint.sh
. "$SCRIPT_DIR/lint.sh"

# Run every sub-check, capturing per-check exit codes for the summary.
violations=0
checks=(
    "required-umbrellas"
    "leaf-ci-matrix"
    "phantom-leaf"
    "readme-matrix"
    "changelog-migration"
)
declare -A results

for name in "${checks[@]}"; do
    run_check_by_name "$name"
    rc=$?
    results["$name"]=$rc
    if [[ $rc -ne 0 ]]; then
        violations=$((violations + 1))
    fi
done

# Print summary to stdout (errors already went to stderr from lint.sh).
echo "---"
echo "feature-lint sub-check summary:"
for name in "${checks[@]}"; do
    rc="${results[$name]}"
    if [[ "$rc" == "0" ]]; then
        printf '  %-22s  PASS\n' "$name"
    else
        printf '  %-22s  FAIL (exit %s)\n' "$name" "$rc"
    fi
done

if [[ $violations -eq 0 ]]; then
    echo "feature-lint: PASS"
    exit 0
fi

echo "feature-lint: $violations violations"
exit 2
