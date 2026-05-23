#!/usr/bin/env bash
# capture-sponge.sh — driver for capturing moreutils `sponge` reference fixtures.
#
# Usage:
#   ./fixtures/scripts/capture-sponge.sh
#
# Preconditions enforced at startup:
#   * LC_ALL=C.UTF-8 (capture protocol; FIXTURES README §Capture protocol)
#   * moreutils `sponge` available on PATH (e.g. `apt install moreutils` in WSL2)
#   * Run from the repo root (presence of Cargo.toml is the marker)
#
# This script writes outputs to fixtures/moreutils_outputs/<category>/ and
# leaves fixtures/inputs/<category>/ alone (inputs are committed under
# .gitattributes binary, and the HINT-001 rule forbids regenerating inputs
# and outputs in the same commit).

set -euo pipefail

# --- Env assertions ----------------------------------------------------------

if [[ "${LC_ALL:-}" != "C.UTF-8" ]]; then
    echo "ERROR: LC_ALL must equal C.UTF-8 at capture time (current: '${LC_ALL:-<unset>}')." >&2
    echo "       Re-run as: LC_ALL=C.UTF-8 ./fixtures/scripts/capture-sponge.sh" >&2
    exit 2
fi

if ! command -v sponge >/dev/null 2>&1; then
    echo "ERROR: moreutils 'sponge' not found on PATH." >&2
    echo "       Install via: apt install moreutils (Debian/Ubuntu) or brew install moreutils (macOS)." >&2
    exit 2
fi

if [[ ! -f Cargo.toml ]]; then
    echo "ERROR: must be run from the rusty-sponge repo root." >&2
    exit 2
fi

# --- Pinned upstream commit recording ----------------------------------------

# Record the moreutils version at capture time. On Debian/Ubuntu, `dpkg -s` returns
# the packaged version; on a from-source build, set MOREUTILS_PIN_OVERRIDE.
MOREUTILS_VERSION="${MOREUTILS_PIN_OVERRIDE:-$(dpkg -s moreutils 2>/dev/null | awk '/^Version:/ {print $2}' || echo 'unknown')}"
echo "Capturing fixtures from moreutils ${MOREUTILS_VERSION}"
echo "  (record the upstream sponge.c commit in fixtures/README.md if not already pinned)"

# --- Categories --------------------------------------------------------------

FIXTURES_ROOT="fixtures"
INPUTS_ROOT="${FIXTURES_ROOT}/inputs"
OUTPUTS_ROOT="${FIXTURES_ROOT}/moreutils_outputs"

mkdir -p "${OUTPUTS_ROOT}"

# Each category subdirectory under inputs/ MUST be present before capture.
# The script iterates them and runs moreutils sponge against each input.

if [[ ! -d "${INPUTS_ROOT}" ]] || [[ -z "$(ls -A "${INPUTS_ROOT}" 2>/dev/null)" ]]; then
    echo "WARNING: ${INPUTS_ROOT}/ is empty. Populate inputs first (per task T031–T033 et al.)." >&2
    exit 0
fi

for category_dir in "${INPUTS_ROOT}"/*/; do
    category=$(basename "${category_dir}")
    out_dir="${OUTPUTS_ROOT}/${category}"
    mkdir -p "${out_dir}"

    echo "  category: ${category}"

    # Iterate every input file in the category. The capture pattern is:
    #   cat <input> | sponge <target_copy>
    # where <target_copy> starts as a fresh copy of the target (if any) per
    # category convention. The output file we save is the resulting <target_copy>
    # contents after sponge completes.
    for input_file in "${category_dir}"*.in; do
        [[ -f "${input_file}" ]] || continue
        base=$(basename "${input_file}" .in)
        target="${out_dir}/${base}.target"

        # Some categories ship a `<base>.preexisting` file that should be copied
        # into the target slot before sponge runs (the existing-target replacement case).
        preexisting="${category_dir}${base}.preexisting"
        if [[ -f "${preexisting}" ]]; then
            cp "${preexisting}" "${target}"
        else
            rm -f "${target}"
        fi

        # Determine which sponge flags apply (per-category convention recorded in
        # fixtures/<category>/CAPTURE.json or, lacking that, default to no flags).
        sponge_flags=""
        if [[ -f "${category_dir}CAPTURE.json" ]]; then
            sponge_flags=$(jq -r '.flags // ""' "${category_dir}CAPTURE.json" 2>/dev/null || echo "")
        fi

        # Run sponge.
        # shellcheck disable=SC2086
        cat "${input_file}" | sponge ${sponge_flags} "${target}"

        echo "    captured: ${target}"
    done
done

echo ""
echo "Capture complete. Review with: git diff --stat fixtures/moreutils_outputs/"
echo "Recapture policy: do NOT regenerate inputs/ and moreutils_outputs/ in the same commit (HINT-001)."
