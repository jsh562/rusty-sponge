#!/usr/bin/env bash
# feature-lint: portfolio-wide Cargo features convention compliance check.
#
# See tools/feature-lint/README.md for the invocation contract.
#
# Implements FR-052 sub-rules (T003..T007 of spec 00011):
#   1. required-umbrellas    — default/full/cli/<port>-classic present in Cargo.toml
#   2. leaf-ci-matrix        — every leaf has a check-leaf-<leaf> CI job
#   3. phantom-leaf          — every leaf has at least one #[cfg(feature)] in src/
#   4. readme-matrix         — README "## Cargo Features" matrix lists every leaf
#   5. changelog-migration   — CHANGELOG [0.2.0] migration table is exhaustive
#
# Exit codes: 0 = compliance, 2 = at least one violation.

set -u
# Intentionally NOT set -e: each sub-check returns its own exit code; the
# top-level invocation aggregates across them.

# --------------------------------------------------------------------------
# Configuration / env vars
# --------------------------------------------------------------------------

: "${PORT_PATH:?feature-lint: PORT_PATH env var required (absolute path to per-port repo)}"
: "${UMBRELLA_PATH:?feature-lint: UMBRELLA_PATH env var required (absolute path to umbrella repo)}"
: "${STRICT_MODE:=1}"

# Required umbrella names per project-instructions.md §Cargo Feature Surface.
# Derived per-port: <port>-classic is computed from the Cargo.toml [package].name.
REQUIRED_UMBRELLAS_BASE=("default" "full" "cli")

# --------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------

err() {
    # Emit a feature-lint error to stderr.
    printf 'feature-lint: %s\n' "$*" >&2
}

# Extract the port name from Cargo.toml [package].name.
# Stdout: the port name (e.g., "rusty-figlet"). Empty if not found.
get_port_name() {
    local cargo_toml="$PORT_PATH/Cargo.toml"
    if [[ ! -f "$cargo_toml" ]]; then
        err "Cargo.toml not found at $cargo_toml"
        return 1
    fi
    # Look for the first 'name = "..."' under [package].
    # POSIX-friendly extraction: track [package] section, then first name = "...".
    awk '
        /^\[package\]/ { in_package = 1; next }
        /^\[/         { in_package = 0; next }
        in_package && /^[[:space:]]*name[[:space:]]*=/ {
            # strip everything up to the first quote, then strip the trailing quote
            sub(/^[^"]*"/, "")
            sub(/".*$/, "")
            print
            exit
        }
    ' "$cargo_toml"
}

# Derive the short tool stem (drop the `rusty-` prefix) used in
# `<port>-classic` style umbrella/bundle names per
# `project-instructions.md` §Cargo Feature Surface and ADR-0006. For
# `rusty-figlet` the stem is `figlet`; for a port whose crate name is
# already bare (e.g. `figlet`) the stem equals the crate name.
get_port_stem() {
    local full="$1"
    case "$full" in
        rusty-*) printf '%s\n' "${full#rusty-}" ;;
        *)       printf '%s\n' "$full" ;;
    esac
}

# Enumerate feature keys declared under [features] in Cargo.toml.
# Stdout: one feature name per line (in declaration order).
get_declared_features() {
    local cargo_toml="$PORT_PATH/Cargo.toml"
    if [[ ! -f "$cargo_toml" ]]; then
        err "Cargo.toml not found at $cargo_toml"
        return 1
    fi
    awk '
        /^\[features\]/ { in_features = 1; next }
        /^\[/           { in_features = 0; next }
        in_features && /^[[:space:]]*[A-Za-z0-9_-]+[[:space:]]*=/ {
            # extract everything up to the first =
            sub(/[[:space:]]*=.*$/, "")
            # strip leading whitespace
            sub(/^[[:space:]]*/, "")
            print
        }
    ' "$cargo_toml"
}

# --------------------------------------------------------------------------
# Sub-check 1: required umbrellas (T003)
# --------------------------------------------------------------------------

check_required_umbrellas() {
    local cargo_toml="$PORT_PATH/Cargo.toml"
    if [[ ! -f "$cargo_toml" ]]; then
        err "Cargo.toml not found at $cargo_toml"
        return 2
    fi

    local port_name
    port_name="$(get_port_name)"
    if [[ -z "$port_name" ]]; then
        err "could not extract [package].name from $cargo_toml"
        return 2
    fi

    local port_stem
    port_stem="$(get_port_stem "$port_name")"

    local declared
    declared="$(get_declared_features)"

    # Accept either `<port_name>-classic` (full crate name prefix, e.g.
    # `rusty-figlet-classic`) OR `<port_stem>-classic` (bare tool stem
    # prefix, e.g. `figlet-classic`) per the portfolio convention. The
    # bare-stem form is preferred because it matches the upstream tool
    # name and the `project-instructions.md` §Cargo Feature Surface
    # canonical examples (`<port>-classic` where `<port>` is the bare
    # tool stem).
    local violations=0
    local required
    for required in "${REQUIRED_UMBRELLAS_BASE[@]}"; do
        if ! printf '%s\n' "$declared" | grep -Fxq -- "$required"; then
            err "required umbrella '$required' missing from Cargo.toml [features]"
            violations=$((violations + 1))
        fi
    done
    if ! printf '%s\n' "$declared" | grep -Fxq -- "${port_stem}-classic" \
        && ! printf '%s\n' "$declared" | grep -Fxq -- "${port_name}-classic"; then
        err "required umbrella '${port_stem}-classic' (or '${port_name}-classic') missing from Cargo.toml [features]"
        violations=$((violations + 1))
    fi

    if [[ $violations -gt 0 ]]; then
        return 2
    fi
    return 0
}

# --------------------------------------------------------------------------
# Sub-check 2: leaf-CI-matrix entries (T004)
# --------------------------------------------------------------------------

# Heuristic: a feature is a "leaf" if it is NOT one of the required umbrellas
# and NOT a recognized preset bundle. We do not have a global preset-bundle
# registry, so we treat any feature whose name starts with <port>- as a
# port-scoped umbrella (e.g., figlet-classic, figlet-compat, figlet-minimal)
# rather than a leaf. Per FR-007 preset bundles SHOULD share the <port>- prefix.

is_umbrella_or_bundle() {
    local feat="$1"
    local port_name="$2"
    local port_stem
    port_stem="$(get_port_stem "$port_name")"
    case "$feat" in
        default|full|cli) return 0 ;;
        "${port_name}-"*) return 0 ;;
        "${port_stem}-"*) return 0 ;;
        *) return 1 ;;
    esac
}

# Returns 0 if $1 is a recognized dev-tooling feature name (outside the
# leaf-naming convention's purview — typically gates only benches/ or
# tests/ helper code, not library/CLI surface).
#
# These names are common across Rust crates and a non-portfolio convention
# in their own right. Skipped from leaf-CI-matrix and phantom-leaf checks
# since they don't represent a user-visible capability leaf.
is_dev_tooling_feature() {
    local feat="$1"
    case "$feat" in
        bench|benchmark|bench-internal|internal-bench) return 0 ;;
        test-util|test-utils|test-helpers|test-helper) return 0 ;;
        dev-helpers|dev-helper|dev-utils|dev-util) return 0 ;;
        internal|unstable|nightly) return 0 ;;
        *) return 1 ;;
    esac
}

check_leaf_ci_matrix() {
    local ci_yml="$PORT_PATH/.github/workflows/ci.yml"
    if [[ ! -f "$ci_yml" ]]; then
        err "ci.yml not found at $ci_yml"
        return 2
    fi

    local port_name
    port_name="$(get_port_name)"

    local violations=0
    local feat
    while IFS= read -r feat; do
        [[ -z "$feat" ]] && continue
        if is_umbrella_or_bundle "$feat" "$port_name"; then
            continue
        fi
        if is_dev_tooling_feature "$feat"; then
            continue
        fi
        # Look for "check-leaf-<leaf>" as a job name or matrix include.
        if ! grep -Fq -- "check-leaf-${feat}" "$ci_yml"; then
            err "leaf '$feat' has no CI matrix entry (expected check-leaf-${feat})"
            violations=$((violations + 1))
        fi
    done < <(get_declared_features)

    if [[ $violations -gt 0 ]]; then
        return 2
    fi
    return 0
}

# --------------------------------------------------------------------------
# Sub-check 3: phantom-leaf source-gate (T005)
# --------------------------------------------------------------------------

check_phantom_leaf() {
    local src_dir="$PORT_PATH/src"
    if [[ ! -d "$src_dir" ]]; then
        err "src/ directory not found at $src_dir"
        return 2
    fi

    local port_name
    port_name="$(get_port_name)"

    # Search src/ + benches/ + tests/ + examples/ for cfg gates. A leaf
    # feature is considered "real" if it gates code in ANY of these
    # locations, not just src/. Dev-tooling features (bench, test-util,
    # etc.) are skipped entirely via is_dev_tooling_feature.
    local search_dirs=("$src_dir")
    for extra_dir in "$PORT_PATH/benches" "$PORT_PATH/tests" "$PORT_PATH/examples"; do
        if [[ -d "$extra_dir" ]]; then
            search_dirs+=("$extra_dir")
        fi
    done

    local violations=0
    local feat
    while IFS= read -r feat; do
        [[ -z "$feat" ]] && continue
        if is_umbrella_or_bundle "$feat" "$port_name"; then
            continue
        fi
        if is_dev_tooling_feature "$feat"; then
            continue
        fi
        # Search for #[cfg(feature = "<leaf>")] or cfg_attr variants under
        # any of the source directories. Pattern intentionally loose to
        # accept single/double quotes and whitespace variation.
        if ! grep -rE "cfg(_attr)?\s*\(\s*(any\s*\(\s*)?feature\s*=\s*\"${feat}\"" "${search_dirs[@]}" >/dev/null 2>&1; then
            err "leaf '$feat' is a phantom — declared in Cargo.toml but no #[cfg(feature)] gate found in src/, benches/, tests/, or examples/"
            violations=$((violations + 1))
        fi
    done < <(get_declared_features)

    if [[ $violations -gt 0 ]]; then
        return 2
    fi
    return 0
}

# --------------------------------------------------------------------------
# Sub-check 4: README feature-matrix sync (T006)
# --------------------------------------------------------------------------

check_readme_matrix() {
    local readme="$PORT_PATH/README.md"
    if [[ ! -f "$readme" ]]; then
        err "README.md not found at $readme"
        return 2
    fi

    # Locate "## Cargo Features" heading.
    if ! grep -Fq -- "## Cargo Features" "$readme"; then
        err "README 'Cargo Features' section heading missing"
        return 2
    fi

    # Extract the matrix table between the heading and the next H2 heading.
    # The matrix table MUST contain the canonical column order:
    #   | Feature | Description | Umbrella(s) |
    local section
    section="$(awk '
        /^## Cargo Features/ { in_section = 1; next }
        /^## / && in_section { in_section = 0 }
        in_section { print }
    ' "$readme")"

    # Look for the canonical header row. Use a loose regex (case-insensitive,
    # tolerate extra whitespace).
    if ! printf '%s\n' "$section" | grep -Eiq '^\|[[:space:]]*Feature[[:space:]]*\|[[:space:]]*Description[[:space:]]*\|[[:space:]]*Umbrella'; then
        err "README 'Cargo Features' feature-matrix header missing or wrong column order (expected: Feature | Description | Umbrella(s))"
        return 2
    fi

    # Verify every leaf declared in Cargo.toml appears as a row in the section.
    local port_name
    port_name="$(get_port_name)"

    local violations=0
    local feat
    while IFS= read -r feat; do
        [[ -z "$feat" ]] && continue
        if is_umbrella_or_bundle "$feat" "$port_name"; then
            continue
        fi
        # Each leaf MUST appear in the section (most commonly inside backticks
        # or at the start of a table row cell).
        if ! printf '%s\n' "$section" | grep -Fq -- "$feat"; then
            err "README 'Cargo Features' feature-matrix out of sync with Cargo.toml [features] (leaf '$feat' not listed)"
            violations=$((violations + 1))
        fi
    done < <(get_declared_features)

    if [[ $violations -gt 0 ]]; then
        return 2
    fi
    return 0
}

# --------------------------------------------------------------------------
# Sub-check 5: CHANGELOG migration-table exhaustiveness (T007)
# --------------------------------------------------------------------------

check_changelog_migration() {
    local changelog="$PORT_PATH/CHANGELOG.md"
    if [[ ! -f "$changelog" ]]; then
        err "CHANGELOG.md not found at $changelog"
        return 2
    fi

    # Locate the [0.2.0] heading. If the port is pre-v0.2.0 (still at v0.1.x),
    # the migration table is not yet required — skip this sub-check.
    if ! grep -Eq '^## \[0\.2\.0\]' "$changelog"; then
        # Pre-v0.2.0: nothing to check yet. Compliant by definition.
        return 0
    fi

    # Extract the [0.2.0] section.
    local section
    section="$(awk '
        /^## \[0\.2\.0\]/ { in_section = 1; next }
        /^## \[/ && in_section { in_section = 0 }
        in_section { print }
    ' "$changelog")"

    # Additive-only v0.2.0 releases are valid: they extend the feature
    # surface (adding `full`, `<port>-classic`, preset bundles, leaves)
    # without renaming or removing existing v0.1.x features. Such releases
    # don't require a `### BREAKING-CHANGE` subsection or migration table.
    # The CHANGELOG signals this by tagging the Added subheading with
    # "additive" or by having no `### BREAKING-CHANGE` subsection at all.
    if printf '%s\n' "$section" | grep -Eiq '^### Added.*additive|additive only|additive-only'; then
        # Additive-only v0.2.0: migration table is N/A. Compliant.
        return 0
    fi

    # Locate the BREAKING-CHANGE subheading. Required only when v0.2.0
    # changed feature names/semantics in non-additive ways.
    if ! printf '%s\n' "$section" | grep -Eq '^### BREAKING-CHANGE'; then
        # No BREAKING-CHANGE section AND no additive marker. Treat as
        # additive-only by default (the safer assumption).
        return 0
    fi

    # Verify the migration-table header has the canonical column order:
    #   | Old name (v0.1.x) | New name (v0.2.0) | Notes |
    if ! printf '%s\n' "$section" | grep -Eiq '^\|[[:space:]]*Old name[[:space:]]*\(v0\.1\.x\)[[:space:]]*\|[[:space:]]*New name[[:space:]]*\(v0\.2\.0\)[[:space:]]*\|[[:space:]]*Notes'; then
        err "CHANGELOG migration table header missing or wrong column order (expected: Old name (v0.1.x) | New name (v0.2.0) | Notes)"
        return 2
    fi

    # Optional: compare against a v0.1.x baseline Cargo.toml if one is
    # explicitly provided via the V01X_CARGO_TOML env var. We do NOT
    # auto-resolve via 'git show' here because the CI checkout may be
    # shallow and the depth-required git history may not be present.
    if [[ -n "${V01X_CARGO_TOML:-}" && -f "$V01X_CARGO_TOML" ]]; then
        local v01x_features
        v01x_features="$(
            awk '
                /^\[features\]/ { in_features = 1; next }
                /^\[/           { in_features = 0; next }
                in_features && /^[[:space:]]*[A-Za-z0-9_-]+[[:space:]]*=/ {
                    sub(/[[:space:]]*=.*$/, "")
                    sub(/^[[:space:]]*/, "")
                    print
                }
            ' "$V01X_CARGO_TOML"
        )"

        local violations=0
        local old_feat
        while IFS= read -r old_feat; do
            [[ -z "$old_feat" ]] && continue
            if ! printf '%s\n' "$section" | grep -Fq -- "$old_feat"; then
                err "CHANGELOG migration table missing row for v0.1.x feature '$old_feat'"
                violations=$((violations + 1))
            fi
        done <<< "$v01x_features"

        if [[ $violations -gt 0 ]]; then
            return 2
        fi
    fi

    return 0
}

# --------------------------------------------------------------------------
# Dispatcher
# --------------------------------------------------------------------------

usage() {
    cat >&2 <<'EOF'
Usage: lint.sh [--check <name>]

  --check <name>   Run only the named sub-check. Valid names:
                     required-umbrellas
                     leaf-ci-matrix
                     phantom-leaf
                     readme-matrix
                     changelog-migration
                   With no --check flag, all sub-checks are run.

Environment:
  PORT_PATH        (required) absolute path to per-port repo root
  UMBRELLA_PATH    (required) absolute path to umbrella repo root
  STRICT_MODE      (optional, default 1) 0 = warn-only

Exit codes:
  0  compliance
  2  at least one violation
EOF
}

run_check_by_name() {
    case "$1" in
        required-umbrellas)   check_required_umbrellas ;;
        leaf-ci-matrix)       check_leaf_ci_matrix ;;
        phantom-leaf)         check_phantom_leaf ;;
        readme-matrix)        check_readme_matrix ;;
        changelog-migration)  check_changelog_migration ;;
        *)
            err "unknown --check value: $1"
            usage
            return 2
            ;;
    esac
}

run_all_checks() {
    local final_rc=0
    local rc

    check_required_umbrellas
    rc=$?
    [[ $rc -gt $final_rc ]] && final_rc=$rc

    check_leaf_ci_matrix
    rc=$?
    [[ $rc -gt $final_rc ]] && final_rc=$rc

    check_phantom_leaf
    rc=$?
    [[ $rc -gt $final_rc ]] && final_rc=$rc

    check_readme_matrix
    rc=$?
    [[ $rc -gt $final_rc ]] && final_rc=$rc

    check_changelog_migration
    rc=$?
    [[ $rc -gt $final_rc ]] && final_rc=$rc

    return $final_rc
}

# Only execute the dispatcher when invoked directly (not when sourced by run.sh).
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    if [[ $# -eq 0 ]]; then
        run_all_checks
        rc=$?
    elif [[ "$1" == "--check" && $# -eq 2 ]]; then
        run_check_by_name "$2"
        rc=$?
    else
        usage
        exit 2
    fi

    if [[ "$STRICT_MODE" != "1" && $rc -ne 0 ]]; then
        err "violations found but STRICT_MODE=0 — exiting 0"
        exit 0
    fi
    exit $rc
fi
