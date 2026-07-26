# Roadmap

Summary of the active Master Plan (`master-plan/`, managed by `mp`).  
Source of truth: `mp status` / `mp list milestones`. Do not edit plan state here.

## Current Status

| Field | Value |
|-------|--------|
| **Version** | 0.2.0 (see [CHANGELOG.md](CHANGELOG.md) for Unreleased) |
| **Branch** | `wip` → promote to `stable` when CI is green |
| **Plan mode** | autonomous (`mp execution`) |
| **Next** | M02/S1 — footer audit / breadcrumb work |
| **Validate** | `mp validate` ok |
| **Tests** | `make test` / `make ci` (coverage: andre-core ≥ 88%, TUI ≥ 38%) |

Historical milestones 00–44: complete — see `master-plan-old/STATUS.md`.

---

## Active milestones (`master-plan/`)

| ID | Title | Lifecycle | Focus |
|----|-------|-----------|--------|
| **M01** | Make config-persistence test hermetic for Linux CI | **complete** | Hermetic `home_dir`; `make ci` green |
| **M02** | Persistent TUI chrome: footer hints + top status bar | approved | **Breadcrumb** on status bar + render tests (footer already context-sensitive) |
| **M03** | Execute screen overhaul: progress gauge + per-package status | approved | **ratatui Gauge**; keep existing spinner/rows/summary |
| **M04** | Action feedback toasts + number-key menu shortcuts | approved | **Toasts only** (digits split to track) |

### Suggested path

1. ~~M01 complete~~  
2. M02/S1–S3 (breadcrumb + render coverage)  
3. M03/S1–S3 (execute Gauge)  
4. M04/S1–S3 (toast subsystem)  
5. Track TW-02 (menu keys 1–7) anytime  

### Tracks

| ID | Kind | Title | Status |
|----|------|-------|--------|
| **TW-02** | tweak | Main Menu number keys 1–7 | pending |

*(Other pending tweaks may exist; `mp list tracks` for full list.)*

---

## Milestone intent (one-liners)

- **M01** — Config-persistence test passes with a temp home on empty Linux CI images; production home resolution unchanged.  
- **M02** — Status bar breadcrumb from `component_stack`; footer/toggles preserved and render-tested.  
- **M03** — Execute screen Gauge for completed/total; existing rows/spinner/summary kept.  
- **M04** — Non-modal toast overlay (auto-dismiss, non-blocking); config save wired.

---

## Versioning

| Phase | Version | Criteria |
|-------|---------|----------|
| Current | v0.2.0 | Homebrew + releases live |
| Future | v1.0.0 | Battle-tested macOS + Linux; frozen `andre.yml` |

Install: `brew install lthiagol/tap/andre` or [releases](https://github.com/lthiagol/andre/releases).

---

## Development principles

1. **No breaking config after v1.0** — `andre.yml` stays stable  
2. **Tests before/with code** — prefer `cargo test` ACs over manual-only  
3. **Plan via `mp`** — never hand-edit `master-plan/` JSON  
4. **Changelog** — Unreleased on `wip`; version bump on `stable` promote  

Refresh this file from `mp status` + milestone intents before README/doc work.
