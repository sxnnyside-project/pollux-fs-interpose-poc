//! Process-level proof: runs the real `poc-fixture` binary under
//! `DYLD_INSERT_LIBRARIES=<this crate's built dylib>`, so every
//! assertion here is about what a real OS process actually did — not
//! about calling this crate's Rust functions directly in-process.
//!
//! `POLLUX_POC_MANIFEST` points at a real Manifest file (read from disk
//! by the interposer itself, on every run, exactly as documented in
//! `src/engine.rs`); `POLLUX_POC_AUDIT_LOG` turns on the test-only audit
//! trail documented in `src/audit.rs` so these tests can observe
//! *whether the real `open()` ran*, not just the fixture's exit code.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Demo {
    root: PathBuf,
    manifest: PathBuf,
    audit_log: PathBuf,
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_nanos()
}

/// Builds a fresh temp directory with test fixtures:
///
/// ```text
/// assets/allowed.txt
/// secrets/denied.txt
/// ```
///
/// plus a real `pollux-protocol` Manifest granting `filesystem.read` on
/// `./assets` only — the same YAML shape
/// `pollux-polyglot-native-bridge`'s own tests already use.
fn setup_demo(label: &str) -> Demo {
    let root = std::env::temp_dir().join(format!(
        "pollux-fs-poc-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    std::fs::create_dir_all(root.join("assets")).expect("create assets/");
    std::fs::create_dir_all(root.join("secrets")).expect("create secrets/");
    std::fs::write(root.join("assets/allowed.txt"), b"allowed content\n")
        .expect("write assets/allowed.txt");
    std::fs::write(root.join("secrets/denied.txt"), b"top secret\n")
        .expect("write secrets/denied.txt");

    let manifest = root.join("manifest.yaml");
    std::fs::write(
        &manifest,
        "version: 1\nfilesystem:\n  read:\n    - ./assets\n",
    )
    .expect("write manifest.yaml");

    let audit_log = root.join("audit.log");

    Demo {
        root,
        manifest,
        audit_log,
    }
}

/// Locates the interposer dylib this same `cargo test` run just built.
/// Cargo places `cdylib` outputs directly in `target/<profile>/`
/// (unhashed) as well as a copy under `target/<profile>/deps/`; this
/// checks the direct location first and falls back to searching `deps/`
/// for robustness across Cargo versions.
fn interpose_dylib_path() -> PathBuf {
    let test_exe = std::env::current_exe().expect("current_exe");
    let deps_dir = test_exe.parent().expect("deps/ directory").to_path_buf();
    let profile_dir = deps_dir.parent().expect("profile directory").to_path_buf();

    let direct = profile_dir.join("libpollux_fs_interpose_poc.dylib");
    if direct.is_file() {
        return direct;
    }

    for entry in std::fs::read_dir(&deps_dir).expect("read deps/ directory") {
        let entry = entry.expect("directory entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("libpollux_fs_interpose_poc") && name.ends_with(".dylib") {
            return entry.path();
        }
    }

    panic!(
        "could not locate the built libpollux_fs_interpose_poc.dylib under {} or {}",
        profile_dir.display(),
        deps_dir.display()
    );
}

fn run_fixture(demo: &Demo, manifest: &Path, args: &[&str]) -> (i32, String) {
    let dylib = interpose_dylib_path();
    let output = Command::new(env!("CARGO_BIN_EXE_poc-fixture"))
        .args(args)
        .current_dir(&demo.root)
        .env("DYLD_INSERT_LIBRARIES", &dylib)
        .env("POLLUX_POC_MANIFEST", manifest)
        .env("POLLUX_POC_AUDIT_LOG", &demo.audit_log)
        .output()
        .expect("spawn poc-fixture");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    (output.status.code().unwrap_or(-1), stdout)
}

fn read_log(demo: &Demo) -> Vec<String> {
    std::fs::read_to_string(&demo.audit_log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn allowed_path_is_actually_opened_and_read() {
    let demo = setup_demo("allow");
    let (code, stdout) = run_fixture(&demo, &demo.manifest, &["./assets/allowed.txt"]);

    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("OPEN_OK"), "stdout: {stdout}");
    assert!(stdout.contains("allowed content"), "stdout: {stdout}");

    let log = read_log(&demo);
    assert!(
        log.iter().any(|line| line == "ALLOW ./assets/allowed.txt"),
        "expected an ALLOW decision in the audit log: {log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line == "REAL_OPEN ./assets/allowed.txt"),
        "expected the real open() to have actually run: {log:?}"
    );
}

/// The DENY test the task requires: it does not merely check that the
/// application "failed to read the file" — it asserts, from the audit
/// log, that the real `open()` for the denied path was never called at
/// all (no `REAL_OPEN` line exists for it), which is the actual claim
/// being proven.
#[test]
fn denied_path_returns_eacces_and_the_real_open_never_runs() {
    let demo = setup_demo("deny");
    let (code, stdout) = run_fixture(&demo, &demo.manifest, &["./secrets/denied.txt"]);

    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("OPEN_ERR"), "stdout: {stdout}");
    assert!(
        stdout.contains("PermissionDenied") || stdout.contains("Eacces") || stdout.contains("13"),
        "expected an EACCES-shaped error, got: {stdout}"
    );

    let log = read_log(&demo);
    assert!(
        log.iter().any(|line| line == "DENY ./secrets/denied.txt"),
        "expected a DENY decision in the audit log: {log:?}"
    );
    assert!(
        !log.iter()
            .any(|line| line == "REAL_OPEN ./secrets/denied.txt"),
        "the real open() was invoked for a DENIED path — enforcement did not \
         actually prevent the operation: {log:?}"
    );
}

/// Fail-closed proof: a Manifest Core itself rejects must deny an
/// otherwise-allowed path, not merely leave it unevaluated or fall
/// through to ALLOW.
#[test]
fn malformed_manifest_fails_closed_even_for_an_otherwise_allowed_path() {
    let demo = setup_demo("failclosed-manifest");
    let bad_manifest = demo.root.join("malformed.yaml");
    std::fs::write(&bad_manifest, "not: [valid, manifest").expect("write malformed manifest");

    let (code, stdout) = run_fixture(&demo, &bad_manifest, &["./assets/allowed.txt"]);

    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("OPEN_ERR"), "stdout: {stdout}");

    let log = read_log(&demo);
    assert!(
        log.iter().any(|line| line == "DENY ./assets/allowed.txt"),
        "an infrastructure failure must still deny the operation: {log:?}"
    );
    assert!(
        !log.iter()
            .any(|line| line == "REAL_OPEN ./assets/allowed.txt"),
        "infrastructure failure must never fall through to the real open(): {log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line.starts_with("INIT_FAILED PolluxEngine::load")),
        "expected engine init to record why it failed: {log:?}"
    );
}

/// Fail-closed proof, second cause: an engine that never got a Manifest
/// path at all must deny, not fall back to any default policy.
#[test]
fn missing_manifest_env_var_fails_closed() {
    let demo = setup_demo("failclosed-missing-manifest");
    // A manifest path that is never set: point POLLUX_POC_MANIFEST at a
    // file that does not exist, forcing the "reading manifest" failure
    // branch in src/engine.rs.
    let nonexistent = demo.root.join("does-not-exist.yaml");

    let (code, stdout) = run_fixture(&demo, &nonexistent, &["./assets/allowed.txt"]);

    assert_eq!(code, 1, "stdout: {stdout}");
    let log = read_log(&demo);
    assert!(
        log.iter().any(|line| line == "DENY ./assets/allowed.txt"),
        "log: {log:?}"
    );
    assert!(
        !log.iter()
            .any(|line| line == "REAL_OPEN ./assets/allowed.txt"),
        "log: {log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line.starts_with("INIT_FAILED reading manifest")),
        "log: {log:?}"
    );
}

#[test]
fn repeated_opens_in_one_process_reuse_the_same_engine() {
    let demo = setup_demo("reuse");
    let (code, stdout) = run_fixture(
        &demo,
        &demo.manifest,
        &[
            "./assets/allowed.txt",
            "./assets/allowed.txt",
            "./assets/allowed.txt",
        ],
    );
    assert_eq!(code, 0, "stdout: {stdout}");

    let log = read_log(&demo);
    let init_count = log.iter().filter(|line| line.as_str() == "INIT").count();
    assert_eq!(
        init_count, 1,
        "the engine must be created exactly once per process, not once per open(): {log:?}"
    );
    let allow_count = log
        .iter()
        .filter(|line| line.as_str() == "ALLOW ./assets/allowed.txt")
        .count();
    assert_eq!(
        allow_count, 3,
        "each of the 3 opens must still be evaluated: {log:?}"
    );
}

/// The manifest bootstrap read must never itself be evaluated as a
/// governed filesystem operation, one way or another.
///
/// Empirically, on this build, the read `src/engine.rs` performs
/// (`std::fs::read_to_string`) is compiled into this same dylib and
/// dyld's `__DATA,__interpose` mechanism does not redirect a symbol's
/// own defining image's calls to it — the same property this crate
/// relies on to call through to the real `open`/`openat` at all (see
/// `src/sys.rs`'s docs). So this read never reaches
/// `crate::decision::handle` in the first place, and the manifest path
/// never appears in the audit log at all — not `ALLOW`, not `DENY`, and
/// not `REENTRANT_PASS`. The reentrancy *guard* itself — needed for the
/// case where a call like this crosses an image boundary instead, e.g.
/// if Pollux Core's own dylib ever performs filesystem I/O internally —
/// is proven directly by `src/guard.rs`'s unit tests, run as part of
/// `cargo test` alongside this file.
#[test]
fn manifest_bootstrap_read_is_never_itself_treated_as_a_governed_operation() {
    let demo = setup_demo("reentrancy");
    let (code, stdout) = run_fixture(&demo, &demo.manifest, &["./assets/allowed.txt"]);
    assert_eq!(code, 0, "stdout: {stdout}");

    let log = read_log(&demo);
    let manifest_path = demo.manifest.to_string_lossy().into_owned();

    assert!(
        !log.iter().any(|line| {
            line == &format!("ALLOW {manifest_path}")
                || line == &format!("DENY {manifest_path}")
                || line == &format!("REENTRANT_PASS {manifest_path}")
        }),
        "the manifest bootstrap read must never itself be evaluated as a \
         governed filesystem operation: {log:?}"
    );
}
