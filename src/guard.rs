//! A narrow, thread-local reentrancy guard — **not** a global mutex.
//!
//! Loading the Manifest ([`crate::engine::engine`]) and evaluating an
//! Operation both cross the Bridge/Core ABI. If Pollux Core's own code —
//! a genuinely separate Mach-O image, `libpollux_ffi.dylib`, loaded
//! alongside this dylib — ever performs its own filesystem I/O during
//! `pollux_engine_create`/`pollux_engine_evaluate` (logging, caching, a
//! future manifest-file-based API, etc.), that call would cross the same
//! interposed `open`/`openat` symbols this dylib replaces, landing right
//! back in [`crate::decision::handle`] *before* the outer call it
//! originated from has finished — for the very first call, that means
//! recursing into `ENGINE`'s `OnceLock::get_or_init` while it is still
//! running, which is unsound to attempt without a guard against it.
//!
//! (This PoC's own [`crate::engine::engine`] reads the Manifest file via
//! plain `std::fs::read_to_string` — verified empirically *not* to
//! trigger this dylib's own interposition, because that call is compiled
//! into this same dylib and dyld's `__DATA,__interpose` mechanism does
//! not redirect a symbol's own defining image's calls to it — see
//! `sys.rs`'s docs, the same property this crate relies on to call
//! through to the real `open`/`openat` at all. So in the build this PoC
//! actually exercises, nothing has yet been observed to organically
//! trigger this guard's bypass branch at the process level; it remains
//! necessary and correct for the cross-image case described above, and
//! its own logic is proven directly by this module's unit tests.)
//!
//! # What this bypasses
//!
//! Every filesystem open made **on the same thread**, strictly **between
//! [`enter`] and the returned [`Guard`]'s `Drop`** — nothing else. In
//! this PoC that span covers exactly one thing: the single Bridge call
//! [`crate::decision::handle`] makes into `engine()`/`evaluate()` for one
//! governed `open`/`openat`. There is no allowlist of specific paths and
//! no notion of "trusted files" — any open made during that span is
//! passed straight through to the real `open`/`openat`, unevaluated.
//!
//! # Why this cannot become a general bypass
//!
//! [`enter`] is called from exactly one place in this crate
//! ([`crate::decision::handle`], immediately before calling into
//! `engine()`/`evaluate()`) and is not reachable from outside this
//! crate — there is no environment variable, path, file descriptor
//! number, or `open`/`openat` argument a governed process can set to
//! raise this flag itself. It is only ever `true` on the same OS thread,
//! for the synchronous duration of one Bridge call this crate makes on
//! its own initiative; a governed process cannot observe or extend that
//! window, and the flag is always cleared (via `Drop`) before control
//! returns to [`crate::decision::handle`]'s caller.

use std::cell::Cell;

// A depth counter, not a bool: `decide` (outer) holds the span across
// its whole `engine()`/`evaluate()` call, and `engine()`'s own Manifest
// read can itself recurse into `enter` (see this module's docs). A bare
// bool would have the inner span's `Drop` clear the flag while the outer
// span is still logically active; the counter only reaches zero — and
// only then flips `is_in_internal_call` back to `false` — when the
// outermost guard drops.
thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// True while the current thread is inside any span opened by [`enter`],
/// including nested ones.
pub fn is_in_internal_call() -> bool {
    DEPTH.with(|depth| depth.get() > 0)
}

/// RAII guard returned by [`enter`]; decrements the depth counter on
/// drop, including on unwind, so a panic inside the guarded span can
/// never leave the counter stuck above zero.
#[must_use]
pub struct Guard(());

impl Drop for Guard {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

/// Opens the reentrancy-guarded span. See the module docs for exactly
/// what this bypasses and why it is safe to.
pub fn enter() -> Guard {
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    Guard(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn false_outside_true_while_entered_false_again_after_drop() {
        assert!(!is_in_internal_call());
        {
            let _guard = enter();
            assert!(is_in_internal_call());
        }
        assert!(!is_in_internal_call());
    }

    #[test]
    fn nested_enter_stays_true_until_the_outer_guard_drops() {
        assert!(!is_in_internal_call());
        let outer = enter();
        assert!(is_in_internal_call());

        {
            let inner = enter();
            assert!(is_in_internal_call());
            drop(inner);
            // The outer guard is still held: a nested internal call
            // finishing early must not turn authorization back on for
            // the still-in-flight outer one.
            assert!(is_in_internal_call());
        }

        drop(outer);
        assert!(!is_in_internal_call());
    }

    #[test]
    fn flag_is_thread_local_not_global() {
        assert!(!is_in_internal_call());
        let outer = enter();
        assert!(is_in_internal_call());

        let other_thread_saw = std::thread::spawn(is_in_internal_call)
            .join()
            .expect("spawned thread must not panic");
        assert!(
            !other_thread_saw,
            "the guard must never be a global mutex — a concurrent \
             application-initiated open on another thread must still be \
             evaluated normally while this thread is inside its own \
             internal span"
        );

        drop(outer);
    }
}
