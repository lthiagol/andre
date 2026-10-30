# AGENTS.md - andre

**Project**: Terminal UI for dotfiles management (GNU Stow model) written in Rust using ratatui.
**Repository**: https://github.com/lthiagol/andre
**Branching**: `wip` (active development) → `stable` (default branch, release line). Work lands on `wip`; fast-forward `wip` into `stable` once CI is green. Don't commit directly to `stable`.
**Goal**: Robust, testable Rust application with comprehensive test coverage.

---

## Git Rules

- **Never commit or push without explicit order.** This applies to every single commit — prior consent from earlier in the session does NOT carry over. Show the message first, wait for confirmation.
- Push only when explicitly asked.
- Work goes on `wip`; each feature/change adds a `CHANGELOG.md` entry under `Unreleased`. Version bumps are decided when `wip` promotes to `stable`.

---

## Build Commands

Prefer `make` targets. Raw `cargo` is fine when you need a one-off flag.

`make test` and `make ci` **prefer `cargo-nextest`** when `cargo-nextest` is on
`PATH`; they fall back to `cargo test` for bare clones without nextest. CI
installs nextest explicitly via `.github/workflows/wip-ci.yml`. The nextest config lives
at `.config/nextest.toml` (nextest 0.9+ default lookup path).

```bash
make check            # cargo check --all --all-features
make fmt              # format (writes)
make fmt-check        # format check only (CI-safe, non-mutating)
make lint             # fmt-check + clippy -D warnings
make test             # lint + test-unit + test-tui + test-e2e (nextest-preferring)
make test-unit        # lib + bin unit tests only (no tui_e2e / e2e / integration)
make test-tui         # interaction, render, status_cache, config_* binaries
make test-e2e         # tui_e2e + andre-core e2e + integration (needs GNU stow)
make ci               # fmt-check + clippy + nextest --profile ci --locked
make run              # TUI against demo/ fixture
make screenshots      # same as run; capture into docs/screenshots/
make doctor           # build/runtime dependency check
make deps             # cargo fetch --locked (+ hints for optional tools)
make nextest          # cargo nextest run --all-features (requires cargo-nextest)
make watch            # cargo watch -x test (requires cargo-watch)
make build            # release binary → dist/andre
make coverage         # HTML coverage report → target/coverage/
make coverage-check   # gate: andre-core >=90%, andre >=38% (runs cargo llvm-cov --workspace --all-features --json)
```

One-off cargo (when make is the wrong shape):

```bash
cargo test -p andre --test tui_e2e
cargo test -p andre --test interaction
```

### Test tier cheat sheet

| Target | Runner | When to use |
|--------|--------|-------------|
| `make test-unit` | nextest (filter `kind(lib) | kind(bin)`) or `cargo test --lib --bins --all-features` | Fast feedback loop during development. Lib + bin unit tests only (131 tests). No filesystem or process side effects beyond temp dirs. |
| `make test-tui` | nextest (filter `binary(/^(interaction|render|status_cache|config_discovery|config_persistence|smoke)/)`) or `cargo test --test ...` | Component-level TUI behavior (keyboard dispatch, render snapshots, config persistence) plus process-level smoke tests (spawn the `andre` binary). No real stow required. |
| `make test-e2e` | nextest (filter `binary(/^(tui_e2e|e2e|integration|helpers)/)`) or `cargo test --test ...` | Real stow binary + real TUI runtime + andre-core test infrastructure (e.g. `helpers` test that needs stow on PATH). Requires GNU `stow` on `PATH`. |
| `make test` | All three above + lint | Pre-commit / pre-push gate. |
| `make ci` | Same as `make test` plus `--locked` and `--profile ci` | What CI runs. |

### Coverage

`make coverage-check` is the gate: it runs
`cargo llvm-cov --workspace --all-features --json` (collect **and** report in
one step), aggregates line counts per crate, and exits non-zero if either crate
drops below its floor (`andre-core` ≥90%, `andre` ≥38%). To re-baseline, edit
the `FLOORS` list in `scripts/coverage-check.py` and record the new % in the
`CHANGELOG.md` entry. HTML coverage (`make coverage`) is local-only.

---

## Architecture

```
andre/
├── andre-core/      # Business logic (config, stow, path, state, adopt)
├── andre/           # TUI binary (components, ui, app, execute)
├── todo.txt         # Open work items (temporary, hand-maintained)
├── AGENTS.md
└── REFERENCE.md        # Full documentation reference
```

**Two crates**: `andre-core` (no TUI deps) + `andre` (ratatui, crossterm, tokio).

### Architecture principles (target shape)

Borrow seams from mature TUIs (e.g. herdr / ratatui event-driven templates), not their product surface:

1. **State ≠ runtime ≠ render** — pure data and mutations stay testable without a real terminal; async/PTY/process handles live outside paint.
2. **Do not mutate during paint** — layout/viewport/scroll bookkeeping happens in update/prepare; `render` should draw from a snapshot (`&` state) where practical.
3. **App-level feedback** — toasts and similar chrome live on `AppContext` / `App`, not buried in one screen (see toast overlay + `tick_toast`).
4. **Keep custom domain widgets** — `FileBrowserWidget`, `GroupListWidget`, themed chrome stay ours; prefer core ratatui widgets (`Clear`, `Gauge`, `List`, …) for generic UI.

### Ratatui references

| Resource | Use for |
|----------|---------|
| https://ratatui.rs | Concepts: layout, widgets, rendering |
| https://ratatui.rs/tutorials/ | Event loop / app structure |
| https://github.com/ratatui/templates | `component`, `event-driven`, `event-driven-async` as architecture references (do not regenerate the app) |
| https://docs.rs/ratatui | API after version upgrades |
| https://github.com/ratatui/awesome-ratatui | Optional third-party widgets — only if std + our widgets are insufficient |

**Version:** workspace pins `ratatui` (see root `Cargo.toml`). Prefer a deliberate, planned bump over silent majors; watch `frame.size()` → `frame.area()` and layout API changes.

### Dependency policy

- Prefer `std` + `andre-core` before adding crates.
- Every direct dependency needs a one-line reason. Current direct graph (audited 2026-07):

| Crate | Consumer | One-line purpose |
|-------|----------|------------------|
| `serde` (+ `derive`) | andre-core | `Config` Serialize/Deserialize |
| `serde_yaml_ng` | andre-core | `andre.yml` parse/emit (maintained `serde_yaml` fork — see DIY notes) |
| `pathdiff` | andre-core | compute relative symlink paths (std lacks `Path::diff_paths`); native engine makes stow-compatible relative links |
| `andre-core` | andre | business logic (workspace path) |
| `ratatui` | andre | TUI framework (widgets, layout, render) |
| `crossterm` | andre | terminal raw mode + key/resize event input |
| `tokio` (`process`,`rt-multi-thread`,`time`,`sync`) | andre | async stow executor (`AsyncExecutor`) |
| `unicode-width` | andre | East-Asian/wide-char display width for elision + layout |
| `tempfile` *(dev)* | both | hermetic temp dirs in tests |

- Do not vendor ratatui/crossterm.
- External binaries (e.g. GNU `stow`) are runtime engines, not Cargo deps — see `global.engine` in `REFERENCE.md`.

**DIY notes** (thin wrappers owned in-tree rather than pulled as crates):
- `dirs` → `andre/src/paths.rs` — XDG-first `$HOME` / `$XDG_CONFIG_HOME`→`~/.config` (Unix) + `%APPDATA%` (Windows); macOS discovery also probes `~/Library/Application Support` (former `dirs` path).
- `thiserror` → hand-written `Display`/`Error`/`From` impls on `andre_core::Error` (`andre-core/src/error.rs`).
- `anyhow` → `andre_core::Result` in the binary and `App` constructors.
- `clap` → hand-rolled `std::env::args` parser in `andre/src/main.rs` (`--config/-c`, `--yolo`, `--onboard`, `--debug`, `--log-file`, `-h/--help`).
- Removed dead deps: `assert_cmd`, `predicates`, and `andre-core` dev-deps `crossterm` + `anyhow`.
- `serde_yaml` → `serde_yaml_ng`: `serde_yaml` is archived and `serde_yml` is deprecated (unmaintained shim); `serde_yaml_ng` is the maintained drop-in fork (verified: `deny_unknown_fields` round-trip + unknown-field wording unchanged).
- `unicode-width` retained (documented): a hand-rolled width helper would mis-measure CJK/emoji; correctness depends on the unicode tables the crate provides.

---

## Planning

Planning is tracked in a plain `todo.txt` at the repo root — a hand-maintained
list of open work items. There is no spec-driven plan tool in this project.

**Rules:**

1. **Add an item when work is identified.** One line, imperative, checkable.
2. **Remove an item when the work lands.** Keep it a live list, not a log.
3. **`CHANGELOG.md` is the historical record.** Every shipped change gets an
   entry under `Unreleased` — that is what replaces per-milestone evidence.
4. **Never complete on red tests.** Tests gate completion.
5. **Evidence is test output, not prose.** Record what ran + exit code, never
   *"test X verifies Y"*.

`ROADMAP.md` summarizes direction and version status. Keep it accurate by hand
when the version, branch state, or roadmap changes.

---

## Project Conventions

### Component Pattern
All screens implement `Component` trait (`components/mod.rs`). Navigation via `Transition` enum (`Push`, `Pop`, `Replace`, `Quit`, `Handled`). Each component owns its state — the `App` struct only holds the component stack and `AppContext`.

### Toast Pattern
App-level `AppContext::toast` + `show_toast` / `tick_toast`. Rendered with ratatui `Clear` + bordered `Paragraph` in `ui/toast.rs` after footer. Does **not** capture input. Newer toast replaces older. Wired from non-modal success paths (e.g. config save).

### Popup Pattern
Modal dialogs stored as `Option<SomeState>` fields on the component. Rendered as overlays in `render()`. `handle_key()` checks popup state first (returns early if active). Examples: `PickerState`, `GroupDialog`.

### Config Editing Pattern
`PendingSettings` snapshot created on first entry → mutations go to pending → `save_config()` applies to `ctx.core` + persists to disk. Global keys (`t`/`v`/`d`/`n`/`a`/`o`) intercepted via `Transition::Handled` to prevent bypassing the snapshot.

### InputMode Pattern
Components with text input or modal popups override `fn input_mode(&self) -> InputMode` (default: `Normal`). `App::component_dispatch` skips global shortcuts when mode is `Modal` or `TextInput`. Example: `AdoptNamePromptComponent` → `TextInput`, `SettingsComponent` → dynamic based on `picker`/`dialog` state.

### Path Resolution
Always via `resolve_path(path, config_dir, home_dir)` from `andre-core/src/path.rs`. Source/target existence checked by `get_group_source()` / `get_group_target()` in config.

### Theme Colors
`ThemeColors::from_theme(theme)` → 10 field color struct (`background`, `border`, `primary`, `success`, `warning`, `error`, `secondary`, `muted`, `highlight`, `indicator`). 8 themes cycled with `t`.

---

## Testing Tiers

| Tier | Location | What it tests |
|------|----------|---------------|
| Unit | In each module | Individual functions |
| Integration | `andre-core/tests/` | Core library with real stow + temp dirs |
| Interaction | `andre/tests/interaction.rs` | Keyboard dispatch through component stack (no terminal) |
| TUI E2E | `andre/tests/tui_e2e.rs` | Full flow with tokio runtime, real stow execution |
| E2E | `andre-core/tests/e2e.rs` | `andre --yolo` binary with fixture scenarios |
| Smoke | `andre/tests/smoke.rs` | Process-level binary checks (`andre --help`, `check_dependencies.sh`) |

Test pattern: `make_test_app()` or `App::new(config_path, false)` + `push_component()` + `component_dispatch(key)`. Inspect state via `as_any().downcast_ref()`. Smoke tests use `env!("CARGO_BIN_EXE_andre")` to spawn the built binary and assert stdout/exit.

### Hermetic test rules

Tests must not leak host environment into their assertions. Two rules cover
this:

1. **`HOME` injection via `App::with_home`.** Config-discovery and persistence
   tests must call `App::with_home` (or `make_test_app`) so the test's `$HOME`
   is the hermetic `TempDir`, never the developer's real home. The original fix
   (`test_add_group_then_save_persists` was failing on minimal Linux CI images
   with empty `/root`) was caused by this leak.
2. **No process-global `HOME`/`PATH` mutation under nextest parallelism.**
   nextest and `cargo test` run multiple test binaries in parallel, but tests
   within a single binary can also run in parallel threads. Tests that mutate
   `std::env::set_var("PATH", ...)` or `std::env::set_var("HOME", ...)` must
   wrap the mutation in `std::panic::catch_unwind` and restore the prior value
   in both the success and panic paths. The current callers of this pattern
   are `engine::tests::stow_engine_missing_binary_returns_clear_error` and
   `smoke::doctor_exits_zero_when_stow_missing`.

If a test needs to observe a `Command::new("stow")` missing-binary error, the
canonical pattern is:

```rust
let saved = std::env::var_os("PATH");
std::env::set_var("PATH", &empty_dir);
let result = std::panic::catch_unwind(|| {
    // ... assert the missing-binary error
});
match saved { Some(p) => std::env::set_var("PATH", p), None => std::env::remove_var("PATH") };
if let Err(e) = result { std::panic::resume_unwind(e); }
```

### Coverage gates

`make coverage-check` runs `scripts/coverage-check.py` against
`cargo llvm-cov --workspace --all-features --json` (collect and report in one
step). Floors: `andre-core ≥90%`, `andre ≥38%`.
Re-baseline by editing `FLOORS` in `scripts/coverage-check.py` and recording
the measured % + justification in the `CHANGELOG.md` entry. The CI gate fails
the build on regression, so any new untested code needs a test before it can
ship.

Because the gate collects coverage by running the whole test suite, it has two
preconditions: GNU `stow` must be on `PATH` (the `e2e` / `helpers` binaries
shell out to it), and the whole suite must pass.

---

## HTML Encoding Workaround

The `edit` tool HTML-encodes double quotes inside Rust string literals (`\"` → `&quot;`). For files containing Rust string literals with double quotes, use Python via bash:

```bash
python3 << 'PYEOF'
content = '''// Rust code with \"double quotes\"
'''
with open('/path/to/file.rs', 'w') as f:
    f.write(content)
PYEOF
```

---

## Quick Source Reference

| What | File |
|------|------|
| Component trait + Transition + InputMode | `andre/src/components/mod.rs` |
| App entry, dispatch, global keys | `andre/src/app.rs` |
| Settings component + group dialogs | `andre/src/components/settings/mod.rs` |
| Settings rendering + popups | `andre/src/ui/settings.rs` |
| Config loading + path resolution | `andre-core/src/config.rs` |
| Stow command building | `andre-core/src/stow.rs` |
| Async executor | `andre/src/execute.rs` |
| File browser widget | `andre/src/ui/widgets/file_browser.rs` |
| Test helpers (key events, app setup) | `andre/tests/helpers.rs` |
| Open work items | `todo.txt` |
| Historical record of shipped changes | `CHANGELOG.md` |
