# andre — packages, profiles and config folder (orchestrator spec; workers do not edit)

> Implemented. Kept as design rationale; see docs/design/README.md.

Repo /home/agent/coding/andre, branch `wip`. Read AGENTS.md first. Workers never run
git commit/push/checkout/stash/reset/tag. Every batch: `make lint` + `make test` exit 0,
CHANGELOG `[Unreleased]` entry, README/REFERENCE in sync, tests for every new behaviour,
hermetic tests (AGENTS.md rules: `App::with_home`, no global HOME/PATH mutation without the
catch_unwind pattern). andre does NOT run git (no commit/push/pull); it may write files
such as `.gitignore`.

## User decisions (fixed)
- Packages are independent units (a dir inside a group's source, as today). Profiles are
  named collections of packages. Creating a package can optionally add it to profiles.
- No machine remembers an "active" profile: the user picks a profile to work on each time
  (TUI picker; `--profile` for headless).
- Profiles can include other profiles. Cycles are a config error.
- Creating a package from live files = move into the package + symlink back (today's adopt).
- Per-package ignore files, both kinds: `.gitignore` (keep secrets out of git) and
  `.stow-local-ignore` (files never linked).
- The config folder is the single home: `$XDG_CONFIG_HOME/andre/` (→ `~/.config/andre/`)
  holding `andre.yml` and `profiles/`. New `--config-folder <DIR>` points andre at a
  config folder elsewhere (e.g. inside a freshly cloned repo) to solve the bootstrap.

## Data model
Config folder:
```
<config-folder>/
├── andre.yml          # global + groups (unchanged schema)
└── profiles/
    ├── personal.yml
    └── agent-work.yml
```
Profile file (`profiles/<name>.yml`, name = file stem, `[a-z0-9][a-z0-9_-]*`):
```yaml
description: Work servers that run agents   # optional
includes: [agent-base]                      # optional, other profile names
packages:                                   # "group/package" refs
  - home/git
  - home/shell
  - config/nvim
```
- `deny_unknown_fields`. Resolution = own packages ∪ included (recursive), de-duplicated,
  stable order (own first, then includes in listed order). Each resolved entry records
  where it came from (own or via which include) for display.
- Unknown include → error naming the profile; cycle → error naming the cycle path.
- A ref to a group/package that does not exist → warning (not fatal); shown as `missing`.
- Package refs are rewritten when a package or group is renamed through andre, and
  removed (with a toast) when a package is deleted through andre.

## Config discovery (breaking, pre-1.0 — document in CHANGELOG)
Order: `--config <FILE>` → `--config-folder <DIR>` (uses `<DIR>/andre.yml` or `andre.yaml`)
→ `$XDG_CONFIG_HOME/andre/` (Unix `~/.config/andre`, plus the macOS fallback already in
paths.rs). The old cwd / `~/dotfiles` / `~/.dotfiles` lookups are removed; if no config is
found but one exists in a legacy location, the error/onboarding message names it and
suggests `--config-folder <that dir>`. Profiles dir is always `<dir of andre.yml>/profiles`.
`--config` and `--config-folder` together → usage error.

## Bootstrap (fresh machine)
When the loaded config folder is not `~/.config/andre` (canonicalised) and
`~/.config/andre` does not exist, and some group package contains the config folder at the
path that would link to `~/.config/andre` → the main menu shows a one-time
"set up this machine" item/hint that stows exactly that package (normal confirm/execute
flow). After that plain `andre` finds the config. No special code for the reverse case:
putting an existing `~/.config/andre` into the repo is just "new package from live files".

## Ignore files
- `.stow-local-ignore`: GNU Stow semantics (one Perl regex per line, `#` comments; matched
  against the path relative to the package root, with `^/` anchoring; when the file is
  present it REPLACES the default/global ignore list for that package). The native engine
  must honour it with the same semantics (verify against GNU Stow docs and with
  integration tests that compare native vs `stow`). The ignore files themselves must never
  be linked by either engine.
- `.gitignore`: plain file in the package root; andre never interprets it for linking.
- Templates offered at creation: none / common secrets (`*.env`, `.env*`, `*history*`,
  `*.key`, `*.pem`, `secrets/`, caches) — exact list decided by the builder, reviewed.
- Edit both from the TUI with the existing pattern-list editor style (add/remove lines);
  no $EDITOR launch.

## TUI
Main menu (digits 1–8):
1. profiles      2. packages      3. new package   4. unstow
5. verify status 6. settings      7. help          8. quit
(`packages` = today's "stow packages" group→package flow; `new package` replaces
"adopt files"; detail panel facts updated accordingly.)

Global action key: `s` cycles the action stow → restow → unstow (top-bar pill gets the
hotkey highlight on its `s`). Skipped in TextInput/Modal like the other global keys;
check no screen already uses `s`. Persisted only through settings save, like the flags.

Profiles screen: list (name, description, package count incl. includes) + detail panel
(resolved packages with origin and status glyph). Keys: Enter = run the current action on
the whole profile → existing confirm → execute flow; `n` new, `e` edit, `r` rename,
`x` delete (confirm), `/`-free. Edit screen: checkbox list of all packages (grouped) +
includes picker + description field. Status of a profile = status screen filtered to its
resolved packages.

New package flow: pick group → name (new, or existing package to add files to) → source:
empty | live files (existing adopt file picker starting at the group target) → ignore
files (toggle .gitignore with template, toggle .stow-local-ignore) → add to profiles
(multi-select, may be none) → preview (moves, links, files created, profile edits) →
execute. Packages screen: `i` opens the ignore editor for the selected package, `p`
adds/removes the package from profiles.

## Headless
`--yolo --profile <name>` stows (current action) exactly the resolved profile;
`--yolo` alone keeps today's behaviour (all packages). `--profile` without `--yolo` opens
the TUI on that profile's screen. Unknown profile → exit 2 with the list of profiles.

## Batches
- F1 core: config-folder discovery + `--config-folder` + legacy hint; profile model
  (load/validate/resolve/includes/cycles/save/rename/delete, ref rewrite helpers).
- F2 core: `.stow-local-ignore` in the native engine (GNU parity tests), ignore files never
  linked, package creation API (empty / from live files into new or existing package,
  ignore templates, add-to-profiles), generalising adopt.rs.
- F3 TUI: main menu 1–8, `s` action key, profiles screens (list/detail/create/edit/rename/
  delete, run action, filtered status).
- F4 TUI: new package flow (replaces adopt), packages-screen `i` ignore editor and `p`
  profile membership.
- F5: headless `--profile`, bootstrap item, demo fixture with profiles, README/REFERENCE
  full pass, screenshots list in todo.txt.

## Added by user request (after F1 started)
- Screen redesign: every inner screen follows one modern pattern, like the main menu
  (list + detail panel where it fits, consistent titles/hints/selection). The pattern and
  per-screen mockups come from the designer's SCREENS.md; F3/F4 build new screens to it.
- Mouse support: crossterm mouse capture; click selects a row, click on the selected row
  (or double-click) activates, wheel scrolls lists, clicking a top-bar chip toggles that
  flag/action, clicking a bottom-bar hint triggers that key. Hit rects are computed in
  prepare()/update (never during paint) and stored on the component.
- Batches: F6 redesign of the remaining existing screens to SCREENS.md; F7 mouse support.
