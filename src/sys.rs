//! Hand-rolled, minimal `extern "C"` declarations for exactly the libc
//! surface this PoC needs — deliberately not the `libc` crate.
//!
//! # Two different `open`/`openat` types, on purpose
//!
//! `open`'s real signature is variadic (`int open(const char *, int,
//! ...)`), and on this target's ABI (Apple AArch64) the variadic tail —
//! `mode`, when present — is passed on the **stack**, not in a register,
//! unlike a normal fixed positional argument. An earlier version of this
//! PoC declared `open`/`openat` as plain fixed-arity functions
//! (`fn(path, flags, mode: c_uint)`) for both directions; that silently
//! corrupted `mode` when *this crate itself* called through with
//! `O_CREAT` (confirmed by an isolated repro: a fixed-arity declaration
//! produced a file with mode `----------`, a properly variadic
//! declaration produced the requested `-rw-r--r--`).
//!
//! So: [`OPEN`]/[`OPENAT`] below are declared **variadic**, matching the
//! real signature, and are the only ones ever *called* by this crate
//! (always with the real, correct number of arguments for the situation:
//! two for a read, three when this crate's own audit log needs
//! `O_CREAT`). [`OpenFn`]/[`OpenAtFn`] are a separate, fixed-arity type
//! used **only** for `poc_open`/`poc_openat`'s own signature in
//! `interpose.rs` — required because Rust cannot *define* (be the callee
//! target of) a variadic `extern "C"` function on stable. Consequently
//! this PoC never reads a caller-supplied `mode` faithfully in the
//! intercepted direction — see `interpose.rs` and the crate's
//! `README.md` limitations section for why that is fine for a
//! read-only-governed PoC and does not affect its actual claim.

use std::os::raw::{c_char, c_int, c_uint, c_void};

pub const O_WRONLY: c_int = 0x0001;
pub const O_CREAT: c_int = 0x0200;
pub const O_APPEND: c_int = 0x0008;
pub const EACCES: c_int = 13;

/// Fixed-arity signature for `poc_open` itself (see this module's docs
/// for why `open`'s real, variadic signature can't be used here).
pub type OpenFn = unsafe extern "C" fn(*const c_char, c_int, c_uint) -> c_int;
/// Fixed-arity signature for `poc_openat` itself; same reasoning.
pub type OpenAtFn = unsafe extern "C" fn(c_int, *const c_char, c_int, c_uint) -> c_int;

extern "C" {
    /// The real, variadic `open` — taken by address for the
    /// `__interpose` entry's `replacee` field, and called directly (with
    /// however many arguments the situation actually needs) as the real
    /// passthrough. See this module's docs for why this must stay
    /// variadic rather than match [`OpenFn`].
    #[link_name = "open"]
    pub fn OPEN(path: *const c_char, flags: c_int, ...) -> c_int;
    /// See `OPEN`'s doc — same rule for `openat`.
    #[link_name = "openat"]
    pub fn OPENAT(dirfd: c_int, path: *const c_char, flags: c_int, ...) -> c_int;

    pub fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    pub fn close(fd: c_int) -> c_int;
    pub fn __error() -> *mut c_int;
}
