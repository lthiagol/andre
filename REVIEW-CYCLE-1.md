# Review cycle 1 — coverage-gate plumbing + native-engine docs

Reviewer lane, herdr-orc pane `w1K:p1`. Baseline `d86ac06` → reviewed `4a41b40`
(branch `wip`, repo `/Users/thiago/Github/andre`).

**Verdict: ok** (7 findings, all LOW severity).

---

## 1. Acceptance-criteria verification (independently reproduced)

| AC | Command actually run | Exit | Result |
|----|----------------------|------|--------|
| AC-01 | `cargo clean` → `cargo llvm-cov clean --workspace && make coverage-check` | 0 | `andre-core: 92.02% (>= 90%, 2225/2418 lines)`, `andre: 54.40% (>= 38%, 2883/5300 lines)`, `Coverage thresholds: OK` |
| AC-02 | `grep -l "coverage-check" .github/workflows/wip-ci.yml .github/workflows/stable-ci.yml` | 0 | both files listed |
| AC-02 | `actionlint .github/workflows/wip-ci.yml .github/workflows/stable-ci.yml` | 0 | clean (SC2129 fix verified) |
| AC-03 | `grep -c "engine" REFERENCE.md` | 0 | 6 — template `global:` block (L243-245) + flags table row (L276) |
| AC-04 | `grep -A5 "^global:" andre.yml.example \| grep "engine"` | 0 | matches (real key `# engine: native` at L9) |
| AC-05 | `grep -c "Make sure \[GNU Stow\] is installed" README.md` | 1 | 0 matches (claim removed) |
| AC-05 | `grep -c "1.0.0-rc1" README.md` | 0 | 3 — badge (L7), curl macOS (L67), curl Linux (L72) |
| AC-06 | `bash -n scripts/release.sh` | 0 | syntax OK |
| AC-06 | `grep -c '"main"' scripts/release.sh` | 1 | 0 matches |

Regression gate: `make ci` → exit 0, **295 tests run / 295 passed / 0 skipped**.
`cargo fmt --all -- --check` → 0. `cargo clippy --all-targets --all-features -- -D warnings` → 0.
`git status --porcelain` → empty.

Scope: `git diff --name-only d86ac06..HEAD` touches only
`.github/workflows/*`, `CHANGELOG.md`, `Makefile`, `README.md`, `REFERENCE.md`,
`andre.yml.example`, `scripts/release.sh`, `todo.txt`.
**No** `andre/src/**`, `andre-core/src/**`, `andre/tests/**`, `andre-core/tests/**`,
or `tests/**` changes.

## 2. Deeper checks beyond the AC greps

- **AC-01 is a real collect, not a stale report.** `cargo llvm-cov` with no
  subcommand runs the `test` subcommand: the transcript above shows the full
  suite executing (20+13+10+13+56+36+4+3+6+98+18+4+14 = 295 tests) before the
  JSON report is emitted. This is what makes it work on a clean tree.
- **Error propagation is loud.** The old recipe piped stderr to `/dev/null`;
  the new one does not. If `cargo llvm-cov` fails (e.g. missing
  `llvm-tools-preview`), stdout is empty → `coverage-check.py` prints
  `coverage-check: invalid JSON` and exits **2**, so `make` fails. Verified:
  `printf '' | python3 scripts/coverage-check.py` → exit 2. The pipeline's exit
  code is the parser's (last stage), not cargo's — acceptable because the
  parser's exit 2 is non-zero and cargo's own error text is now visible.
- **Engine semantics match the code, not just the prose.**
  `andre-core/src/config.rs:21` declares `global.engine: Option<String>`;
  `effective_engine()` (L245) defaults to `native`;
  `engine_for()` (`andre-core/src/engine.rs:54`) selects `stow` only on an
  explicit `"stow"`. `NativeEngine` rejects `adopt` and `dotfiles` with an
  error recommending `global.engine: stow` (`engine.rs:158-165`, asserted by
  `native_rejects_adopt_and_dotfiles`), and regex `--ignore` patterns are not
  honored by native (`engine.rs:111-113`). The new docs say exactly this, and
  the "native is the default / stow is opt-in" direction is correct in
  REFERENCE.md, andre.yml.example, and README.md. No doc states the opposite.
- **CI toolchain wiring is correct.** `dtolnay/rust-toolchain@stable` accepts
  `components: llvm-tools-preview`; the component and `cargo-llvm-cov` come
  from the same toolchain, so the `llvm-cov` binary version matches the
  toolchain's llvm-tools. `cargo install cargo-llvm-cov --locked` installs the
  current 0.8.x, which works with the stable toolchain. `stow` is installed
  (apt step) **before** the new `make coverage-check` step, and the new step
  runs **after** `make ci`, so the e2e tests that shell out to `stow` will
  find it.
- **No CI step or job was dropped.** `make ci` (with `NEXTTEST: 1`) is intact
  in both workflows; `stable-ci.yml`'s `tap-update` job is intact, and the
  SC2129 fix emits the identical four `$GITHUB_OUTPUT` variables with the
  identical values (`version`, `commit`, `sha`, `date`) — the `{ ... } >> file`
  grouping is purely a shell-style change.
- **AC-06 verified functionally, not just by grep.** Both paths were exercised
  end-to-end in a scratch clone (`/tmp`, since removed) with a local bare
  `origin`:
  - `scripts/release.sh 1.0.0-rc2` → bumped both `Cargo.toml`s, committed,
    tagged `v1.0.0-rc2`, pushed `wip` and the tag. No `main` involved.
  - `scripts/release.sh --promote 9.9.9-test` → fast-forwarded `stable` onto
    the tagged commit (`Updating 6bbeed6..4a41b40 / Fast-forward`) and pushed
    `stable`. `git merge --ff-only` is the right operation for this repo's
    model, and it fails loudly (non-zero) if `stable` has diverged.

## 3. Findings

All LOW. None block; no HIGH or MEDIUM issues found.

- **F-01 (low, docs)** — `AGENTS.md` L67 and L232-233 still say the gate
  "parses `cargo llvm-cov report --json`", and `scripts/coverage-check.py`'s
  module docstring (L4) says the same. The Makefile now runs
  `cargo llvm-cov --workspace --all-features --json` (collect + report). The
  gate's behavior is correct, but `AGENTS.md` is this repo's authoritative
  build-command reference and now misdescribes the command the gate runs.
  Suggest: update both to `cargo llvm-cov --workspace --all-features --json`.

- **F-02 (low, docs)** — The "andre is a Stow wrapper" framing survives in
  the docs even though the native engine is the default: `README.md` L12
  ("A TUI wrapper for GNU Stow"), `REFERENCE.md` L7 (same tagline) and L57
  ("andre wraps Stow in an interactive TUI"). These do not claim stow is
  *required*, so this is not a contradiction of the native default — but the
  taglines now describe an opt-in engine as the product's core. Also
  `REFERENCE.md` L114 (Homebrew section) still reads "Installs Rust and GNU
  Stow." unconditionally, while the "Build from source" table (L121) was
  correctly changed to mark GNU Stow optional — the two sections are now
  inconsistent.

- **F-03 (low, release-script)** — `scripts/release.sh --promote` leaves the
  worktree checked out on `stable` and never returns to `wip` (confirmed in
  the scratch-clone run: `git branch` showed `* stable` afterwards). The
  release path then refuses to run ("Must be on wip branch"). It also does not
  `git fetch` before the ff-only merge and the `git push origin stable`, so a
  local `stable` that lags `origin/stable` makes the promotion push fail
  non-fast-forward. Suggest: `git fetch origin` before the merge and
  `git checkout wip` at the end of the promote path.

- **F-04 (low, release-script)** — `usage()` L20 reads
  `Release from 'wip' (default: 1.0.0-rc1 style, e.g. 1.0.0-rc1)`. There is no
  default version and no "style" — the parenthetical reads like leftover
  text and is confusing. Separately, the script's success output still claims
  "CI handles: binaries, GitHub Release, Homebrew tap update", but
  `.github/workflows/` contains only `wip-ci.yml` and `stable-ci.yml`, whose
  jobs are `ci` (lint+test+coverage) and `tap-update` — **no** workflow
  builds release binaries or creates a GitHub Release. The `README.md` curl
  URLs (`andre-aarch64-macos`, `andre-x86_64-linux` under the release tag)
  therefore have no producer inside this repo. This wording is pre-existing
  and out of AC-05/AC-06 scope, but the release script was rewritten and
  re-emits the claim, so it is worth correcting (or documenting where the
  binaries come from).

- **F-05 (low, docs)** — `make coverage-check` now implies a full test-suite
  run (`cargo llvm-cov`'s default subcommand is `test`). That is exactly what
  AC-01 asked for, but it means the gate has two new implicit preconditions
  that are recorded nowhere: GNU `stow` must be installed (the `e2e` and
  `helpers` test binaries shell out to it), and the whole suite must pass.
  Neither `AGENTS.md` (L44-45, L232) nor the Makefile comment block
  mentions this. Suggest one line in `AGENTS.md` noting the gate runs the
  suite and needs `stow` on `PATH`.

- **F-06 (low, nit)** — `.github/workflows/wip-ci.yml` still ends without a
  trailing newline; the new final line (`run: make coverage-check`) inherits
  the missing newline that the previous last line (`run: make ci`) had. YAML
  is valid either way, but the file now ends mid-line-terminator.

- **F-07 (low, nit)** — `todo.txt` "Documentation" section was replaced with
  a prose placeholder — "(nothing open — `global.engine`, the native-engine
  default, and the GNU Stow optionality are documented in ...)" — instead of
  the list format the file uses elsewhere (and that `AGENTS.md`'s Planning
  rules describe: "One line, imperative, checkable"). A short
  "- (nothing open)" line would match the file's existing shape.

## 4. Notes for the orchestrator

- The runner's claimed evidence reproduced exactly on every AC, including on
  a genuinely clean tree (`cargo clean` + `cargo llvm-cov clean --workspace`
  before `make coverage-check`) — the specific false-green the task warns
  about does not occur.
- `make ci` is 295/295 after the change; no source or test files were
  touched, so the regression risk is nil.
- No push was performed and no implementation file was edited during this
  review; the only worktree addition is this findings file.
