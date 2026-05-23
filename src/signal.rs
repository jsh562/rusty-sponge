//! Signal-driven cleanup so an in-progress sibling tempfile is removed before
//! the process exits on SIGINT/SIGTERM/SIGHUP (Unix) or `CTRL_C_EVENT` /
//! `CTRL_BREAK_EVENT` / `CTRL_CLOSE_EVENT` (Windows).
//!
//! Architecture (HINT-003, AD-012/AD-013):
//!   1. The handler is async-signal-safe — it ONLY stores `true` into a
//!      process-wide [`AtomicBool`].
//!   2. The buffer drain loop polls the flag via [`is_cancelled`] between
//!      chunks (see `crate::buffer::Buffer::drain_reader`).
//!   3. On flag set, the read loop returns `io::ErrorKind::Interrupted`. The
//!      error propagates up to caller code which drops the `NamedTempFile`,
//!      removing the on-disk artifact before exit.
//!   4. Uncatchable signals (SIGKILL, Windows process termination) cannot be
//!      cleaned up before exit — `tempfile`-crate `Drop` is the only fallback,
//!      and a hard kill mid-write may leak the tempfile. Documented limit.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Process-wide cancellation flag for signal-driven exit. Set to `true` by
/// signal handlers; polled by [`crate::buffer::Buffer::drain_reader`].
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Reset the process-wide cancel flag (test-only helper).
#[cfg(test)]
pub fn reset_cancel() {
    CANCEL.store(false, Ordering::SeqCst);
}

/// Returns `true` if a registered signal has been delivered.
pub fn is_cancelled() -> bool {
    CANCEL.load(Ordering::SeqCst)
}

/// Reference type still exported for library consumers that want to plug
/// their own cancellation source. The process-wide static is the canonical
/// signal target; this Arc<AtomicBool> is for advanced/embedding cases.
pub type CancelFlag = Arc<AtomicBool>;

/// Construct a fresh, unsignalled cancellation flag (for library embedders
/// who want their own scope-local cancellation).
pub fn cancel_flag() -> CancelFlag {
    Arc::new(AtomicBool::new(false))
}

/// Install platform-appropriate signal handlers that, on receipt of any of
/// the standard "terminate cleanly" signals, store `true` into `flag`.
///
/// On Unix this registers SIGINT, SIGTERM, and SIGHUP via `signal-hook`.
/// On Windows this registers a `SetConsoleCtrlHandler` for `CTRL_C_EVENT`,
/// `CTRL_BREAK_EVENT`, and `CTRL_CLOSE_EVENT`.
///
/// Returns `Ok(())` on success. Errors here are non-fatal in practice — the
/// caller may log them and proceed without signal cleanup; uncatchable
/// signals always fall back to the `tempfile`-crate `Drop` behavior.
#[cfg(unix)]
pub fn install_handlers() -> std::io::Result<()> {
    use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::flag::register;

    // We need an Arc<AtomicBool> for signal-hook's API, but our canonical
    // flag is a static. Bridge: a per-install Arc that, when set, also sets
    // the static. signal-hook stores `true` directly into the Arc — there's
    // no callback hook. So we register against an internal Arc, and our
    // `is_cancelled` polls BOTH the Arc and the static.
    //
    // Simpler: just keep one process-wide Arc inside a OnceLock, and have
    // `is_cancelled` read it.
    use std::sync::OnceLock;
    static BRIDGE_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    let bridge = BRIDGE_FLAG.get_or_init(|| Arc::new(AtomicBool::new(false)));

    register(SIGINT, Arc::clone(bridge))?;
    register(SIGTERM, Arc::clone(bridge))?;
    register(SIGHUP, Arc::clone(bridge))?;

    // Spawn a watcher thread that propagates the Arc-flag into the static.
    // This is because the polling sites only read the static (no Arc plumbing).
    let bridge_clone = Arc::clone(bridge);
    std::thread::Builder::new()
        .name("rusty-sponge-signal-watcher".into())
        .spawn(move || {
            loop {
                if bridge_clone.load(Ordering::SeqCst) {
                    CANCEL.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        })?;

    Ok(())
}

#[cfg(windows)]
pub fn install_handlers() -> std::io::Result<()> {
    use windows_sys::Win32::Foundation::BOOL;
    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
    };

    unsafe extern "system" fn handler(ctrl_type: u32) -> BOOL {
        if matches!(
            ctrl_type,
            CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT
        ) {
            CANCEL.store(true, Ordering::SeqCst);
            // Return TRUE = "we handled it"; the OS continues to the next
            // handler in the chain and ultimately terminates the process.
            // The 5-second close-event grace window is enough for our main
            // thread to observe the flag and drop the tempfile before exit.
            1
        } else {
            0
        }
    }

    // SAFETY: SetConsoleCtrlHandler is FFI; safe to call from any thread.
    let ok = unsafe { SetConsoleCtrlHandler(Some(handler), 1) };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_cancel_flag_starts_unset() {
        reset_cancel();
        assert!(!is_cancelled());
    }

    #[test]
    fn install_handlers_does_not_panic() {
        // The handler-install path should succeed in a normal test process.
        // On Unix this registers real signal handlers — they remain installed
        // for the lifetime of the test process, which is fine because
        // signal-hook's flag-register is idempotent across multiple calls.
        // We do NOT actually raise a signal in this test (would terminate
        // the test process).
        let _ = install_handlers();
        reset_cancel();
        assert!(!is_cancelled(), "no signal raised → flag stays clear");
    }

    #[test]
    fn cancel_flag_factory_still_works_for_embedders() {
        let f = cancel_flag();
        assert!(!f.load(Ordering::SeqCst));
        f.store(true, Ordering::SeqCst);
        assert!(f.load(Ordering::SeqCst));
    }
}
