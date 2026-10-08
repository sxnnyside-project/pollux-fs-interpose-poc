# Pollux FS Interpose PoC task runner.
# Every recipe wraps a real cargo / cargo-deny command.

# Bootstrap dependencies.
install:
    cargo fetch --locked

# Fast type/borrow-check without codegen.
dev:
    cargo check

# Produce build artifacts.
build:
    cargo build --locked

# Run test suite.
test:
    cargo test --locked

# Correctness / type checking across all targets.
typecheck:
    cargo check --all-targets --locked

# Static analysis via clippy with all warnings treated as errors.
lint:
    cargo clippy --all-targets -- -D warnings

# Verify dependency advisories, licenses, bans and sources.
deny:
    cargo deny check

# Apply formatting in place.
format:
    cargo fmt --all

# Verify formatting without modifying files.
format-check:
    cargo fmt --all -- --check

# Full quality gate, non-mutating; invoked by CI.
check: format-check lint typecheck test deny

# Clean build artifacts and caches.
clean:
    cargo clean
