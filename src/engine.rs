//! Loads the Pollux engine exactly once per process, via the existing,
//! unmodified `pollux_polyglot_native_bridge::PolluxEngine` — this
//! module adds no authorization logic of its own, only a place to hold
//! the one engine instance the whole process reuses.

use std::sync::OnceLock;

use pollux_polyglot_native_bridge::PolluxEngine;

use crate::audit;

pub enum EngineState {
    Ready(PolluxEngine),
    /// Any failure resolving the Manifest or constructing the Core
    /// engine. Deliberately not further subdivided: every case here
    /// means "the governed process must fail closed," and that decision
    /// does not depend on which specific step failed.
    Failed,
}

static ENGINE: OnceLock<EngineState> = OnceLock::new();

/// Returns the process-wide engine, creating it on first call.
///
/// # Must only be called while the reentrancy guard is held
///
/// Reading the Manifest file below performs a real `open`/`openat` call.
/// In this PoC's actual build that call happens to be exempt from this
/// dylib's own interposition (it is compiled into this same image — see
/// `sys.rs`'s and `guard.rs`'s docs) and so never re-enters
/// [`crate::decision::handle`] in practice. That is a property of
/// *where this particular read happens to run*, not a guarantee this
/// function can rely on in general — a future change (or Core's own
/// dylib doing filesystem I/O of its own) could make a call here cross
/// an image boundary and land back in `handle`, which would call back
/// into [`engine`] while `ENGINE.get_or_init` is still running.
/// [`crate::decision::handle`] is the only caller, and it always enters
/// the guard first, so this function must never be called from anywhere
/// else without doing the same.
pub fn engine() -> &'static EngineState {
    ENGINE.get_or_init(|| {
        audit::log("INIT");

        let manifest_path = match std::env::var("POLLUX_POC_MANIFEST") {
            Ok(path) => path,
            Err(_) => {
                audit::log("INIT_FAILED missing POLLUX_POC_MANIFEST");
                return EngineState::Failed;
            }
        };

        let manifest_yaml = match std::fs::read_to_string(&manifest_path) {
            Ok(yaml) => yaml,
            Err(error) => {
                audit::log(&format!("INIT_FAILED reading manifest: {error}"));
                return EngineState::Failed;
            }
        };

        let result = PolluxEngine::load(&manifest_yaml);
        match result {
            Ok(engine) => {
                audit::log("INIT_OK");
                EngineState::Ready(engine)
            }
            Err(error) => {
                audit::log(&format!("INIT_FAILED PolluxEngine::load: {error}"));
                EngineState::Failed
            }
        }
    })
}
