# Pollux FS Interpose PoC — Contributor & AI Guidelines

## What this repository is

`pollux-fs-interpose-poc` is a macOS-only proof of concept demonstrating process-level dynamic library interposition (`DYLD_INSERT_LIBRARIES`). It intercepts libc `open()` and `openat()` calls and gates them behind real Pollux Core `ALLOW`/`DENY` decisions via `pollux-polyglot-native-bridge`.

## Important Boundaries

- **Not a production sandbox:** It demonstrates feasibility of non-intrusive runtime gating. Production enforcement requires kernel-mediated mechanisms (such as macOS Endpoint Security Framework or Linux Landlock/seccomp).
- **Zero internal policy logic:** It translates intercepted paths into `filesystem.read` wire operations and relies completely on Pollux Core decisions.

## Development and Quality Gates

All development commands are governed by `Justfile`:

```bash
just install       # Fetch dependencies
just check         # Full gate: format + lint + check + test + cargo deny
just test          # Run process-level and unit test suite
```

- Code must pass `cargo fmt --check`.
- `clippy` is enforced with `-D warnings`.
- Dependencies and licenses must satisfy `cargo deny check`.
- Tests verify reentrancy safety and real child-process EACCES denial.
