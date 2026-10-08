//! Shared body for both `open()` and `openat()` interposition.
//!
//! Contains no authorization logic: it builds the wire Operation
//! ([`crate::operation::build_read_operation`]), calls the existing,
//! unmodified `PolluxEngine::evaluate`, and only *reads* the `outcome`
//! Core already decided ([`crate::operation::parse_outcome`]). Every
//! branch below that isn't "Core said allow" or "Core said deny" is an
//! infrastructure failure, and every infrastructure failure denies.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

use crate::operation::Outcome;
use crate::{audit, engine, guard, operation, sys};

/// # Safety
///
/// `path` must be either null or a valid, NUL-terminated C string for
/// the duration of this call — guaranteed by `open`/`openat`'s own
/// contract, which every caller already satisfies before reaching the
/// interposed symbol.
pub unsafe fn handle(path: *const c_char, real: impl FnOnce() -> c_int) -> c_int {
    if path.is_null() {
        audit::log("DENY <null-path>");
        return deny();
    }

    // SAFETY: caller's contract, restated on this function's own doc.
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(path) => path,
        Err(_) => {
            audit::log("DENY <invalid-utf8-path>");
            return deny();
        }
    };

    if guard::is_in_internal_call() {
        // Reached only for filesystem opens Pollux's own evaluation
        // machinery makes on this thread (today: reading the Manifest
        // file during engine init) — never for an application-initiated
        // open, which always goes through `decide` below first. See
        // `guard`'s module docs for the exact scope of this bypass.
        audit::log(&format!("REENTRANT_PASS {path}"));
        return call_real(path, real);
    }

    let outcome = {
        let _guard = guard::enter();
        decide(path)
    };

    match outcome {
        Outcome::Allow => {
            audit::log(&format!("ALLOW {path}"));
            call_real(path, real)
        }
        Outcome::Deny => {
            audit::log(&format!("DENY {path}"));
            deny()
        }
    }
}

fn call_real(path: &str, real: impl FnOnce() -> c_int) -> c_int {
    audit::log(&format!("REAL_OPEN {path}"));
    real()
}

/// Consults the real, linked Pollux Core through the unmodified Bridge.
/// Every failure mode — engine never initialized, the ABI call itself
/// failing, or a trace whose shape this crate doesn't recognize — is
/// `Outcome::Deny`. There is no case in which failing to get an answer
/// from Core becomes `Outcome::Allow`.
fn decide(path: &str) -> Outcome {
    match engine::engine() {
        engine::EngineState::Failed => Outcome::Deny,
        engine::EngineState::Ready(pollux_engine) => {
            let operation_json = operation::build_read_operation(path);
            match pollux_engine.evaluate(&operation_json) {
                Ok(trace_json) => operation::parse_outcome(&trace_json).unwrap_or(Outcome::Deny),
                Err(_) => Outcome::Deny,
            }
        }
    }
}

fn deny() -> c_int {
    // SAFETY: `__error()` returns this thread's own errno storage; a
    // plain, always-valid write to it.
    unsafe { *sys::__error() = sys::EACCES };
    -1
}
