# Review cycle 2 — blocker-only pass over the cycle-1 fixes

Reviewer lane, herdr-orc pane `w1K:p1`. Baseline `6aeb1ca` → reviewed `6192261`.

**Verdict: ok — 0 HIGH/MEDIUM blockers.** All cycle-1 findings (F-01..F-07)
were addressed by the runner; nothing regressed.

## Fixes confirmed

| Finding | Fix | Verified how |
|---|---|---|
| F-01 | `AGENTS.md` + `scripts/coverage-check.py` now name `cargo llvm-cov --workspace --all-features --json` | `grep -n "llvm-cov" AGENTS.md scripts/coverage-check.py` — every hit is the collect+report form; no `report --json` left |
| F-02 | deferred, recorded as an open item in `todo.txt` | listed under `## Documentation` as a `- [ ]` item, explicitly attributed to cycle 1 |
| F-03 | `--promote` fetches `origin`, ff-catches local `stable` up to `origin/stable`, and returns to `wip` via an `EXIT` trap | functional test in a scratch clone (below) |
| F-04 | `usage()` no longer claims a default version; `ci_notes()` replaces the false "CI handles: binaries, GitHub Release" line | `grep -c "CI handles: binaries" scripts/release.sh` → 0; `ci_notes()` text matches the workflows as they stand |
| F-05 | the gate's preconditions (`stow` on `PATH`, whole suite green) documented in `AGENTS.md` | `AGENTS.md` L234-240 |
| F-06 | `wip-ci.yml` ends with a newline | diff shows `-…coverage-check\ No newline` → `+…coverage-check` |
| F-07 | `todo.txt` Documentation section is a list again | diff |
| extra | `a19e7fd` stages `Cargo.lock` with the version bump | see below |

## `a19e7fd` (staging `Cargo.lock`) — no problem

- The committed `Cargo.lock` is consistent with the manifests today:
  `andre/Cargo.toml` and `andre-core/Cargo.toml` are both `1.0.0-rc1`, and
  `Cargo.lock` lists `andre` / `andre-core` at `1.0.0-rc1` — so `cargo build
  --locked` / `make ci` (`nextest --locked`) are unaffected.
- In the scratch-clone release run, the bump regenerated the lock and the
  script committed it: afterwards `andre/Cargo.toml` = `1.0.0-rc9` and
  `Cargo.lock` = `1.0.0-rc9`, with `git status --porcelain` empty. Without
  this fix the tree stayed dirty and the next `--promote` was refused with
  "Working tree must be clean" — the fix is correct and necessary.

## `scripts/release.sh` — functional re-test (scratch clone, local bare origin)

| Scenario | Result |
|---|---|
| `release.sh 1.0.0-rc9` | bumped both manifests + lock, committed, tagged, pushed `wip` + tag; tree clean; still on `wip` |
| `release.sh --promote 1.0.0-rc9` | `6bbeed6..9898b29` **Fast-forward**, pushed `stable`; `stable` == tag commit; **no merge commits**; checkout back on `wip` |
| second `release.sh 1.0.0-rc10` after the promote | **not** refused with "Must be on wip branch" — the F-03 bug is gone |
| local `stable` lagging `origin/stable` | `git fetch origin` + `git merge --ff-only origin/stable` caught it up (`6192261..9898b29` FF), then `9898b29..db8ffb5` FF, pushed; still pure fast-forward |
| truly diverged `stable` (commit absent from the release line) | `fatal: Not possible to fast-forward, aborting.`, **exit 128**, `stable` unchanged locally and on origin, checkout restored to `wip`, worktree clean |

## Answers to the specific questions

1. **`Cargo.lock` / `--locked`** — consistent, no breakage (above).
2. **`EXIT` trap** — installed only inside the promote path, *before* the first
   `git checkout stable`, and it always restores `$RELEASE_BRANCH` (`wip`),
   which is the correct end state for that path. The release path never
   switches branches, so the trap cannot fire there. On success the script
   restores explicitly and then clears the trap (`trap - EXIT`), so there is no
   double checkout. Verified on the failure path (diverged `stable` → checkout
   still back on `wip`).
3. **Merge commits / force pushes** — none. The only mutations are
   `git merge --ff-only` (twice) and plain `git push origin stable`;
   `git log --merges stable` is empty after the promotions. A diverged
   `stable` aborts with exit 128 instead of forcing anything.
4. **`ci_notes()` accuracy** — accurate: the workflows' `ci` job runs
   `make ci` + `make coverage-check` (lint/test + coverage gates) and the
   `tap-update` job opens the Homebrew tap PR on a tag; no workflow builds
   release binaries or creates a GitHub Release, which is what the text now
   says.
5. **The 6 original ACs** — all still hold, re-run from a cleaned coverage
   state: AC-01 → `andre-core: 92.02%`, `andre: 54.40%`, "Coverage thresholds:
   OK"; AC-02 → both workflows matched, `llvm-tools-preview` present in both,
   `actionlint` exit 0; AC-03 → 6 `engine` hits in `REFERENCE.md` (template +
   flags table); AC-04 → the commented `engine:` key matches; AC-05 → the
   "Make sure GNU Stow is installed" count is 0 and `1.0.0-rc1` count is 3;
   AC-06 → `bash -n` clean and `"main"` count 0.
6. **Scope** — the cycle-2 diff touches `.github/workflows/wip-ci.yml` (EOF
   newline only), `AGENTS.md`, `CHANGELOG.md`, `scripts/coverage-check.py`,
   `scripts/release.sh`, `todo.txt`. No `andre/src/**`, `andre-core/src/**`,
   `andre/tests/**`, `andre-core/tests/**` or `tests/**` changes, and the
   `Makefile` coverage recipe is untouched.

## Regression gate

`make ci` → exit 0, **295 tests run / 295 passed / 0 skipped**.
`cargo fmt --all -- --check` → 0. `cargo clippy --all-targets --all-features
-- -D warnings` → 0. `git status --porcelain` → empty.

## Informational (not filed as a blocker)

`ci_notes()` points at `make release-assets` for the binaries, but that target
emits `dist/andre` and `dist/andre-macos-<arch>` and builds no Linux binary,
while `README.md`'s download URLs expect assets named `andre-aarch64-macos`
and `andre-x86_64-linux`. The naming mismatch predates this task and no
workflow or script in the repo produces those names; it is a release-packaging
question, not something these commits regressed.
