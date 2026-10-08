//! `pollux-fs-interpose-poc` — macOS-only proof of concept: intercept a
//! real `open()`/`openat()` via `DYLD_INSERT_LIBRARIES` interposition and
//! gate the real syscall behind a live Pollux Core ALLOW/DENY decision,
//! reached through the existing, unmodified
//! `pollux-polyglot-native-bridge` → `pollux-core v0.1.0` path:
//!
//! ```text
//! interposer -> pollux-polyglot-native-bridge -> remote pollux-core v0.1.0
//! ```
//!
//! # This is not a security boundary
//!
//! `DYLD_INSERT_LIBRARIES` interposition can be bypassed by the governed
//! process itself: unsetting the environment variable, calling a raw
//! syscall directly, or running as a hardened-runtime signed binary (for
//! which macOS ignores `DYLD_INSERT_LIBRARIES` unless the target opts
//! in) all defeat it. This crate exists only to prove that a real
//! filesystem open *can* be placed behind a real Core Decision, and that
//! DENY actually prevents the real `open`/`openat` from running — not to
//! provide production enforcement. See `README.md` for the full
//! threat/bypass discussion.
//!
//! # No authorization logic here
//!
//! This crate defines no `Capability`, `Resource`, `Grant`, or
//! `Decision` type of its own. [`operation`] builds the exact
//! `OperationWire` JSON shape Core's own wire protocol
//! (`pollux-protocol/1`) already defines and reads the `outcome` field
//! Core already put in its trace — it does not compute one. Every
//! ALLOW/DENY here is Core's answer, obtained through
//! `pollux_polyglot_native_bridge::PolluxEngine::evaluate`, unmodified.
//!
//! # Scope
//!
//! Only `open`/`openat` are interposed, both handled identically as
//! `filesystem.read` requests regardless of `flags` (see
//! [`operation::build_read_operation`]'s docs for why). No other libc
//! function is intercepted, no child-process propagation is attempted
//! beyond whatever `DYLD_INSERT_LIBRARIES` environment inheritance
//! already gives for free, and no anti-bypass hardening is implemented.

mod audit;
mod decision;
mod engine;
mod guard;
mod interpose;
mod operation;
mod sys;

pub use engine::EngineState;
pub use guard::is_in_internal_call;
