//! Test-only instrumentation. Not part of the enforcement path: nothing
//! here reads or influences an ALLOW/DENY Decision — it only records
//! what already happened, so a test process outside the governed one can
//! observe facts a return code alone can't prove, chiefly: *did the real
//! `open`/`openat` actually run for this path, or not*.
//!
//! A no-op unless `POLLUX_POC_AUDIT_LOG` is set — production use of this
//! interposer, if it had one, would not set it.

use std::ffi::CString;
use std::os::raw::c_void;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::sys;

static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

fn log_path() -> Option<&'static PathBuf> {
    LOG_PATH
        .get_or_init(|| std::env::var_os("POLLUX_POC_AUDIT_LOG").map(PathBuf::from))
        .as_ref()
}

/// Appends one line to the audit log, if configured.
///
/// Opens the log file via `sys::OPEN` called directly by name, with the
/// real, correctly-stack-passed `mode` argument a proper variadic call
/// requires here (see `sys`'s module docs) — the same real passthrough
/// mechanism [`crate::interpose::poc_open`] itself uses to reach the
/// real `open` rather than this crate's own replacement, so writing an
/// audit line is never itself mistaken for, or evaluated as, a governed
/// filesystem operation.
pub fn log(line: &str) {
    let Some(path) = log_path() else { return };
    let Ok(cpath) = CString::new(path.to_string_lossy().into_owned()) else {
        return;
    };

    // SAFETY: `cpath` is a valid, NUL-terminated C string for this call;
    // the returned `fd`, if non-negative, is a freshly opened file this
    // function alone owns until it closes it a few lines down.
    let fd = unsafe {
        sys::OPEN(
            cpath.as_ptr(),
            sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND,
            0o644u32,
        )
    };
    if fd < 0 {
        return;
    }

    let mut full = String::with_capacity(line.len() + 1);
    full.push_str(line);
    full.push('\n');
    // SAFETY: `fd` was just opened above and is not used concurrently by
    // any other code in this process; `full` is a valid buffer of its
    // own stated length for the duration of the call.
    unsafe {
        sys::write(fd, full.as_ptr() as *const c_void, full.len());
        sys::close(fd);
    }
}
