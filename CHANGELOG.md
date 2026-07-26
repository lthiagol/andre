# Changelog

All notable changes to andre are recorded here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

Work lands on the `wip` branch with an entry under **Unreleased**. Version
bumps and release dates are decided when `wip` promotes to `stable`.

## [Unreleased]

### Changed

- Package version bumped to `1.0.0-rc1` (git tag `v1.0.0-rc1`) for the GitHub release line.

### Added

- Makefile ergonomics: `fmt-check`, `ci`, `run`, `screenshots`, `doctor`, `deps`, `nextest`, `watch`, `release-assets`, plus a `demo/` fixture for local TUI runs.
- Testing infrastructure: `make test` and `make ci` prefer `cargo-nextest` (with a `cargo test` fallback for bare clones); `.config/nextest.toml` provides `default` and `ci` profiles (fail-fast on/off, longer test timeout in `ci`); `make test-unit` / `test-tui` / `test-e2e` split the suite by tier; `make coverage-check` is a hard gate (floors defined in `scripts/coverage-check.py`: andre-core ≥90%, andre ≥38% — M08 floor was 88%; M09 raised it) backed by `scripts/coverage-check.py` (M08).
- Woodpecker CI: `.woodpecker/base.yml` runs `make ci` (test step installs `cargo-nextest` only) + `make coverage-check` (coverage step installs `cargo-nextest`, `cargo-llvm-cov`, and `python3`) (M08). Each Woodpecker step runs in its own container with no shared filesystem, so tool installs live inside each consuming step — M08 F-02 external review fix; the YAML header documents the isolation rule so future maintainers don't recreate the original mistake.
- Testing quality (M09): gap tests for `StowEngine` (missing-binary surfaces a clear error; dry-run apply never touches the filesystem), process-level smoke tests in `andre/tests/smoke.rs` (`andre --help`, unknown-flag usage error, `check_dependencies.sh` exits 0 with stow absent on `PATH`), and AGENTS.md testing cheat sheet covering hermetic `App::with_home` + `PATH` mutation rules. Coverage floor for `andre-core` raised 88% → 90% (measured 91.78%); `andre` floor unchanged at 38% (measured 54.17%; TUI surface has limited marginal coverage growth without a UI harness).
- Test home injection: `App::with_home` / hermetic `make_test_app` so config-persistence tests do not depend on host `$HOME` contents (M01).
- Status bar breadcrumb from the component stack (`Main > Config`), with live toggles retained and render coverage for footer context hints (M02).
- App-level toast overlay (`Clear` + themed box); successful config save shows “Config saved” (failed save shows error toast); auto-dismiss on deadline (M04).
- Draft milestones M05 (TUI architecture / herdr seams), M06 (dependency hygiene), M07 (native link engine default, stow opt-in).
- Execute screen progress gauge (`completed/total`) above the existing spinner, per-package rows, and completion summary (M03).

### Changed

- TUI architecture (M05): `Component::prepare` owns viewport/scroll bookkeeping before paint; `App` owns `AsyncExecutor` (abort handle) for Execute's lifetime via `Transition::Replace` (Confirm no longer drops the only handle).
- Dependency cleanup (M06): dropped `dirs`, `thiserror`, `anyhow`, `clap`, and dead dev-deps (`assert_cmd`, `predicates`) in favor of local helpers / `andre_core::Result` / `std::env::args`; migrated archived `serde_yaml` to the maintained `serde_yaml_ng` fork. Direct-dep policy table added to `AGENTS.md`. `unicode-width` retained (documented).
- **Default link engine is now `native` (BREAKING).** `andre` no longer shells out to GNU Stow for the common stow/unstow/restow path (M07): a pure-Rust engine creates file-level symlinks directly. To keep the classic GNU Stow backend (folding, `--adopt`, `--dotfiles`, regex `--ignore`), set `global.engine: stow` in `andre.yml`. `doctor` now treats `stow` as an optional engine, not a hard runtime requirement. Config discovery stays XDG-first (`$XDG_CONFIG_HOME` / `~/.config`); on macOS also probes `~/Library/Application Support/andre` for back-compat with the former `dirs` path.
- `make lint` now depends on non-mutating `fmt-check` instead of `fmt`.
- Title-screen banner: boxed letters now spell **A.N.D.R.E.** (was the old "AWESOME STOW" art), and the tagline is the backronym "Another Neat Dotfile Repository Engine" (was "The ultimate dotfiles manager").

### Fixed

- E2E fixtures `08-defer-existing-target` and `09-override-existing-target` now ship a conflicting `target/.bashrc` so the conflict/FAILED assertions pass; re-enabled the three previously `#[ignore]`d e2e tests (08, 09, 14). Scenario 14 already resolved paths relative to the config file directory and needed no fixture change.
- `make coverage-check` awk script was syntactically broken (unbalanced quote, no crate-level totals in `llvm-cov --summary-only` output) and would never exit 0 even with thresholds met; rewritten as `scripts/coverage-check.py` consuming `cargo llvm-cov report --json` (M08).
- `test_add_group_then_save_persists` fails on minimal Linux CI images with empty `/root` (M01).
- Hermetic home injection extended to `make_empty_app` / `make_two_group_app`; unit tests cover `App::new` default home and `with_home` override (M01 external review).
- Render tests now sync `terminal_size` to the TestBackend; narrow-width coverage no longer false-greens against the default 80×24 layout (M02 external review).
- Execute Finished list scrolls again; completion summary separates skipped vs failed; remove dead Ctrl+C cancel hint (M03 external review).
- Toast error styling (`ToastKind`); config save no longer applies pending or pops Settings when the write fails (M04 external review).

## Prior releases

andre `0.1.0` and `0.2.0` predate this changelog. For their milestone-level
history, see [`master-plan-old/STATUS.md`](master-plan-old/STATUS.md)
(milestones 00–44).
