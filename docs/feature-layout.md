# rusty-sponge — v0.2.0 Feature Layout

**Status**: implementation draft for the v0.2.0 Cargo features convention
backfill (spec 00011, Phase 4 — rusty-sponge).

**Authority**:
- `specs/adrs/0006-cargo-features-convention-for-portfolio-ports.md` (why)
- `project-instructions.md` §Cargo Feature Surface (what)
- This document — the per-port carving + WHY for each leaf, per HINT-003
  + HINT-009 of spec 00011.

**Reference port**: rusty-figlet v0.2.0 (commit b493d57) — see
`rusty-figlet/docs/feature-layout.md` (FROZEN reference port) for the format
anchor. rusty-sponge conforms to the same shape with the minimum-convention
surface dictated by its single-capability scope.

**Iteration model**: v0.2.0 is a **purely additive** SemVer-minor release.
Every v0.1.x feature name and composition is preserved verbatim; new
umbrellas (`full`, `sponge-classic`, `sponge-minimal`) are layered on top
without renaming or narrowing the existing `cli` / `default` / `sponge-alias`
/ `bench` features. Library and binary API surfaces are unchanged.

## Single-capability port — spec 00011 §Scope Edge Cases

rusty-sponge is a **single-capability port**: it has exactly one documented
capability — soak up stdin and write it atomically to a file (a Rust port
of moreutils `sponge`). Spec 00011 §Scope Edge Cases dictates that
single-capability ports apply the **minimum convention**:

> ports with only one capability adopt the minimum convention:
> `full = ["cli"]` and `<port>-classic = ["cli"]` are the required
> umbrellas; ZERO leaves carved beyond those required umbrellas.

This document records the carving exercise and the explicit decision
to NOT split orthogonal sub-capabilities into leaves — every additional
behavior of `rusty-sponge` (Default-mode ergonomics, Strict moreutils
compat, atomic-rename path, signal-driven cleanup, spill-to-tempfile,
`-a` append mode, `completions` subcommand) is part of the single core
capability surface and removing any of them would break either the
documented public CLI / library contract or the atomic-safety guarantee
that is the entire raison d'être of the tool.

## Source-tree walk

`src/` modules (v0.1.0, post-Phase-1 baseline):

| Module                | Always-on? | CLI-only deps                                       | Notes                                                                       |
|-----------------------|-----------:|-----------------------------------------------------|-----------------------------------------------------------------------------|
| `error.rs`            | yes        | (thiserror — always-on)                             | `Error` enum; library + binary need it.                                     |
| `buffer.rs`           | yes        | (tempfile — always-on)                              | Hybrid in-memory + tempfile-spill buffer engine.                            |
| `atomic.rs`           | yes        | (tempfile — always-on)                              | Sibling-tempfile + atomic rename path. The headline atomic-safety promise. |
| `writethrough.rs`     | yes        | (tempfile — always-on)                              | Non-atomic write-through path for symlink / reparse targets (FR-010).      |
| `lib.rs`              | yes        | none                                                | Public API (`SpongeBuilder`, `Sponge`, `Target`, `CompatibilityMode`).      |
| `cli.rs`              | no — `cli` | clap                                                | clap-derive `Cli` struct + `Subcommand::Completions`.                       |
| `mode.rs`             | no — `cli` | none (pure helper)                                  | Strict-mode precedence resolver (`--strict` > env > argv[0]).               |
| `signal.rs`           | no — `cli` | signal-hook (Unix), windows-sys (Windows)           | Signal handler install + cleanup-on-exit dispatch.                          |
| `strict.rs`           | no — `cli` | (clap_complete + clap pulled by `cli`)              | Hand-rolled Strict-mode argv pre-scanner + byte-equal moreutils dispatcher.|
| `main.rs`             | no — `cli` | clap, clap_complete, anyhow, signal-hook            | Binary entry; gated by `required-features = ["cli"]`.                       |
| `bin/sponge.rs`       | no — `sponge-alias` | (inherits `cli`)                            | `sponge` alias binary; gated by `required-features = ["sponge-alias"]`.     |

## Leaf-carving criteria (HINT-009)

A capability becomes a leaf when ALL of the following hold:

1. It is **self-containable** — gated cleanly via `#[cfg(feature = "<leaf>")]`
   at the module or top-level item boundary (HINT-004).
2. Either (a) it has a **sole optional dependency** that no other leaf needs
   (HINT-005), OR (b) it is a pure-cfg-gate of an internal module worth
   exposing as a knob.
3. Disabling it does NOT break any always-on library/CLI surface.

A capability does NOT become a leaf when:

- It is foundational (atomic-rename, spill buffer, writethrough fallback)
  — disabling it would break the headline atomic-safety guarantee.
- It is part of the single documented capability surface (Default mode,
  Strict mode, signal cleanup, `-a` append, completions subcommand).
- It would create more than ~6 leaves (FR-007 + HINT-003 envelope).

## v0.2.0 Carved Leaves

**ZERO new leaves carved at v0.2.0**. Every capability inside rusty-sponge
is either:

1. Foundational always-on library code (atomic-rename procedure, spill
   buffer, writethrough fallback) — cannot be stripped without breaking
   the public surface or the atomic-safety guarantee.
2. Already gated by the v0.1.x `cli` umbrella (clap-derived argument
   parsing, completions subcommand, signal handler install, Strict-mode
   pre-scanner).
3. Already gated by the v0.1.x `sponge-alias` feature (the second `sponge`
   binary entry).
4. A dev-tooling feature (`bench` → criterion benches) outside the
   convention's runtime-capability purview.

### Leaves intentionally NOT carved

The following candidate leaves were considered + rejected:

- **`signal`**: signal handler install + cleanup-on-exit dispatch lives
  in `src/signal.rs` behind `dep:signal-hook` (Unix) and `windows-sys`
  (Windows, target-conditional always-on). It is part of the FR-011
  documented atomic-safety contract — without it, a Ctrl-C mid-write
  could leave a partial tempfile in the target's directory. Stripping
  this would silently break the headline promise. Rejected per
  HINT-009 criterion 3.
- **`completions`**: Could be carved as `["dep:clap_complete"]`, but
  per spec 00011 §Scope Edge Cases minimum-convention single-capability
  ports declare ZERO new leaves. `clap_complete` is bundled into the
  v0.1.x `cli` umbrella verbatim. Carving it would either rename `cli`
  (breaking SemVer additivity) or duplicate the surface.
- **`spill-tempfile`**: The hybrid in-memory + tempfile-spill buffer is
  the only mechanism by which arbitrarily-large inputs are handled
  without exhausting RAM. The `tempfile` dep is always-on library code.
  No carving signal.
- **`strict-compat`**: rusty-sponge's Strict mode is dispatched inline
  in `lib.rs::run()` (via `mode::resolve` + `strict::run`) — about 40
  lines including the argv pre-scanner. The hand-rolled getopt mirror
  lives in `src/strict.rs`. Both are gated by the `cli` umbrella in
  v0.1.x (since they consume `clap` + `clap_complete`). Carving out a
  separate `strict-compat` leaf would require splitting `strict.rs`
  away from `cli.rs`, which is more refactoring than the additive
  v0.2.0 release allows. The capability survives untouched inside
  the existing `cli` composition.
- **`sponge-alias`**: This v0.1.x feature ships a second binary named
  `sponge`. It IS retained verbatim per the v0.2.0 SemVer additive
  contract — but it is NOT one of the 2 required preset bundles per
  FR-007 (those are `sponge-classic` and `sponge-minimal` below).
  Documented separately as an installation-time convenience knob.
- **`bench`**: The v0.1.x `bench` feature is a tooling / benchmark
  scaffold (criterion benches under `benches/throughput.rs`), not a
  runtime capability leaf. It remains a dev-tooling feature outside
  the convention's purview (the vendored `tools/feature-lint/lint.sh`
  allowlist skips `bench` from leaf-CI-matrix and phantom-leaf checks)
  and is retained verbatim from v0.1.0.

## Preset bundles (FR-007 — 2 required for single-capability ports)

Per spec 00011 §Scope Edge Cases + FR-007, even single-capability ports
declare 2 preset bundles to give the keep-list workaround documentation
something concrete to point at.

### `sponge-classic` (REQUIRED — bare port, 1:1 with moreutils `sponge`)

```toml
sponge-classic = ["cli"]
```

- Includes `cli` so the binary exists.
- Single-capability port; the `cli` umbrella IS the bare-port surface.
- Use case: `cargo install rusty-sponge --no-default-features --features sponge-classic`
  for a moreutils-`sponge` drop-in replacement (Strict mode is invoked
  via the `--strict` flag, `RUSTY_SPONGE_STRICT` env var, or `sponge-alias`
  binary name — none of these require additional features).

### `sponge-minimal`

```toml
sponge-minimal = ["cli"]
```

- Same composition as `sponge-classic` (single-capability port has no
  smaller subset to carve).
- Use case: explicit "minimal CLI install" alias for users who prefer
  the `<port>-minimal` naming convention seen across other Rusty ports
  (figlet-minimal, pwgen-minimal, ts-minimal). Documented as an
  intentional semantic alias rather than a distinct composition.

### `sponge-alias` (v0.1.x feature retained, NOT a convention preset)

`sponge-alias = ["cli"]` from v0.1.0 ships an additional `sponge` binary
alongside `rusty-sponge`. It is retained verbatim per the v0.2.0 SemVer
additive contract — but it is NOT one of the 2 required preset bundles
per FR-007 (those are `sponge-classic` and `sponge-minimal` above).
`sponge-alias` is documented separately as an installation-time
convenience knob, not a capability subset.

### `bench` (v0.1.x dev-tooling feature retained, NOT a convention preset)

`bench = ["dep:criterion"]` from v0.1.0 enables `benches/throughput.rs`.
It is dev-tooling, not a runtime capability — outside the convention's
purview per the vendored feature-lint allowlist.

## Cross-port glossary candidates

No leaves carved → no cross-port glossary contributions from rusty-sponge
in this iteration. If a future minor release adds an orthogonal capability
(e.g., a `pidfile` leaf for sponge-as-a-service deployments), the leaf
would be a candidate for promotion to `docs/feature-vocabulary.md` per
FR-053.

## CI matrix shape (FR-010..FR-014)

Per plan §Per-Port v0.2.0 CI Matrix, scaled to a zero-leaf port:

- **Tier 1 — `test-default`**: full DDR-003 cross-compile matrix
  (5 targets). Post-v0.2.0 `default = ["full"]` and `full = ["cli"]`,
  so the kitchen-sink test resolves to the same set as v0.1.0
  `default = ["cli"]` — no regression in coverage.
- **Tier 2 — `test-no-default`**: Linux x86_64 only. `cargo test
  --no-default-features --lib` + dep-tree audit (SC-001 evidence).
- **Tier 3 — `test-<bundle>`**: one job per preset bundle. Linux only.
  - `test-sponge-classic`
  - `test-sponge-minimal`
- **Tier 4 — `check-leaf-<leaf>`**: SKIPPED. Zero leaves → no
  per-leaf compile-check jobs. A placeholder comment in `ci.yml`
  documents why this tier is empty. The `bench` feature is in the
  vendored feature-lint allowlist (dev-tooling) and does not require
  a Tier-4 entry.
- **Tier 5 — `lint-convention`**: single Linux job invoking the
  vendored `tools/feature-lint/run.sh` script.

Per FR-014, bundle/lint jobs are constrained to Linux x86_64.

## Vendored feature-lint

Per spec 00011 §Phase 2 iteration 6 precedent (rusty-figlet vendored
the lint script because the umbrella `jsh562/rustylib` is private and
cross-repo `actions/checkout` cannot reach it), rusty-sponge vendors
`tools/feature-lint/{lint.sh,run.sh,README.md}` from the umbrella into
the port repo. The vendored copy is byte-equal to the umbrella source
of truth as of the freeze commit (post the dev-tooling-allowlist +
benches/tests-search + additive-CHANGELOG-support fixes from rusty-ts
v0.2.0 / E011 Phase 3 iteration 2).

## Why no new leaves — explicit rationale

Spec 00011 §Scope Edge Cases anticipates this case verbatim:

> Some ports have only one orthogonal capability. Those ports adopt the
> minimum convention: `full = ["cli"]` and `<port>-classic = ["cli"]`
> as aliases; the convention SHAPE is consistent across the portfolio
> even when the per-port leaf carving yields zero leaves.

rusty-sponge deliberately chooses the zero-leaf path because:

1. The atomic-safety guarantee is the entire reason this tool exists.
   Carving any of its supporting machinery (signal handlers, spill
   buffer, atomic rename, writethrough fallback) into an opt-out leaf
   would silently change the FR-006 contract for users who turned that
   leaf off.
2. The cost of carving a speculative leaf (cfg-gate scaffolding,
   per-leaf CI matrix entry, README/CHANGELOG row, glossary candidacy)
   outweighs the value when no orthogonal capability exists to gate.
3. The portfolio-wide convention shape (umbrella set, README "Cargo
   Features" section, lint compliance) is preserved verbatim — a
   downstream library consumer reading the README for rusty-sponge
   gets the same one-glance feature matrix UX as one reading
   rusty-figlet or rusty-ts.
4. v0.2.0 is **purely additive**. Every v0.1.x feature is preserved
   verbatim; no SemVer break. Future minor releases can add leaves
   without breaking the v0.2.0 contract: a hypothetical `pidfile`
   v0.3.0 feature would slot in as `pidfile = ["dep:atomicwrites"]`
   alongside the existing umbrellas with zero migration cost.
