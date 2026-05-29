# rusty-sponge — CI Runtime Baseline

**Status**: v0.2.0 baseline projection (E011 Phase 4 T055).

**Authority**:
- Spec 00011 SC-010 — wall-clock for the longest-path job MUST stay
  under 25 minutes (HARD gate per Clarifications Q1).
- HINT-010 of spec 00011 — remediation strategies if the gate is
  approached.

## Method

rusty-sponge is a single-capability port per spec 00011 §Scope Edge Cases.
The CI matrix scales with the leaf count; since zero leaves are carved,
Tier 4 (`check-leaf-<leaf>`) is empty, leaving a smaller-than-average
matrix vs the reference rusty-figlet port (which has 5 v0.2.0 leaves and
5 corresponding Tier 4 jobs).

The full v0.2.0 CI matrix for rusty-sponge has these jobs:

| Tier | Job name | OS | Count | Notes |
|------|----------|-----|------:|-------|
| Pre-gates | fmt, clippy, audit, msrv | Linux | 4 | Identical to v0.1.x. |
| Tier 1 | test-default | linux x86_64 + aarch64 + macos x86_64 + macos aarch64 + windows x86_64 | 5 | Full DDR-003 cross-compile matrix (preserves v0.1.x coverage). The Linux x86_64 entry also runs the SC-002 default-install smoke. |
| Tier 2 | test-no-default | Linux x86_64 | 1 | Bare library + SC-001 dep-tree audit. |
| Tier 3 | test-sponge-classic, test-sponge-minimal | Linux x86_64 | 2 | Preset bundle install + SC-003 size check. |
| Tier 3 (SC-004) | test-keeplist | Linux x86_64 | 1 | Keep-list workaround install smoke. |
| Tier 4 | (none) | — | 0 | **Zero leaves carved — Tier 4 intentionally empty per docs/feature-layout.md.** |
| Tier 5 | lint-convention | Linux x86_64 | 1 | Vendored `tools/feature-lint/run.sh` invocation. |
| Optional | convention-lint-self-test | Linux x86_64 | 1 | `workflow_dispatch` only — does NOT run on PR/main. |
| Legacy | library-no-default-features | Linux x86_64 | 1 | Retained v0.1.x parity gate. |
| Publish | publish-dry-run | Linux x86_64 | 1 | Runs after every tier above; cargo publish --dry-run. |

**Total scheduled jobs per PR/main push**: 4 (pre-gates) + 5 (Tier 1) + 1
(Tier 2) + 2 (Tier 3) + 1 (SC-004) + 1 (Tier 5) + 1 (legacy) + 1
(publish-dry-run) = **16 jobs**.

The `convention-lint-self-test` job runs only on `workflow_dispatch`
and is excluded from the steady-state PR/main load.

## Wall-clock projection

The longest single-job wall-clock is expected to be the `test-default`
matrix entry running `aarch64-unknown-linux-gnu` under `cross`
(cross-compile via QEMU adds ~30-50% on cargo-build wall vs native), or
the Windows x86_64 entry (Windows runners are typically the slowest in
GitHub-hosted matrix). rusty-sponge has a fault-injection test
(`tests/atomic_safety.rs::linux_sigkill`) that is `#[ignore]`'d at
v0.1.0, so the default CI test pass does not exercise it.

Per the v0.1.0 baseline (single matrix entry per target, no Cargo
Features Convention overlays):

| Target | v0.1.0 wall-clock (median of 3 runs, projected) |
|--------|---------------------------------------------:|
| linux x86_64 (native) | ~3 min |
| linux aarch64 (cross via QEMU) | ~6-8 min |
| macos x86_64 | ~3 min |
| macos aarch64 | ~3 min |
| windows x86_64 | ~5-7 min |

The v0.2.0 matrix adds 5 Tier 2/3/SC-004/5 Linux x86_64 jobs that all
run in parallel with Tier 1. Each is bounded by a single Linux
`cargo build/test --features <bundle>` cycle (~2-3 min each on a warm
rust-cache). Net effect on the longest-path wall-clock: negligible
(the critical path remains the slowest Tier 1 cross-compile).

**Projected longest-path wall-clock for v0.2.0**: ~8-10 minutes
(aarch64-linux cross via QEMU), well under the 25-minute HARD gate
per SC-010.

## Empirical capture (deferred)

Per HINT-010 the empirical capture (3 full CI runs, median of the
slowest matrix entry) is deferred until the v0.2.0 PR opens its first
CI run. The projection above is well under the 25-minute gate; if the
first 3 CI runs show otherwise, this document is updated with the
empirical numbers and remediation steps per HINT-010(a/b/c).

## Remediation strategies (HINT-010, not yet needed)

If the empirical wall-clock approaches the 25-minute gate:

1. **Cache aggressively** — the existing `Swatinem/rust-cache@v2`
   keyed by target already does this; verify cache hit-rate.
2. **Drop `cross` for aarch64-linux** — use `actions/setup-qemu` +
   native arm runners if available, or accept the cross-compile-only
   (no test) for that target.
3. **Move per-preset SC-003 install + size check to a single
   matrix-driven job** — currently each Tier 3 job redundantly
   compiles the binary. A single matrix-strategy job could
   parallelize across preset bundles with one shared rust-cache.

None of these are needed at v0.2.0; capturing here for future
maintenance.
