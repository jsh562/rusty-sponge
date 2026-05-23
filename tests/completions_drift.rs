//! Drift test: regenerate the committed completion scripts in-memory and
//! assert they match the on-disk copies under `completions/`. If clap or
//! the CLI definition changes, this test fails and the user runs
//! `cargo run -- completions <shell> > completions/<file>` to update.
//!
//! This pattern is preferred over `build.rs`-driven file mutation (which
//! would dirty user installs of the crate) — see SAD baseline §"Drift-tested
//! generated documentation".

use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;

fn project_completion(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("completions")
        .join(name)
}

fn generated_for(shell: &str) -> Vec<u8> {
    let mut cmd = Command::cargo_bin("rusty-sponge").expect("binary built");
    let output = cmd
        .arg("completions")
        .arg(shell)
        .output()
        .expect("completions subcommand runs");
    assert!(
        output.status.success(),
        "completions {shell} should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

/// Helper: normalize line endings so the committed CRLF/LF mismatch (which
/// .gitattributes guards against) does not cause spurious failures locally
/// on Windows. The release pipeline regenerates from the host runner and
/// commits LF — but a local checkout on Windows may have CRLF if the
/// .gitattributes rule isn't applied yet (early in repo history).
fn normalize(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().copied().filter(|&b| b != b'\r').collect()
}

#[test]
fn bash_completion_committed_matches_generated() {
    let committed = fs::read(project_completion("rusty-sponge.bash"))
        .expect("committed bash completion missing");
    let generated = generated_for("bash");
    assert_eq!(
        normalize(&committed),
        normalize(&generated),
        "bash completion drift detected — run: cargo run -- completions bash > completions/rusty-sponge.bash"
    );
}

#[test]
fn zsh_completion_committed_matches_generated() {
    let committed =
        fs::read(project_completion("_rusty-sponge")).expect("committed zsh completion missing");
    let generated = generated_for("zsh");
    assert_eq!(
        normalize(&committed),
        normalize(&generated),
        "zsh completion drift detected — run: cargo run -- completions zsh > completions/_rusty-sponge"
    );
}

#[test]
fn fish_completion_committed_matches_generated() {
    let committed = fs::read(project_completion("rusty-sponge.fish"))
        .expect("committed fish completion missing");
    let generated = generated_for("fish");
    assert_eq!(
        normalize(&committed),
        normalize(&generated),
        "fish completion drift detected — run: cargo run -- completions fish > completions/rusty-sponge.fish"
    );
}

#[test]
fn powershell_completion_committed_matches_generated() {
    let committed = fs::read(project_completion("rusty-sponge.ps1"))
        .expect("committed powershell completion missing");
    let generated = generated_for("powershell");
    assert_eq!(
        normalize(&committed),
        normalize(&generated),
        "powershell completion drift detected — run: cargo run -- completions powershell > completions/rusty-sponge.ps1"
    );
}

#[test]
fn completions_subcommand_rejected_in_strict_mode() {
    // FR-018: Strict mode rejects every Default-mode addition.
    // `completions` is one of those additions.
    let output = Command::cargo_bin("rusty-sponge")
        .expect("binary built")
        .args(["--strict", "completions", "bash"])
        .output()
        .expect("strict mode runs");

    // The Strict path takes over BEFORE clap sees `completions`. The
    // `completions` token becomes the first positional → strict mode treats
    // it as a file target. It is NOT recognized as a subcommand, which
    // is the desired rejection behavior. Result: rusty-sponge tries to
    // write stdin to a file literally named "completions", which would
    // succeed (creating the file). The key behavioral assertion is that
    // the subcommand is NOT executed — no shell-completion bytes appear
    // on stdout.
    let stdout_lossy = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout_lossy.contains("_rusty-sponge") && !stdout_lossy.contains("complete -F"),
        "completions subcommand must NOT execute in Strict mode (stdout: {stdout_lossy:?})"
    );
}
