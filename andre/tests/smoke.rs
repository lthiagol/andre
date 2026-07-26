//! Process-level smoke tests (M09 S2).
//!
//! These tests spawn the built `andre` binary as a subprocess and assert
//! observable behavior of the CLI. Unlike `interaction.rs` and `render.rs`
//! which test the in-process `App` component stack, these tests cover the
//! actual binary entry point — the surface that shell users, scripts, and
//! release artifacts see.
//!
//! The binary path is resolved via `env!("CARGO_BIN_EXE_andre")`. Both
//! `cargo test` and `cargo nextest run` set this env var per test process
//! (nextest has supported this since 0.9.x), so the smoke tests work under
//! both runners without per-test path guessing.

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Path to the `andre` binary built for the current test process.
fn andre_bin() -> &'static str {
    env!("CARGO_BIN_EXE_andre")
}

/// Path to the `check_dependencies.sh` script at the repo root.
fn doctor_script() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().join("check_dependencies.sh")
}

#[test]
fn andre_help_exits_zero_and_mentions_usage() {
    let output = Command::new(andre_bin())
        .arg("--help")
        .output()
        .expect("failed to spawn andre binary");

    assert!(
        output.status.success(),
        "andre --help should exit 0, got {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let upper = stdout.to_uppercase();
    assert!(
        upper.contains("USAGE") || upper.contains("OPTIONS"),
        "andre --help output should mention USAGE or OPTIONS; got:\n{}",
        stdout
    );
}

#[test]
fn andre_help_lists_known_flags() {
    // The help text enumerates every flag the CLI accepts. A regression that
    // renames or drops a flag should break this test before users notice.
    let output = Command::new(andre_bin())
        .arg("--help")
        .output()
        .expect("failed to spawn andre binary");
    assert!(output.status.success(), "andre --help must exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    for needle in ["--config", "--yolo", "--onboard", "--debug", "--help"] {
        assert!(
            stdout.contains(needle),
            "andre --help should mention {needle}; got:\n{stdout}"
        );
    }
}

#[test]
fn andre_unknown_flag_exits_nonzero() {
    // Usage errors must NOT exit 0 (a zero exit would let CI scripts that
    // pipe `--help` to a pager accidentally treat typos as success).
    let output = Command::new(andre_bin())
        .arg("--definitely-not-a-real-flag")
        .output()
        .expect("failed to spawn andre binary");

    assert!(
        !output.status.success(),
        "andre --definitely-not-a-real-flag should exit non-zero, got {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Usage") || stderr.contains("usage"),
        "andre should print a usage error for unknown flags; stderr:\n{}",
        stderr
    );
}

// M09 S3: the doctor script must exit 0 when GNU stow is unresolvable on
// PATH. Stow is an optional engine (M07), so a missing binary is a warning,
// not a failure. We reproduce that condition by isolating PATH to a tempdir
// that has no `stow` binary, then invoking check_dependencies.sh.
//
// Race-safety: this test mutates PATH in the test binary's process. The
// `andre::smoke` binary currently has no other test that reads PATH, so the
// race does not manifest in practice. cargo test DOES run tests within a
// binary in parallel (default test-threads = num-cpus); nextest 0.9+ runs
// each test in its own process. Any future smoke test that reads PATH (e.g.,
// a `which` check) MUST wrap its `env::var` in the catch_unwind+set_var
// dance below, or risk observing the stripped PATH set by this test.
// `catch_unwind` here covers the panic case (assertion failure inside the
// override) — it does NOT serialize against other parallel tests.

#[test]
fn doctor_exits_zero_when_stow_missing() {
    // Find the directory containing the host's `stow` binary (if any), then
    // build a PATH that excludes ONLY that directory. Everything else on
    // PATH — including rustc/cargo/cc — stays intact, so the script's build
    // dependency checks still pass. This reproduces the "developer machine has
    // stow, but we want to verify the script handles its absence gracefully"
    // scenario without requiring a stripped CI environment.

    let tmp = TempDir::new().unwrap();
    let no_stow = tmp.path().join("no_stow_dir");
    std::fs::create_dir_all(&no_stow).unwrap();

    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let original_path_str = original_path.to_string_lossy();

    // Locate the directory holding `stow` via `which`. If `stow` is already
    // missing on PATH, skip the override and run the script directly.
    let stow_dir = Command::new("/bin/bash")
        .arg("-c")
        .arg("command -v stow | xargs -I{} dirname {} 2>/dev/null || true")
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        });

    let mut path_without_stow: Vec<&str> = original_path_str
        .split(':')
        .filter(|p| !p.is_empty())
        .filter(|p| match &stow_dir {
            Some(dir) => *p != dir.as_str(),
            None => true,
        })
        .collect();
    // Prepend a directory that is guaranteed empty so the new PATH is
    // observably different from the original. (Cosmetic only.)
    path_without_stow.insert(0, no_stow.to_str().unwrap());
    let stripped = path_without_stow.join(":");

    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", &stripped);

    let result = std::panic::catch_unwind(|| {
        // Use absolute bash path so the modified PATH doesn't break bash lookup.
        let bash = Command::new("/bin/bash")
            .arg(doctor_script())
            .output()
            .expect("failed to spawn check_dependencies.sh");

        let stdout = String::from_utf8_lossy(&bash.stdout);
        let stderr = String::from_utf8_lossy(&bash.stderr);
        assert!(
            bash.status.success(),
            "check_dependencies.sh must exit 0 when stow is missing; got {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            bash.status,
            stdout,
            stderr
        );
        // Optional-engine messaging should be visible so users know stow is
        // not required.
        let combined = format!("{stdout}{stderr}").to_lowercase();
        assert!(
            combined.contains("stow"),
            "doctor output should mention stow even when missing; got:\n{stdout}{stderr}"
        );
    });

    match saved {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
