//! The actual `DYLD_INSERT_LIBRARIES` interposition entries.
//!
//! When this dylib is loaded via `DYLD_INSERT_LIBRARIES`, `dyld` scans
//! every `__DATA,__interpose` section (including this one) and redirects
//! calls resolving to each entry's `replacee` address to its
//! `replacement` address instead, for every image loaded in the process.
//! This is Apple's own documented mechanism (`dyld_interpose.h`) — not
//! something this crate invents.
//!
//! **This is not a security boundary.** See the crate root docs and
//! `README.md`: a governed process can unset `DYLD_INSERT_LIBRARIES`,
//! call a raw syscall directly, or otherwise route around this dylib
//! entirely. This module exists only to prove that a real `open`/
//! `openat` call *can* be placed behind a real Core Decision — not to
//! provide production enforcement.

use std::os::raw::{c_char, c_int, c_uint, c_void};

use crate::decision;
use crate::sys::{self, OpenAtFn, OpenFn};

#[repr(C)]
struct Interpose {
    replacement: *const c_void,
    replacee: *const c_void,
}

// SAFETY: both fields are plain addresses derived from `fn item as
// fn-pointer-type as *const c_void` casts, never dereferenced by this
// crate itself — `dyld` reads this section as raw data at load time,
// before any Rust code here runs concurrently with anything.
unsafe impl Sync for Interpose {}

#[used]
#[link_section = "__DATA,__interpose"]
static INTERPOSE_OPEN: Interpose = Interpose {
    replacement: poc_open as OpenFn as *const c_void,
    replacee: sys::OPEN as unsafe extern "C" fn(*const c_char, c_int, ...) -> c_int
        as *const c_void,
};

#[used]
#[link_section = "__DATA,__interpose"]
static INTERPOSE_OPENAT: Interpose = Interpose {
    replacement: poc_openat as OpenAtFn as *const c_void,
    replacee: sys::OPENAT as unsafe extern "C" fn(c_int, *const c_char, c_int, ...) -> c_int
        as *const c_void,
};

/// Replaces every call to `open()` any image in this process makes, once
/// this dylib is loaded via `DYLD_INSERT_LIBRARIES`, with a Pollux
/// Core ALLOW/DENY-gated version.
///
/// # Safety
///
/// Called by `dyld`'s interpose mechanism with exactly the arguments the
/// original caller passed to `open`. `path` is treated as a possibly
/// untrusted, caller-supplied C string pointer — see
/// [`decision::handle`], which is where it is actually read.
#[no_mangle]
pub unsafe extern "C" fn poc_open(path: *const c_char, flags: c_int, _mode: c_uint) -> c_int {
    // `_mode` is deliberately never forwarded: on this target's ABI the
    // variadic tail of a real `open(..., ...)` call is stack-passed, so
    // a value read here as a plain fixed argument cannot be trusted to
    // be what the original caller actually supplied — see `sys`'s
    // module docs. This PoC governs `filesystem.read` only, and a read
    // never needs `O_CREAT`/`mode`; forwarding nothing here is correct
    // for that scope and is a documented limitation outside it.
    guarded_entry(path, move || {
        // SAFETY: forwarding exactly `path`/`flags`, calling the real
        // `open` directly by name — see `sys`'s module docs for why
        // that reaches the real function rather than recursing (not
        // `dlsym(RTLD_NEXT, ...)`, which this PoC tried first and found
        // recurses into this very function instead).
        unsafe { sys::OPEN(path, flags) }
    })
}

/// The `openat` counterpart of [`poc_open`]; same contract.
///
/// # Safety
///
/// Same contract as [`poc_open`], for `openat`'s argument shape.
#[no_mangle]
pub unsafe extern "C" fn poc_openat(
    dirfd: c_int,
    path: *const c_char,
    flags: c_int,
    _mode: c_uint,
) -> c_int {
    // See `poc_open`'s doc for why `_mode` is not forwarded.
    guarded_entry(path, move || {
        // SAFETY: see `poc_open`.
        unsafe { sys::OPENAT(dirfd, path, flags) }
    })
}

/// Never lets a panic anywhere in the decision path unwind across the
/// `extern "C"` boundary into `dyld`/the governed process — catches it
/// and fails closed (`EACCES`) instead. "Fail closed" must hold even for
/// bugs in this crate itself, not only for well-behaved Core errors.
fn guarded_entry(path: *const c_char, real: impl FnOnce() -> c_int) -> c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `path` carries the same validity contract documented
        // on `poc_open`/`poc_openat`, which `decision::handle` relies on.
        unsafe { decision::handle(path, real) }
    }));
    match result {
        Ok(return_code) => return_code,
        Err(_) => {
            crate::audit::log("PANIC fail-closed");
            // SAFETY: see `decision::deny`'s equivalent write.
            unsafe { *sys::__error() = sys::EACCES };
            -1
        }
    }
}
