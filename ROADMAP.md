# Roadmap

Direction, version status, and release criteria for **andre**.
Open work items live in [`todo.txt`](todo.txt). Shipped changes are recorded
in [`CHANGELOG.md`](CHANGELOG.md).

## Current Status

| Field | Value |
|-------|--------|
| **Version** | `1.0.0-rc1` (see [CHANGELOG.md](CHANGELOG.md) for Unreleased) |
| **Branch** | `wip` → promote to `stable` when CI is green |
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

---

## Versioning

| Phase | Version | Criteria |
|-------|---------|----------|
| Current | v1.0.0-rc1 | Release-candidate line; tagged, release assets published |
| Next | v1.0.0 | Battle-tested macOS + Linux; frozen `andre.yml` |

Install: `brew install lthiagol/tap/andre` or [releases](https://github.com/lthiagol/andre/releases).

---

## Post-1.0 ideas (not yet planned)

Parked from the original project charter:

- More themes
- Richer config validation
- Adoption-UX expansions
- Whether to expand scope beyond Stow post-1.0 (undecided)

---

## Development principles

1. **No breaking config after v1.0** — `andre.yml` stays stable
2. **Tests before/with code** — prefer `cargo test` assertions over manual-only
3. **Small work items first** — `todo.txt` for polish, a written spec for features
4. **Changelog** — Unreleased on `wip`; version bump on `stable` promote

Keep this file accurate by hand when the version, branch state, or roadmap changes.
