//! `sponge` binary alias entry point (gated behind the `sponge-alias` Cargo feature).
//!
//! Shares the same body as [`rusty_sponge::run`]; argv[0] auto-detect inside
//! `run()` routes invocations as `sponge` into Strict mode per FR-020.

fn main() -> std::process::ExitCode {
    rusty_sponge::run()
}
