# Changelog

All notable changes to andre are recorded here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

Work lands on the `wip` branch with an entry under **Unreleased**. Version
bumps and release dates are decided when `wip` promotes to `stable`.

## [Unreleased]

_(nothing yet)_

## [1.0.0-rc1] — 2026-10-30

The first published release on GitHub. Published as a pre-release with
binaries for macOS (Apple Silicon + Intel) and Linux (x86_64 + ARM64), and
bumped into the `lthiagol/tap/andre` Homebrew formula.

### Added

- Makefile ergonomics: `fmt-check`, `ci`, `run`, `screenshots`, `doctor`, `deps`, `nextest`, `watch`, `release-assets`, plus a `demo/` fixture for local TUI runs.
- Testing infrastructure: `make test` and `make ci` prefer `cargo-nextest` (with a `cargo test` fallback for bare clones); `.config/nextest.toml` provides `default` and `ci` profiles (fail-fast on/off, longer test timeout in `ci`); `make test-unit` / `test-tui` / `test-e2e` split the suite by tier; `make coverage-check` is a hard gate (floors defined in `scripts/coverage-check.py`: andre-core ≥90%, andre ≥38%) backed by `scripts/coverage-check.py`.
- Testing quality: gap tests for `StowEngine` (missing-binary surfaces a clear error; dry-run apply never touches the filesystem), process-level smoke tests in `andre/tests/smoke.rs` (`andre --help`, unknown-flag usage error, `check_dependencies.sh` exits 0 with stow absent on `PATH`), and AGENTS.md testing cheat sheet covering hermetic `App::with_home` + `PATH` mutation rules. Coverage floor for `andre-core` raised 88% → 90% (measured 91.78%); `andre` floor unchanged at 38% (measured 54.17%; TUI surface has limited marginal coverage growth without a UI harness).
- Test home injection: `App::with_home` / hermetic `make_test_app` so config-persistence tests do not depend on host `$HOME` contents.
- Status bar breadcrumb from the component stack (`Main > Config`), with live toggles retained and render coverage for footer context hints.
- App-level toast overlay (`Clear` + themed box); successful config save shows "Config saved" (failed save shows error toast); auto-dismiss on deadline.
- Execute screen progress gauge (`completed/total`) above the existing spinner, per-package rows, and completion summary.
- `todo.txt` — plain-text list of open work items, replacing the previous spec-driven planning tool.
- CI coverage gate: both `.github/workflows/wip-ci.yml` and `.github/workflows/stable-ci.yml` now install `cargo-llvm-cov` plus the `llvm-tools-preview` rustup component and run `make coverage-check`, enforcing the `andre-core` ≥90% / `andre` ≥38% floors on every run.
- `global.engine` documentation: the key is now shown in the `REFERENCE.md` "Complete Template" and the "Configuration Flags" table (both accepted values — `native` default and `stow`), and as a commented `engine:` key in `andre.yml.example`.

### Changed

- Package version bumped to `1.0.0-rc1` (git tag `v1.0.0-rc1`) for the GitHub release line.
- TUI architecture: `Component::prepare` owns viewport/scroll bookkeeping before paint; `App` owns `AsyncExecutor` (abort handle) for Execute's lifetime via `Transition::Replace` (Confirm no longer drops the only handle).
- Dependency cleanup: dropped `dirs`, `thiserror`, `anyhow`, `clap`, and dead dev-deps (`assert_cmd`, `predicates`) in favor of local helpers / `andre_core::Result` / `std::env::args`; migrated archived `serde_yaml` to the maintained `serde_yaml_ng` fork. Direct-dep policy table added to `AGENTS.md`. `unicode-width` retained (documented).
- **Default link engine is now `native` (BREAKING).** `andre` no longer shells out to GNU Stow for the common stow/unstow/restow path: a pure-Rust engine creates file-level symlinks directly. To keep the classic GNU Stow backend (folding, `--adopt`, `--dotfiles`, regex `--ignore`), set `global.engine: stow` in `andre.yml`. `doctor` now treats `stow` as an optional engine, not a hard runtime requirement. Config discovery stays XDG-first (`$XDG_CONFIG_HOME` / `~/.config`); on macOS also probes `~/Library/Application Support/andre` for back-compat with the former `dirs` path.
- `make lint` now depends on non-mutating `fmt-check` instead of `fmt`.
- Title-screen banner: boxed letters now spell **A.N.D.R.E.** (was the old "AWESOME STOW" art), and the tagline is the backronym "Another Neat Dotfile Repository Engine" (was "The ultimate dotfiles manager").
- Planning moved off the `mp` spec-driven toolkit: `master-plan/` and the legacy `master-plan-old/` directory are removed. `todo.txt` tracks open work, `CHANGELOG.md` remains the record of shipped changes, and `ROADMAP.md` is maintained by hand.
- `scripts/release.sh` now follows this repo's branch model: it releases from `wip` and promotes `wip` → `stable` as a separate, explicit `--promote` step, instead of requiring and pushing a nonexistent `main` branch.
- CI now builds the release binaries and publishes the GitHub Release. A new `release-assets` job in `stable-ci.yml` runs on a tag push and builds `andre-aarch64-macos`, `andre-x86_64-macos`, `andre-x86_64-linux` and `andre-aarch64-linux` — one per platform, natively, since cross-linking to Linux from macOS needs a Linux toolchain — then uploads them to the GitHub Release. Previously no workflow produced the assets the `README.md` download URLs point at.
- GNU Stow is documented as an optional runtime dependency: it is only required when `global.engine: stow` is set. `REFERENCE.md` and `README.md` no longer state that stow must be installed, and the `README.md` version badge and download URLs now match the package version `1.0.0-rc1`.
- Project framing now leads with the project's own identity instead of describing it by its opt-in backend: the `README.md` and `REFERENCE.md` taglines, the `README.md` "What is andre?" section, the `REFERENCE.md` "How to Use andre" section, the `AGENTS.md` project line, and the `andre --help` header all now use the **A.N.D.R.E.** backronym (*Another Neat Dotfile Repository Engine*) — matching the title-screen banner. andre is described as implementing the GNU Stow dotfiles model with a terminal UI on top, running on its built-in native engine by default.

### Fixed

- The `HOMEBREW_TAP_TOKEN` secret holds a classic personal access token that GitHub now rejects with `401 Bad credentials`, so every run of the Homebrew tap-update job failed at the tap checkout with "could not read Username for 'https://github.com'". The job now authenticates with an SSH deploy key (`ANDRE_TAP_SSH_KEY`) scoped to the tap and opens the PR with `ANDRE_TAP_PR_TOKEN`.
- The Homebrew tap-update job could never have succeeded. It checked out a `stable` branch in `lthiagol/homebrew-tap` (the tap's default branch is `main`, and it has no `stable` branch), edited a `Formula/andre-dev.rb` that does not exist in the tap, and opened its PR against `--base stable`. The tap's own CI also verifies that each formula's `sha256` matches its own `url`, and the job wrote the *tag* tarball's checksum into the dev formula whose `url` points at the *commit* tarball — a checksum mismatch that only fails at `brew install`, after the full download. The workflow now checks out `main`, targets `--base main`, computes a separate `dev_sha` for the commit archive, and preserves the formulas' existing indentation.
- Release-asset names did not match the download URLs advertised in `README.md`: the Makefile produced `dist/andre-macos-<arch>`, `dist/andre-linux-x86_64` and `dist/andre-linux-arm64` while the README linked `andre-aarch64-macos` and `andre-x86_64-linux`, so every published download URL would 404. The Makefile now emits `andre-aarch64-macos`, `andre-x86_64-macos`, `andre-x86_64-linux` and `andre-aarch64-linux`, and the README lists all four.
- E2E scenario 14 (`test_scenario_14_config_relative_paths`) was silently broken on fresh clones: the fixture file `tests/e2e/14-config-relative-paths/packages/test-pkg/.env` was excluded by `.gitignore` (`.env` defense-in-depth rule), so a fresh checkout had no `packages/` directory and the binary's `validate_sources` rejected every config with "Source directory not found". Renamed the fixture file to `env` (non-ignored) so it now tracks in git, and updated the test assertion accordingly. CI on `wip` was red on every push since `42cd9a6` un-ignored the test without first making the fixture trackable.
- E2E fixtures `08-defer-existing-target` and `09-override-existing-target` now ship a conflicting `target/.bashrc` so the conflict/FAILED assertions pass; re-enabled both `#[ignore]`d tests (08, 09).
- `make coverage-check` awk script was syntactically broken (unbalanced quote, no crate-level totals in `llvm-cov --summary-only` output) and would never exit 0 even with thresholds met; rewritten as `scripts/coverage-check.py` consuming `cargo llvm-cov report --json`.
- `test_add_group_then_save_persists` fails on minimal Linux CI images with empty `/root`.
- Hermetic home injection extended to `make_empty_app` / `make_two_group_app`; unit tests cover `App::new` default home and `with_home` override.
- Render tests now sync `terminal_size` to the TestBackend; narrow-width coverage no longer false-greens against the default 80×24 layout.
- Execute Finished list scrolls again; completion summary separates skipped vs failed; remove dead Ctrl+C cancel hint.
- Toast error styling (`ToastKind`); config save no longer applies pending or pops Settings when the write fails.
- `make coverage-check` now collects and reports in one step (`cargo llvm-cov --workspace --all-features --json`). It previously ran only `cargo llvm-cov report --json`, which reported previously-collected data and failed with `coverage-check: invalid JSON` on a clean tree.
- `scripts/release.sh --promote` no longer leaves the worktree checked out on `stable`: it fetches `origin` first (so a local `stable` lagging `origin/stable` no longer makes the push fail as non-fast-forward) and returns the checkout to `wip` on both the success and error paths. The script's summary of what CI does was corrected to match reality: CI now builds the release binaries, publishes the GitHub Release and opens the Homebrew tap PR, while `make release-assets` remains the way to build them locally.
- The `HOMEBREW_TAP_TOKEN` secret holds a classic personal access token that GitHub now rejects with `401 Bad credentials`, so every run of the Homebrew tap-update job failed at the tap checkout with "could not read Username for 'https://github.com'". The job now authenticates with an SSH deploy key (`ANDRE_TAP_SSH_KEY`) scoped to the tap and opens the PR with `ANDRE_TAP_PR_TOKEN`.
- The Homebrew tap-update job could never have succeeded. It checked out a `stable` branch in `lthiagol/homebrew-tap` (the tap's default branch is `main`, and it has no `stable` branch), edited a `Formula/andre-dev.rb` that did not exist in the tap, and opened its PR against `--base stable`. The tap's own CI also verifies that each formula's `sha256` matches its own `url`, and the job wrote the *tag* tarball's checksum into the dev formula whose `url` points at the *commit* tarball — a checksum mismatch that only fails at `brew install`, after the full download. The workflow now checks out `main`, targets `--base main`, computes a separate `dev_sha` for the commit archive, and preserves the formulas' existing indentation.
- Release-asset names did not match the download URLs advertised in `README.md`: the Makefile produced `dist/andre-macos-<arch>`, `dist/andre-linux-x86_64` and `dist/andre-linux-arm64` while the README linked `andre-aarch64-macos` and `andre-x86_64-linux`, so every published download URL would 404. The Makefile now emits `andre-aarch64-macos`, `andre-x86_64-macos`, `andre-x86_64-linux` and `andre-aarch64-linux`, and the README lists all four.
- The `aarch64-unknown-linux-gnu` release binary failed to link on an x86_64 runner: "rust-lld: error: --fix-cortex-a53-843419 is only supported on AArch64". It is now built on a native ARM runner (`ubuntu-24.04-arm`) instead of cross-linked.
- The release publish step only set the pre-release flag when creating a release, so a release made before that rule kept GitHub's "Latest" pointing at a release candidate. The flag is now reconciled on every publish.
- The `v1.0.0-rc1` tag pointed at a commit that predated the e2e scenario 14 fixture fix, so the tag's CI run failed and no release was ever published from it. The tag was moved to the fixed commit.

## Prior releases

andre `0.1.0` and `0.2.0` predate this changelog and were published on
Codeberg, so their changes are not itemized here and their release assets are
not on GitHub.
