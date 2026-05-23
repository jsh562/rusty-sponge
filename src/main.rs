//! `rusty-sponge` binary entry point. Thin wrapper around [`rusty_sponge::run`].

fn main() -> std::process::ExitCode {
    rusty_sponge::run()
}
