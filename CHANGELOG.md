# Changelog

All notable changes to **pollux-fs-interpose-poc** are documented here.

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

---

## [0.1.0] — 2026-10-08

### Added

- Initial macOS dynamic library interposition proof-of-concept (`DYLD_INSERT_LIBRARIES`).
- Interception of libc `open` and `openat` functions gated by real Pollux Core decisions.
- Thread-local reentrancy guard preventing recursive interposition loops during manifest loading.
- End-to-end process-level test harness verifying `EACCES` prevention of denied filesystem operations.

---

[Unreleased]: https://github.com/sxnnyside-project/pollux-fs-interpose-poc/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sxnnyside-project/pollux-fs-interpose-poc/releases/tag/v0.1.0
