# Pollux FS Interpose PoC

![Banner](pollux-fs-interpose-poc-banner.png)

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/License-MIT-green)
[![CI](https://github.com/sxnnyside-project/pollux-fs-interpose-poc/workflows/CI/badge.svg)](https://github.com/sxnnyside-project/pollux-fs-interpose-poc/actions)

<p align="center">
  <strong>macOS Interposition ✦ Zero Modification ✦ Runtime Gating</strong><br>
  <em>macOS dynamic library interposition proving real libc open()/openat() calls gated by Pollux Core decisions.</em>
</p>

<p align="center">
  <a href="#about">About</a> ✦
  <a href="#features">Features</a> ✦
  <a href="#installation">Installation</a> ✦
  <a href="#usage">Usage</a> ✦
  <a href="#architecture">Architecture</a> ✦
  <a href="#contributing">Contributing</a>
</p>

---

## About

**Pollux FS Interpose PoC** is a macOS-only proof of concept: a real `open()`/`openat()` call intercepted via `DYLD_INSERT_LIBRARIES` interposition, gated by a real Pollux Core ALLOW/DENY decision reached through the unmodified [`pollux-polyglot-native-bridge`](../pollux-polyglot-native-bridge):

```text
interposer -> pollux-polyglot-native-bridge -> remote pollux-core v0.1.0
```

### What this proves

```text
read("./assets/allowed.txt")  -> Core -> ALLOW -> real open() runs, file is read
read("./secrets/denied.txt")  -> Core -> DENY  -> real open() never runs, EACCES
```

### Philosophy

> *"Enforcement must prove that unauthorized operations never touch the kernel, while leaving the application binary completely unmodified."*

Pollux FS Interpose PoC is part of the Sxnnyside Project's Pollux Ecosystem.

---

## Features

- **Dynamic Libc Interposition**: Intercepts `open()` and `openat()` using Mach-O `__DATA,__interpose` mechanics without relinking target binaries.
- **Pure Core Decisions**: Relies on unmodified `PolluxEngine` evaluation; defines no independent capability or authorization logic.
- **Thread-Local Reentrancy Guard**: Prevents recursive interception during internal manifest resolution and engine initialization.
- **Fail-Closed Execution**: Missing, malformed, or unreadable manifests immediately deny operations and abort syscall delegation.
- **Verifiable Audit Logging**: Test-only execution tracing to verify whether physical syscalls were dispatched or aborted.

---

## Important Notice: Security Boundary

`DYLD_INSERT_LIBRARIES` interposition can be bypassed by the governed process itself: unsetting the environment variable, executing raw syscalls directly, or running under Hardened Runtime.

This crate exists solely to prove that filesystem operations *can* be placed behind a real Pollux Core Decision, and that a `DENY` decision stops the real syscall from occurring. Production enforcement requires kernel mediation (Endpoint Security Framework on macOS, Landlock/seccomp on Linux) which cannot be bypassed in userland.

---

## Installation

### Prerequisites

- macOS (`aarch64-apple-darwin` or `x86_64-apple-darwin`)
- Rust (`1.93.0` or stable)
- Just command runner

### From Source

```bash
git clone https://github.com/sxnnyside-project/pollux-fs-interpose-poc.git
cd pollux-fs-interpose-poc

just install
just build
```

---

## Usage

### Running the Demonstration

Build the dynamic interposition library and fixture:

```bash
cargo build
```

Execute a binary with dynamic interposition:

```bash
DYLD_INSERT_LIBRARIES=target/debug/libpollux_fs_interpose_poc.dylib \
POLLUX_POC_MANIFEST=./manifest.yaml \
target/debug/poc-fixture ./assets/allowed.txt ./secrets/denied.txt
```

Set `POLLUX_POC_AUDIT_LOG=/path/to/audit.log` to record each evaluated decision and whether the real `open()` call was dispatched.

---

## Architecture

```text
pollux-fs-interpose-poc/
├── src/
│   ├── lib.rs              # C-dylib exports and rlib entrypoints
│   ├── interpose.rs        # Mach-O __DATA,__interpose symbol replacement table
│   ├── engine.rs           # Manifest loading and bridge evaluation orchestration
│   ├── guard.rs            # Thread-local reentrancy guard
│   ├── operation.rs        # Translates libc path arguments to wire Operation
│   ├── sys.rs              # Raw libc and dlsym forwarding helpers
│   └── bin/
│       └── poc_fixture.rs  # Target test fixture binary
└── tests/
    └── enforcement.rs      # Process-level integration tests under DYLD_INSERT_LIBRARIES
```

---

## Scope & Limitations

- Only `open` and `openat` are interposed.
- Intercepted operations are modeled as `filesystem.read` requests.
- Does not enforce boundaries across child processes beyond standard environment inheritance.
- No anti-tampering or kernel-level protections are implemented.

---

## Contributing

Contributions are accepted. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Before contributing, read the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  <strong>Pollux FS Interpose PoC</strong> — A Sxnnyside Project<br>
  <em>&copy; 2026 Sxnnyside Project</em>
</p>
