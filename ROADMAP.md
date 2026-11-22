# Roadmap

Direction, version status, and release criteria for **andre**.
Open work items live in [`todo.txt`](todo.txt). Shipped changes are recorded
in [`CHANGELOG.md`](CHANGELOG.md).

## Current Status

| Field | Value |
|-------|--------|
| **Version** | `1.0.0-rc1` — published as a [pre-release](https://github.com/lthiagol/andre/releases/tag/v1.0.0-rc1); installs are Homebrew or from source (binary releases dropped 2026-11-02) |
| **Branch** | `wip` is ahead of `stable` with the unreleased work below; `stable` is fast-forwarded from `wip` once CI is green |
| **Open work** | see [`todo.txt`](todo.txt) |
| **Tests** | `make test` / `make ci` (coverage: andre-core ≥ 90%, TUI ≥ 38%) |

---

## Shipped (recent)

All planned work through the v1.0 release-candidate line is complete.

| Area | What landed |
|------|-------------|
| **Config persistence** | Hermetic test home (`App::with_home`) — no host `$HOME` dependency |
| **TUI chrome** | Persistent footer hints, top status bar, breadcrumb from the component stack |
| **Execute screen** | Progress gauge (`completed/total`), per-package rows, completion summary |
| **Feedback** | App-level toast overlay (auto-dismiss, non-blocking) wired to config save |
| **Architecture** | `Component::prepare` owns viewport/scroll bookkeeping; `App` owns the async executor |
| **Dependencies** | Dropped `dirs`/`thiserror`/`anyhow`/`clap`; `serde_yaml` → `serde_yaml_ng` |
| **Link engine** | `native` engine is now the default; GNU Stow remains opt-in via `global.engine: stow` (BREAKING) |
| **Testing** | `cargo-nextest` default, tiered `make test-unit`/`test-tui`/`test-e2e`, coverage floor gate |

### On `wip`, not yet released (breaking config changes)

| Area | What landed |
|------|-------------|
| **Config** | `~/.config/andre/` is the only config home; `--config-folder` for a fresh machine; legacy lookups removed |
| **Packages** | One packages folder (`packages:`), every subfolder a package, all linked into one target (`~`); `groups:` removed |
| **Profiles** | `profiles/<name>.yml` collections with includes; stow/unstow/status per profile; `--yolo --profile` |
| **New package** | Wizard: empty or from live files (move + link back), `.gitignore` / `.stow-local-ignore`, add to profiles |
| **TUI** | Every screen on one list + detail pattern, 8 refreshed themes, Nerd/Unicode icons, mouse support |

---

## Versioning

| Phase | Version | Criteria |
|-------|---------|----------|
| Current | v1.0.0-rc1 | Release-candidate line; tagged, notes-only release (no binary assets) — installs are Homebrew / from source |
| Next | v1.0.0 | Battle-tested macOS + Linux; frozen `andre.yml` |

Install: `brew install lthiagol/tap/andre`, or from source (`make install` / `cargo install`).

---

## Post-1.0 ideas (not yet planned)

Parked from the original project charter:

- More themes
- Richer config validation
- Whether to expand scope beyond Stow post-1.0 (undecided)

---

## Development principles

1. **No breaking config after v1.0** — `andre.yml` stays stable
2. **Tests before/with code** — prefer `cargo test` assertions over manual-only
3. **Small work items first** — `todo.txt` for polish, a written spec for features
4. **Changelog** — Unreleased on `wip`; version bump on `stable` promote

Keep this file accurate by hand when the version, branch state, or roadmap changes.
