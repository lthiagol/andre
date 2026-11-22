# andre — flat packages (groups removed): designer spec; workers do not edit

> Implemented. Kept as design rationale; see docs/design/README.md.

Companion to `FEATURES.md` and `SCREENS.md` (whose §1 pattern, glyphs, hint rules, dialog
rules and mouse rules all still apply). Read `AGENTS.md` first. This file **overrides**
FEATURES.md and SCREENS.md wherever they mention groups, `group/package` refs, folding,
the unstow group list, the wizard's group step or the settings group editor.

Workers never run git commit/push/checkout/stash/reset/tag. Every batch: `make lint` and
`make test` exit 0, the one CHANGELOG entry (§4.0) amended, README/REFERENCE in sync with
what the batch changed, tests for every new behaviour, hermetic test rules (AGENTS.md:
`App::with_home`, no global HOME/PATH mutation without the catch_unwind pattern).

## 0. Decisions

| # | Decision | Why |
|---|----------|-----|
| E1 | `andre.yml` gets two top-level keys: **`packages:`** (the packages folder) and **`target:`** (optional, default `~`). `groups:` is gone. | User decision; one folder, one target. |
| E2 | **`target:` is kept** as an optional key, default the home folder. It is still *one* target for every package; it only lets a config relocate "home". It is not shown as a settings row (see Q4); docs list it under advanced keys. | The demo and e2e fixtures stay hermetic **however they are launched** (`cargo run -- -c demo/andre.yml` by hand included), not only when a Makefile remembers to set `HOME`. 13 of 14 e2e fixtures convert by renaming keys. HOME injection alone would make a hand-run demo with `dry_run: false` link into the real home. Cost: one `Option<String>` and one resolver. |
| E3 | Tests keep using HOME injection (`App::with_home`, `AppContext::for_tests`, `HOME` on spawned children) for everything that derives from home (bootstrap, `.stow-global-ignore`, `~` display); fixtures that link set `target:` to a temp folder only where the test asserts on link paths outside the injected home. | Both tools exist; use the one that states the intent. |
| E4 | Package = every direct subfolder of the packages folder **whose name does not start with `.`** (`.git` is never a package). Names are flat (`nvim`). | A repo root is the natural packages folder and always holds `.git`. See Q3 for other non-package folders. |
| E5 | Profile refs are plain package names. Old `group/package` refs are a profile load issue (profile not loaded, issue shown), no rewrite. | No migration code. |
| E6 | `groups:` in `andre.yml` is a load error with the text in §1.3. `packages:` + `groups:` together is also an error during the interim (§4.0). | No migration code. |
| E7 | Ignore: `global.ignore` (substring list, every package) + per-package `.stow-local-ignore` + `~/.stow-global-ignore` remain. Per-group `ignore` goes. Nothing else read group ignores (checked: `effective_ignores`, `get_group_ignore`, `set_group_ignore`, the settings group menu and `IgnoreScope::Group` are the only users). | User decision. |
| E8 | **Every grouped list becomes a flat list** (packages, profile edit, status, review, execute). The tree widget (`ui/tree.rs`) is replaced by a flat `ui/package_list.rs` and deleted. Folding, tri-state header marks (`▣`) and the fold glyphs go. | Nothing left to group by; profiles do the collecting. |
| E9 | New list key **`*` = mark all / unmark all** on the packages browser and profile edit (replaces "Space on a group header"). See Q2. | Without groups there is no other way to run everything from the TUI. `*` avoids the global letters (SCREENS D5). |
| E10 | Unstow opens **straight into the browse screen at the target**. `TargetSelectComponent` is deleted. | One target. |
| E11 | Wizard loses the group step: `1 name › 2 source › 3 ignore › 4 profiles › 5 review`. | One packages folder. |
| E12 | Settings: the `groups` section and every group dialog go. The `config` section shows `packages` (Enter = folder browse dialog) and `global ignore`. | Only folder-level config is left. |
| E13 | Bootstrap = the package whose folder holds `.config/andre` (relative to the target) and resolves to the loaded config folder. Offered only when the target is the home folder. | Same rule as today, minus the group loop. |
| E14 | The change lands in batches with a **short-lived interim**: G1 adds the flat keys next to `groups:`, screens move to the flat model one batch at a time, G10 deletes groups. `stable` must not be fast-forwarded until G10 is in. See Q1. | Keeps every batch small and green; a single switch would touch ~40 files and ~15 test files at once. |

## 1. Model

### 1.1 Schema (before → after)

Before:

```yaml
global:
  engine: native
  ignore: [.DS_Store]
  stow: { verbosity: 1, action: stow }
groups:
  home:
    source: packages/home
    target: "~"
    ignore: [README.md]
  config:
    source: packages/config
    target: ~/.config
```

After:

```yaml
packages: ~/src/dotfiles     # required; every subfolder is a package (dot-folders skipped)
# target: ~                  # optional; where every package links (default: home folder)
global:                      # optional now (defaults to {})
  engine: native
  ignore: [.DS_Store, README.md]
  stow: { verbosity: 1, action: stow }
```

Profile file (`profiles/personal.yml`):

```yaml
description: Personal laptop
includes: [base]
packages: [ssh, nvim]        # was: [home/ssh, config/nvim]
```

Rust (final state, `andre-core/src/config.rs`):

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The packages folder, as written. `None` until set (a warning, not an error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packages: Option<String>,
    /// Where every package links. `None` = the home folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub global: GlobalSettings,          // GlobalSettings: Default
}

/// One package as every consumer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageEntry {
    pub name: String,        // `nvim` (interim: `home/nvim` for a groups config)
    pub dir: PathBuf,        // <packages>/nvim
    pub source: PathBuf,     // <packages>  (stow -d)
    pub target: PathBuf,     // resolved target (stow -t)
    pub ignores: Vec<String>,// global.ignore (interim: + the group's list)
}

impl Config {
    pub fn packages_dir(&self, config_dir: &Path, home_dir: &Path) -> Option<PathBuf>;
    pub fn target_dir(&self, config_dir: &Path, home_dir: &Path) -> PathBuf;
    pub fn package_entries(&self, config_dir: &Path, home_dir: &Path) -> Vec<PackageEntry>; // name order
    pub fn package(&self, name: &str, config_dir: &Path, home_dir: &Path) -> Option<PackageEntry>;
    pub fn ignores(&self) -> Vec<String>;               // replaces effective_ignores(group)
    pub fn set_packages(&mut self, packages: Option<String>);
}
```

Field order is `packages, target, global` so a saved file reads top-down. Paths resolve
with `resolve_path` (relative → config folder, `~`, `$VAR`), as today. The config folder is
the canonical parent of `andre.yml` (unchanged), so a config inside a stowed package
(`~/src/dotfiles/andre/.config/andre/andre.yml` with `packages: ../../..`) resolves to the
repo even when opened through the `~/.config/andre` link.

`AppContext` gets one convenience: `pub fn package_entries(&self) -> Vec<PackageEntry>`
(and `package(name)`), calling the config methods with its dirs. Screens use these, never
`resolve_path` directly.

### 1.2 Discovery and validation

- Config discovery is unchanged (`--config` → `--config-folder` → `$XDG_CONFIG_HOME/andre`).
- `packages:` unset → `ConfigWarning::NoPackagesFolder` (replaces `NoGroups`):
  `no packages folder: set \`packages:\` in andre.yml`. The TUI opens; lists are empty
  with the empty state of §3.6; settings can set it.
- `packages:` set but missing on disk → load error, as a missing group source is today
  (`App::new` → `validate_sources` → `Error::SourceNotFound`), message
  `Packages folder not found: <path>`.
- `target` equal to or inside the packages folder → load error
  `Error::InvalidConfig("target <t> is inside the packages folder <p>")` (it would link
  packages into themselves). The packages folder inside the target (the normal
  `~/src/dotfiles` under `~`) is fine.
- A missing target folder is not an error (headless creates it, as today).
- New `Error::InvalidConfig(String)` variant; `Display` = the string.

### 1.3 The `groups:` error (G10)

`Config::load` parses to `serde_yaml_ng::Value` first; a top-level `groups` key returns
`Error::InvalidConfig(GROUPS_REMOVED)` before `deny_unknown_fields` runs:

```text
Error loading config /home/u/.config/andre/andre.yml: `groups:` is no longer supported.
andre reads one packages folder now:
  packages: ~/dotfiles   # every subfolder is a package; relative paths start at the config folder
  target: ~              # optional; where links go (default: your home folder)
Profile refs are plain package names: `nvim`, not `home/nvim`.
```

Exit 1 (unchanged path in `main.rs`). A profile with a `home/nvim` ref is a load issue:
`invalid package ref 'home/nvim': refs are plain package names like 'nvim'`.
Interim (G1–G9): `groups:` and `packages:` in one file →
`Error::InvalidConfig("set \`packages:\` or \`groups:\`, not both")`.

### 1.4 Headless

- `--yolo`: runs the current action on every package (name order). Output, one line per
  package then the unchanged totals line:

  ```text
  ok    git
  ok    shell
  FAIL  kitty  (engine=native): existing target is not owned by stow: .config/kitty/kitty.conf
  2 succeeded, 1 failed (total: 3)
  ```

  `ok` lines go to stdout, `FAIL` lines to stderr. The `group(s) skipped` line goes.
  No packages folder → stderr `no packages folder: set \`packages:\` in <config path>`,
  exit 1. Empty packages folder → `0 succeeded, 0 failed (total: 0)`, exit 0.
- `--yolo --profile <name>`: only the resolved profile's packages; missing refs are the
  existing `profile warning:` lines; unknown profile exit 2 with the list; unresolvable
  (cycle, unknown include) exit 1. Unchanged except refs are names.
- `--profile <name>` without `--yolo`: unchanged (TUI on that profile).
- `--onboard`: writes `global: {}\n` (no `groups: {}`) and opens settings with the cursor
  on the `packages` row.
- Core API: `yolo::build_plan(config, config_dir, home) -> Vec<PackageEntry>`,
  `yolo::profile_plan(.., &ResolvedProfile) -> Vec<PackageEntry>`,
  `yolo::run_package(entry, engine, flags, global_ignore) -> Result<()>`. `StowJob` and
  `run_job` are deleted.

## 2. Where groups live today, and what replaces them

`D` = delete, `R` = replace, `K` = keep (rename noted). Batch in brackets.

### 2.1 Core (`andre-core`)

| Place | Today | After |
|-------|-------|-------|
| `config.rs` `Config.groups`, `Group` | map of source/target/ignore | R by `packages`, `target` (§1.1) [G1 add, G10 D] |
| `resolve/get_group_source/target`, `resolve_group_path`, `validate_sources` | per group | R `packages_dir`, `target_dir`, `package(_entries)`; `validate_sources` checks the packages folder [G1; old D G10] |
| `effective_ignores(group)`, `get_group_ignore`, `set_group_ignore` | global + group list | R `ignores()` [G1; old D G10] |
| `add/remove/rename_group`, `update_group_source/target` | settings group editor | R `set_packages` [G1]; old D [G10] |
| `ConfigWarning::NoGroups` | warning | R `NoPackagesFolder` [G1] |
| `packages.rs` `discover_packages` | every subdir | K, skips dot-folders (E4) [G1] |
| `profile.rs` ref rule `group/package`, `split_package_ref` | `home/git` | R plain names (`create::is_valid_package_name`) [G1 accepts both, G10 names only]; `split_package_ref` D [G10] |
| `profile.rs` `rename_group`, `remove_group` | settings | D [G6] |
| `profile.rs` `known_packages` | `group/pkg` per group | R names of `package_entries` [G1] |
| `engine.rs` `LinkOp.group` | display + logs | R `name` [G2]; `name` D in G10 (== `package`) |
| `stow.rs` `ignored_by_group` | substring list | K, rename `ignored_by_config` [G2] |
| `yolo.rs` `StowJob`, `build_plan`, `profile_plan`, `run_job` | group jobs | R §1.4 [G2] |
| `bootstrap.rs` `Bootstrap.group`, group loop | | R loop over `package_entries`; `Bootstrap { package, link }` [G2] |
| `create.rs` `NewPackage.group` | ref prefix | R `package_ref: String` [G2]; D in G10 (ref = `package`) |
| `state.rs`, `lib.rs` re-exports | `Group` | D `Group` export [G10]; export `PackageEntry` [G1] |
| `error.rs` | | add `InvalidConfig(String)` [G1] |
| `adopt.rs` | target listing for unstow/picker | K (not group-specific) |

### 2.2 TUI (`andre`)

| Area | Today | After | Batch |
|------|-------|-------|-------|
| `execute.rs` `build_execution_results(HashMap<group, Vec<pkg>>)`, `ExecutionResult.group` | | takes `&[String]` names; `ExecutionResult { name, success, error_message }` | G2 (shape), G3 (names) |
| `components/execute.rs`, `ui/execute.rs` | `group/package` rows | `name` rows | G2 |
| `status_cache.rs` key `(group, pkg)`, `prefetch_group` | | key = name; `prefetch(entry, global)`, `prefetch_all(entries, global)`; passes `entry.ignores` to `check_stowed_status_with_ignores` (closes the todo.txt status-ignores item) | G3 |
| `ui/tree.rs` | grouped, foldable, markable | **D**; new `ui/package_list.rs` (§3.0) | G3 add, G5 D |
| `components/confirm.rs` (review) | tree of groups, `HashMap` input, group detail | flat list, `Vec<String>` input, package detail only; `stow -d <packages> -t <target> -S <pkg>` | G3 |
| `components/menu.rs` facts + descriptions + bootstrap | groups, `N in M groups`, targets list | §3.1 facts; bootstrap review by name | G4 |
| `components/packages` | tree, group rows/detail, `listing` map, fold keys | flat list, `*` all, no group detail | G4 |
| `components/profiles/mod.rs`, `view.rs` | status via `split_package_ref`, group hash | status by name; hash `packages`/`target` | G4 |
| `components/profiles/edit*.rs` | tree of groups, missing-group rows | flat list; missing refs as `? name  missing` rows after the known ones; `*` all | G5 |
| `components/status` | tree, fold, folded summaries, `group_header` | flat list; no fold keys/hints | G5 |
| `ui/icons.rs` | `checkbox_some`, `fold_open`, `fold_closed` | D `checkbox_some`, `fold_closed`; `fold_open` → `caret` (status filter chip `▾`) | G5 |
| `components/help/topics.rs` | `←→ fold / unfold groups`, `▣`, `▾ ▸`, group sentences | §3.10 | G4 (packages), G5 (keys, glyphs), G7 (new package) |
| `components/settings/*`, `ui/settings.rs` | `groups +` section, `Row::Group`, group menu/name/remove/browse dialogs, `GroupChange`, `Snapshot`, `NameTarget`, `Pick::*`, `IgnoreScope::Group`, `refs_into` & co | `Row::Packages`; one browse dialog (`Pick::Packages`); ignore editor global-only; all group types D | G6 |
| `main.rs` `--onboard` content | `groups: {}` | `global: {}` + cursor on `packages` | G6 |
| `components/wizard/*` | `Step::Group`, `group_list`, `listing` map, picker at group target, `group/pkg` refs | steps renumbered (E11); listing = `package_entries`; picker at `target_dir`; ref = name | G7 |
| `ui/step_bar.rs` tests | `group` step | `name` step | G7 |
| `ui/footer.rs` | `wizard_group_hints`, `UnstowTargetSelect`, fold hints, `settings_hints(on_group)`, `+ group` | D / `settings_hints()` | G4–G8 |
| `components/target_select.rs` | unstow group list | **D**; menu pushes `UnstowBrowseComponent::at(target_dir)` | G8 |
| `components/unstow/browse.rs` `group_name`, owning package via group source | | no group; owning package = first component of the link's path under the packages folder | G8 |
| `ui/status_bar.rs` crumb `UnstowTargetSelect` | | D (the `top_bar_group` helpers are span groups: keep) | G8 |
| `main.rs` `run_yolo`, `headless_plan` | per group | §1.4 | G2 |
| Mouse | fold cells, target-select list, settings group dialogs, wizard group list | gone with their screens; mark-cell click stays (via `package_list::mark_hit`) | G3–G8 |

Dead after the change, all **deleted** (no compat shims left): `ui/tree.rs`,
`components/target_select.rs`, `Mark::Some`, `Glyph::Fold`, `icons.checkbox_some`,
`icons.fold_closed`, `wizard_group_hints`, `Step::Group`, every settings group type,
`ProfileSet::{rename_group, remove_group}`, `split_package_ref`, `StowJob`, `run_job`,
`Config::{groups, *group*}`, `Group`, `ConfigWarning::NoGroups`, `LinkOp.name`,
`NewPackage.package_ref`, tests/e2e `03-multi-group-config`.

## 3. Screens (80x24, unicode icons)

Only screens whose look changes. Everything else in SCREENS.md §2 stands (dialogs, leave
dialog, pickers, ignore editor, too-small, profiles empty/new/delete).

### 3.0 Shared widget `ui/package_list.rs` (replaces `ui/tree.rs`)

```rust
pub enum Mark { On, Off, Inherited(String) }               // ▣ is gone
pub struct PackageRow { pub name: String, pub status: Option<StowStatus>,
                        pub mark: Option<Mark>, pub meta: String }
pub enum Toggle { Changed, Inherited { via: String }, Unmarkable }
pub struct PackageList { rows: Vec<PackageRow> }
impl PackageList {
    pub fn new(rows: Vec<PackageRow>) -> Self;
    pub fn rows(&self) -> &[PackageRow];
    pub fn set_status(&mut self, name: &str, status: Option<StowStatus>) -> bool;
    pub fn toggle_mark(&mut self, row: usize) -> Toggle;
    /// `*`: marks every own-markable row; if all already are, unmarks them. Inherited rows untouched.
    pub fn toggle_all(&mut self) -> Toggle;
    pub fn marked(&self) -> Vec<String>;                   // list order
}
pub fn package_line(row: &PackageRow, checkboxes: bool, icons: &Icons,
                    c: &ThemeColors, width: u16) -> Line<'static>;
pub fn mark_hit(row: &PackageRow, col: u16, checkboxes: bool) -> bool; // mouse mark cell
```

Row columns: `[mark] [status] name ……… meta`, i.e. tree rows minus the 4-col group
indent and the fold column. Status word column from 44 cols, inherited `via <profile>` in
`text_dim`, unknown `·`, cut-before-meta: same rules and tests as tree.rs (port them).

### 3.1 Main menu

Packages item selected. Facts: packages → `folder`, `packages`, `stowed`, `links to`;
new package → `folder`, `packages`, `links to`; unstow → `target`; profiles → unchanged
(`packages 9`, no `in N groups`); settings → unchanged. Descriptions: packages
`Pick packages, then run the current action on them.`; new package
`Create a package, empty or from files that already live in ~.`; unstow
`Remove links from the target.`; settings
`Edit the packages folder, theme, icons and stow options.`

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│                                                                              │
│                   ╔════╗          ╔════╗          ╔════╗                     │
│                   ║ A. ║  ╔════╗  ║ D. ║  ╔════╗  ║ E. ║                     │
│                   ╚════╝  ║ N. ║  ╚════╝  ║ R. ║  ╚════╝                     │
│                           ╚════╝          ╚════╝                             │
│                                                                              │
│                   Another Neat Dotfile Repository Engine                     │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ actions ───────────────────────╮ ╭ packages ─────────────────────────────╮ │
│ │ ◈  profiles                  1 │ │ Pick packages, then run the current   │ │
│ │▌⇢  packages                  2 │ │ action on them.                       │ │
│ │ +  new package               3 │ │                                       │ │
│ │ ≠  unstow                    4 │ │ folder    ~/src/dotfiles              │ │
│ │ ✓  verify status             5 │ │ packages  9                           │ │
│ │ ≡  settings                  6 │ │ stowed    6 of 9                      │ │
│ │ ?  help                      7 │ │ links to  ~                           │ │
│ │ ×  quit                      8 │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  1–8 jump  Enter open                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│                                                                              │
│                   ╔════╗          ╔════╗          ╔════╗                     │
│                   ║ A. ║  ╔════╗  ║ D. ║  ╔════╗  ║ E. ║                     │
│                   ╚════╝  ║ N. ║  ╚════╝  ║ R. ║  ╚════╝                     │
│                           ╚════╝          ╚════╝                             │
│                                                                              │
│                   Another Neat Dotfile Repository Engine                     │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ actions ───────────────────────╮ ╭ new package ──────────────────────────╮ │
│ │ ◈  profiles                  1 │ │ Create a package, empty or from files │ │
│ │ ⇢  packages                  2 │ │ that already live in ~.               │ │
│ │▌+  new package               3 │ │                                       │ │
│ │ ≠  unstow                    4 │ │ folder    ~/src/dotfiles              │ │
│ │ ✓  verify status             5 │ │ packages  9                           │ │
│ │ ≡  settings                  6 │ │ links to  ~                           │ │
│ │ ?  help                      7 │ │                                       │ │
│ │ ×  quit                      8 │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  1–8 jump  Enter open                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Bootstrap (facts: `package andre`, `from <packages>/andre`):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main                                                                  │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ actions ───────────────────────╮ ╭ set up this machine ──────────────────╮ │
│ │▌⌂  set up this machine       0 │ │ This config folder lives in a repo.   │ │
│ │ ◈  profiles                  1 │ │ Link it to ~/.config/andre so plain   │ │
│ │ ⇢  packages                  2 │ │ `andre` finds it next time.           │ │
│ │ +  new package               3 │ │                                       │ │
│ │ ≠  unstow                    4 │ │ package   andre                       │ │
│ │ ✓  verify status             5 │ │ links     ~/.config/andre             │ │
│ │ ≡  settings                  6 │ │ from      ~/src/dotfiles/andre        │ │
│ │ ?  help                      7 │ │ action    stow (always)               │ │
│ │ ×  quit                      8 │ │                                       │ │
│ │                                │ │ Enter reviews the link first.         │ │
│ │                                │ │ Shown until ~/.config/andre exists.   │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  0–8 jump  Enter open                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.2 Profiles (detail refs are names)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ profiles 3 ────────────────────╮ ╭ agent-work ───────────────────────────╮ │
│ │▌◈ agent-work                 5 │ │ Work servers that run agents          │ │
│ │ ◈ base                       3 │ │                                       │ │
│ │ ◈ personal                   7 │ │ includes  base                        │ │
│ │                                │ │ packages  5  (2 own, 3 via base)      │ │
│ │                                │ │ status    ✓ 3  → 1  ✗ 1               │ │
│ │                                │ │                                       │ │
│ │                                │ │ ✓ aws                                 │ │
│ │                                │ │ → kube                                │ │
│ │                                │ │ ✓ git                 via base        │ │
│ │                                │ │ ✓ shell               via base        │ │
│ │                                │ │ ✗ andre               via base        │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter stow  + new  e edit  r rename  x delete  c check         theme  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.3 Profile edit (flat list, `*` all)

Missing refs (in the profile, not on disk) are extra rows after the known packages:
`■ ? oldpkg      missing`, toggleable (unmarking drops the ref on save).

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles › personal                                            │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 3 of 9 ───────────────╮ ╭ personal · edited ● ──────────────────╮ │
│ │ □ ✓ andre                      │ │ description                           │ │
│ │ □ ✓ aws                        │ │  Personal laptop█                     │ │
│ │ ■ ✓ git              via base  │ │                                       │ │
│ │ □ → kitty                      │ │ includes                              │ │
│ │ □ → kube                       │ │  ■ base                               │ │
│ │ □ ✓ nvim                       │ │  □ agent-work                         │ │
│ │ ■ ✓ shell            via base  │ │                                       │ │
│ │▌■ ✓ ssh                        │ │ resolved  3 packages                  │ │
│ │ □ → tmux                       │ │ own       1  ssh                      │ │
│ │                                │ │ via base  2  git, shell               │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Tab pane  Space toggle  * all  w save  Esc back        theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.4 Packages browser (flat)

- Enter: current action on the marks, else the cursor package → review. `*` marks all /
  unmarks all. `←`/`→` do nothing here now (no folding). `i`, `p` unchanged.
- Detail: status headline, `folder` (package dir), `links to` (target), files, ignore,
  profiles, per-file section.

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 1 of 9 ───────────────╮ ╭ shell ────────────────────────────────╮ │
│ │ □ ✗ andre                      │ │ ◐ partial — some files linked         │ │
│ │ □ ✓ aws                        │ │                                       │ │
│ │ □ ✓ git                        │ │ folder    ~/src/dotfiles/shell        │ │
│ │ □ → kitty                      │ │ links to  ~                           │ │
│ │ □ → kube                       │ │ files     4  (2 linked)               │ │
│ │ □ ✓ nvim                       │ │ ignore    .gitignore ✓  stow ✗        │ │
│ │▌■ ◐ shell                      │ │ profiles  base, personal              │ │
│ │ □ ✓ ssh                        │ │                                       │ │
│ │ □ → tmux                       │ │ ✓ .bashrc                             │ │
│ │                                │ │ ✓ .profile                            │ │
│ │                                │ │ → .zshrc                              │ │
│ │                                │ │ → .config/starship.toml               │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter stow 1  Space mark  * all  i ignore  p profiles  Esc back       q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Membership dialog: title is `<name> in profiles`:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 1 of 9 ───────────────╮ ╭ shell ────────────────────────────────╮ │
│ │ □ ✗ andre                      │ │ ◐ partial — some files linked         │ │
│ │ □ ✓ aws         ╭ shell in profiles ───────────────────╮                 │ │
│ │ □ ✓ git         │▌■ base                               │                 │ │
│ │ □ → kitty       │ ■ personal                           │                 │ │
│ │ □ → kube        │ □ agent-work                         │                 │ │
│ │ □ ✓ nvim        │                                      │                 │ │
│ │▌■ ◐ shell       │ Space toggle                         │                 │ │
│ │ □ ✓ ssh         │       [ Esc cancel ]  [ Enter save ] │                 │ │
│ │ □ → tmux        ╰──────────────────────────────────────╯                 │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space toggle  Enter save  Esc cancel                   theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.5 Packages browser — no packages folder / empty folder

The `6 settings` line is the clickable empty hint (it pushes settings with the cursor on
`packages`). For a set but empty folder: list text `no packages in ~/src/dotfiles`, hint
`3 new package`, detail headline without the `!` line.

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 0 ────────────────────╮ ╭ packages ─────────────────────────────╮ │
│ │                                │ │ ! andre.yml sets no `packages:`       │ │
│ │                                │ │                                       │ │
│ │                                │ │ A package is a folder inside the      │ │
│ │                                │ │ packages folder. Its files mirror     │ │
│ │       no packages folder       │ │ your home folder: shell/.bashrc       │ │
│ │                                │ │ links to ~/.bashrc.                   │ │
│ │     6 settings                 │ │                                       │ │
│ │                                │ │ Set the folder in settings, or add    │ │
│ │                                │ │ `packages: ~/dotfiles` to andre.yml.  │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Esc back                                               theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.6 New-package wizard (5 steps)

Step 1 — name (was step 2; Esc on step 1 = leave, leave dialog if anything typed):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  [1 name] › 2 source › 3 ignore › 4 profiles › 5 review                      │
│                                                                              │
│ ╭ name ──────────────────────────╮ ╭ new package ──────────────────────────╮ │
│ │  name  nv█                     │ │ Creates nv/ in the packages folder,   │ │
│ │                                │ │ an empty package.                     │ │
│ │▌+ create `nv`                  │ │                                       │ │
│ │   existing:                    │ │ path      ~/src/dotfiles/nv           │ │
│ │   → nvim  (add files to it)    │ │ links to  ~                           │ │
│ │                                │ │                                       │ │
│ │                                │ │ Type to filter existing packages;     │ │
│ │                                │ │ pick one to add files to it.          │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ type name  ↑↓ pick  Enter next  Esc cancel             theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 2 — source:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ name › [2 source] › 3 ignore › 4 profiles › 5 review                      │
│                                                                              │
│ ╭ source ────────────────────────╮ ╭ move live files in ───────────────────╮ │
│ │ ○ empty package                │ │ Pick files that already live in ~.    │ │
│ │▌● move live files in           │ │ andre moves them into the package     │ │
│ │                                │ │ and links them back. Nothing is       │ │
│ │                                │ │ lost; the files keep working.         │ │
│ │                                │ │                                       │ │
│ │                                │ │ Next: a file picker opens at ~.       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  Enter next  Esc back                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 2b — live files (opens at the target):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package › files                                            │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ name › [2 source] › 3 ignore › 4 profiles › 5 review                      │
│                                                                              │
│ ╭ ~/.config/nvim ────────────────╮ ╭ init.lua ─────────────────────────────╮ │
│ │ ■ / lua                        │ │ kind      file, 2.1 KB                │ │
│ │▌■ • init.lua                   │ │ moves to  nvim/.config/nvim/          │ │
│ │ □ • lazy-lock.json             │ │ marked    2 items                     │ │
│ │ □ ~ spell  → ~/sync/spell      │ │                                       │ │
│ │ □ / .cache                     │ │ ~ symlinks cannot be moved in.        │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space mark  → open  ← up  Enter next  Esc back         theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 3 — ignore: unchanged body, bar `✓ name › ✓ source › [3 ignore] › 4 profiles › 5 review`.

Step 4 — profiles:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ name › ✓ source › ✓ ignore › [4 profiles] › 5 review                      │
│                                                                              │
│ ╭ add to profiles ───────────────╮ ╭ personal ─────────────────────────────╮ │
│ │▌■ personal                     │ │ Personal laptop                       │ │
│ │ □ base                         │ │                                       │ │
│ │ □ agent-work                   │ │ packages  7                           │ │
│ │                                │ │ after     8 (+ nvim)                  │ │
│ │                                │ │                                       │ │
│ │                                │ │ None is fine: the package still       │ │
│ │                                │ │ shows under packages.                 │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space toggle  Enter next  Esc back                     theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 5 — review:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ name › ✓ source › ✓ ignore › ✓ profiles › [5 review]                      │
│                                                                              │
│ ╭ review · new package nvim ───────────────────────────────────────────────╮ │
│ │ move   ~/.config/nvim/init.lua → nvim/.config/nvim/                      │ │
│ │ move   ~/.config/nvim/lua/     → nvim/.config/nvim/                      │ │
│ │ link   ~/.config/nvim/init.lua ⇢ package                                 │ │
│ │ link   ~/.config/nvim/lua      ⇢ package                                 │ │
│ │ create nvim/.gitignore  (common secrets, 7 lines)                        │ │
│ │ edit   profiles/personal.yml  + nvim                                     │ │
│ │                                                                          │ │
│ │ 2 moves · 2 links · 1 file · 1 profile                                   │ │
│ │                                                                          │ │
│ │                                                                          │ │
│ ╰──────────────────────────────────────────────────────────────────────────╯ │
│                                               [ Esc back ]  [ Enter create ] │
│                                                                              │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter create  Esc back                                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.7 Review and execute

Review: flat list in name order; detail command uses the packages folder.

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages › review                                              │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ stow 3 packages ───────────────╮ ╭ shell ────────────────────────────────╮ │
│ │ → kitty                        │ │ stow shell                            │ │
│ │▌◐ shell                        │ │                                       │ │
│ │ → tmux                         │ │ engine    native                      │ │
│ │                                │ │ target    ~                           │ │
│ │                                │ │ links     2 new, 2 kept               │ │
│ │                                │ │                                       │ │
│ │                                │ │ $ stow -d ~/src/dotfiles -t ~         │ │
│ │                                │ │   -S shell                            │ │
│ │                                │ │                                       │ │
│ │                                │ │ dry-run is off: files change.         │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│                                                   [ n cancel ]  [ y stow 3 ] │
│──────────────────────────────────────────────────────────────────────────────│
│ y stow  n cancel  ↑↓ inspect                           theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages › run                                                 │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  done  ✓ 2 succeeded  ✗ 1 failed                                             │
│ ╭ results ───────────────────────╮ ╭ kitty ────────────────────────────────╮ │
│ │▌✗ kitty                        │ │ stow failed                           │ │
│ │ ✓ shell                        │ │                                       │ │
│ │ ✓ tmux                         │ │ ✗ existing target is not owned by     │ │
│ │                                │ │   stow: .config/kitty/kitty.conf      │ │
│ │                                │ │                                       │ │
│ │                                │ │ A real file sits where the link       │ │
│ │                                │ │ goes. Move it into the package        │ │
│ │                                │ │ with `new package` → existing         │ │
│ │                                │ │ package, or delete it.                │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│                                                               [ Enter done ] │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter done  ↑↓ inspect                                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.8 Status

No fold keys, no folded summaries. Filter dropdown and scanning state as SCREENS 2.10
with flat rows.

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › status                                                         │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  profile [all ▾]          ✓ 5  ◐ 1  → 2  ✗ 1                                 │
│ ╭ packages 9 ────────────────────╮ ╭ andre ────────────────────────────────╮ │
│ │▌✗ andre                        │ │ ✗ conflict — a real file sits         │ │
│ │ ✓ aws                          │ │   where a link must go                │ │
│ │ ✓ git                          │ │                                       │ │
│ │ ✓ kitty                        │ │ target    ~/.config/andre             │ │
│ │ → kube                         │ │                                       │ │
│ │ ✓ nvim                         │ │ ✗ andre.yml      real file            │ │
│ │ ◐ shell                        │ │ ✓ profiles/      linked               │ │
│ │ ✓ ssh                          │ │                                       │ │
│ │ → tmux                         │ │ Fix: new package → andre              │ │
│ │                                │ │ → move live files in.                 │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ f profile  r rescan  Esc back                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles › status                                              │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  profile [agent-work ▾]   ✓ 3  → 1  ✗ 1                                      │
│ ╭ agent-work 5 ──────────────────╮ ╭ andre ────────────────────────────────╮ │
│ │▌✗ andre              via base  │ │ ✗ conflict — a real file sits         │ │
│ │ ✓ aws                          │ │   where a link must go                │ │
│ │ ✓ git                via base  │ │                                       │ │
│ │ → kube                         │ │                                       │ │
│ │ ✓ shell              via base  │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ f profile  r rescan  Esc back                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.9 Unstow (browse is the first screen)

Crumb is `main › unstow`; the list title is the current folder (`~`, `~/.config`, …).
`←` at the target does nothing; Esc pops to the main menu.

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › unstow                                                         │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ ~  · 2 marked ─────────────────╮ ╭ .bashrc ──────────────────────────────╮ │
│ │ □ / .config                    │ │ link      ~/.bashrc                   │ │
│ │ □ / .local                     │ │ points to src/dotfiles/shell/.bashrc  │ │
│ │▌■ ~ .bashrc                    │ │ package   shell                       │ │
│ │ ■ ~ .profile                   │ │                                       │ │
│ │ □ ~ .tmux.conf                 │ │ Unlinking leaves the package file     │ │
│ │ □ ~ .zshrc                     │ │ in place; only the link goes.         │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space mark  → open  ← up  Enter review  Esc back       theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › unstow › review                                                │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ remove 2 links ────────────────╮ ╭ .bashrc ──────────────────────────────╮ │
│ │▌~ .bashrc                      │ │ remove    ~/.bashrc                   │ │
│ │ ~ .profile                     │ │ points to shell/.bashrc               │ │
│ │                                │ │                                       │ │
│ │                                │ │ The source file stays in the          │ │
│ │                                │ │ package. Restow shell to put the      │ │
│ │                                │ │ link back.                            │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│                                                 [ n cancel ]  [ y remove 2 ] │
│──────────────────────────────────────────────────────────────────────────────│
│ y remove  n cancel  ↑↓ inspect                         theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.10 Settings

`config` section rows: `packages` and `global ignore`. No `groups` section, no `+`/`x`.
Packages detail: what it is, now/saved, package count in the pending folder, `links to`
(the target; `(target: in andre.yml)` suffix when the key is set), file/paths. Enter opens
the folder browse dialog (the old group source browse, retitled `packages folder`); the
pending value is written as the old source browse wrote it. Changing the folder does not
touch profiles (refs naming packages the new folder lacks show `?` / missing).

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings ──────────────────────╮ ╭ packages ─────────────────────────────╮ │
│ │ stow options                   │ │ Every folder here is a package.       │ │
│ │   action          stow         │ │                                       │ │
│ │   verbose         0            │ │ now       ~/src/dotfiles              │ │
│ │   dry-run         ✗            │ │ saved     ~/src/dotfiles              │ │
│ │   no-fold         ✗            │ │ packages  9                           │ │
│ │   adopt           ✗            │ │ links to  ~                           │ │
│ │   dotfiles        ✗            │ │                                       │ │
│ │ appearance                     │ │ file      ~/.config/andre/andre.yml   │ │
│ │   theme           default      │ │ paths     relative to ~/.config/      │ │
│ │   icons           unicode      │ │           andre                       │ │
│ │ config                         │ │                                       │ │
│ │▌  packages        ~/src/dotfil…│ │ Enter picks another folder.           │ │
│ │   global ignore   3 patterns   │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter change  w save  Esc back                         theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings ──────────────────────╮ ╭ packages ─────────────────────────────╮ │
│ │   theme╭ packages folder ─────────────────────────────────────╮ge.       │ │
│ │   icons│ ~/src                                                │          │ │
│ │ config │                                                      │          │ │
│ │▌  packa│ / ..                                                 │          │ │
│ │   globa│▌/ dotfiles                                           │          │ │
│ │        │ / notes                                              │          │ │
│ │        │ / work                                               │          │ │
│ │        │            [ Esc cancel ]  [ Enter choose dotfiles ] │          │ │
│ │        │                                                      │          │ │
│ │        ╰──────────────────────────────────────────────────────╯          │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ → open  ← up  Enter choose  Esc cancel                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 3.11 Help

Keys topic drops `←→ fold / unfold groups`, gains `* mark / unmark all`. Glyph topic drops
`▣` and `▾ ▸`; `?` reads `no such package folder`. Packages topic: `A package is a folder
in the packages folder. Its files mirror your home folder, so shell/.bashrc links to
~/.bashrc.` and `Enter runs the action on the marks, or on the cursor's package.` New
package topic: `Name the package: a new one, or an existing one to add files to.`

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › help                                                           │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ help ──────────────────────────╮ ╭ keys ─────────────────────────────────╮ │
│ │▌keys                           │ │ everywhere                            │ │
│ │ mouse                          │ │  ↑↓ / jk    move                      │ │
│ │ status glyphs                  │ │  Enter      open / run                │ │
│ │ profiles                       │ │  Space      mark                      │ │
│ │ packages                       │ │  Esc        back                      │ │
│ │ new package                    │ │  s          cycle the action          │ │
│ │ ignore files                   │ │  v d n a o  stow options              │ │
│ │ about                          │ │  t          theme                     │ │
│ │                                │ │  q          quit                      │ │
│ │                                │ │                                       │ │
│ │                                │ │ lists                                 │ │
│ │                                │ │  *          mark / unmark all         │ │
│ │                                │ │  + e r x    new, edit, rename, delete │ │
│ │                                │ │  w          save edits                │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ topic  PgUp/PgDn scroll  Esc back                   theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › help                                                           │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ help ──────────────────────────╮ ╭ status glyphs ────────────────────────╮ │
│ │ keys                           │ │ ✓  stowed    every file linked        │ │
│ │ mouse                          │ │ →  unstowed  no links yet             │ │
│ │▌status glyphs                  │ │ ◐  partial   some files linked        │ │
│ │ profiles                       │ │ ✗  conflict  real file in the way     │ │
│ │ packages                       │ │ ?  missing   no such package folder   │ │
│ │ new package                    │ │ ∅  empty     package has no files     │ │
│ │ ignore files                   │ │ ·  unknown   not scanned yet          │ │
│ │ about                          │ │                                       │ │
│ │                                │ │ ■ □    marked, unmarked               │ │
│ │                                │ │ ● ○    chosen, not chosen             │ │
│ │                                │ │ / ~ •  folder, symlink, file          │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ topic  PgUp/PgDn scroll  Esc back                   theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Unchanged except text: ignore editor crumb `main › packages › shell › ignore`; global ignore
editor title `ignore · global`; profile delete dialog; leave dialog.

## 4. Build plan

### 4.0 Rules for every G batch

- Order: G1 → G2 → G3 → G4 → G5 → G6 → G7 → G8 → G9a → G9b → G10. Each leaves `make lint`
  and `make test` exit 0. `stable` is not fast-forwarded before G10 (wip is mid-change).
- **Interim (G1–G9b):** a config may use `groups:` (old) or `packages:` (new), not both.
  Core APIs from G1 serve both through `PackageEntry` (group configs yield names
  `group/pkg`). Screens moved in G3–G8 read only `PackageEntry` / `packages_dir` /
  `target_dir`, so they work for both, except the wizard, settings and unstow, which
  are flat-only from their batch on (a groups config sees an empty state there). Their
  tests move to flat fixtures in that batch.
- **CHANGELOG:** G1 adds one entry under `[Unreleased]` → `### Changed (breaking)`:
  `**Groups removed: one packages folder.**` Each later batch extends that entry's bullet
  list (no new top-level entry). Other non-breaking additions (`*`, per-package yolo
  lines) go under `### Added` / `### Changed`.
- Docs: REFERENCE "Full Configuration Reference" switches to §1.1 in G1; each batch
  updates the README/REFERENCE sections for its screens; G10 does the full sweep.
- Render tests are `andre/tests/render.rs` at 80x24 with unicode icons, plus a no-panic
  render at 120x40 and 200x50 for every screen the batch touches.
- Fixture shape for flat tests (helpers get `make_flat_test_app()` in G3, which becomes
  `make_test_app()` in G9b):
  ```yaml
  packages: dotfiles
  global:
    stow: { verbosity: 0 }
  ```
  with `HOME` = the temp dir (`App::with_home`), packages under `<tmp>/dotfiles/<name>`.

### G1 — core: flat config model next to groups
`config.rs`, `error.rs`, `packages.rs`, `profile.rs`, `lib.rs`.
- `Config` gains `packages`, `target` (§1.1 order and serde attrs); `groups` becomes
  `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`; `global` gets
  `#[serde(default)]`. `Error::InvalidConfig`. Both keys → error (§1.3 interim text).
- `PackageEntry`, `packages_dir`, `target_dir`, `package_entries`, `package`, `ignores`,
  `set_packages`; `validate_sources` also checks the packages folder; target-inside-packages
  error (§1.2); `NoPackagesFolder` replaces `NoGroups` (warn when neither key yields
  packages config).
- `discover_packages` skips dot-folders.
- Profiles: a ref is valid if it is a package name or `group/package`; `known_packages`
  = `package_entries` names; `InvalidPackageRef` message `expected a package name like 'nvim'`.
- Tests (unit): flat config round-trips with key order `packages, target, global`;
  `global` may be omitted; `target_dir` defaults to home and resolves relative to the
  config folder; `package_entries` lists `git`, `nvim` but not `.git`, with
  `source`/`target`/`ignores` set; groups config yields `home/git` entries with the group's
  target and global+group ignores; both keys → `InvalidConfig` naming both; target inside
  packages → error; missing packages folder → `SourceNotFound`; profile with `[nvim]`
  resolves against flat `known_packages`.
- Docs: REFERENCE config section + `andre.yml.example` rewritten to §1.1 (the example
  file is not loaded by tests; check `grep -n groups andre.yml.example` is empty).
  CHANGELOG entry created.

### G2 — core consumers: yolo, bootstrap, create, LinkOp
`yolo.rs`, `bootstrap.rs`, `create.rs`, `engine.rs`, `stow.rs`, `andre/src/main.rs`,
`andre/src/execute.rs`, `components/execute.rs`, `ui/execute.rs`, `components/menu.rs`
(bootstrap only), `components/wizard/finish.rs` (field rename only).
- §1.4 yolo API and output; `LinkOp.group` → `name`; `ExecutionResult { name, .. }`;
  `ignored_by_group` → `ignored_by_config`; `Bootstrap { package, link }` over
  `package_entries`; `NewPackage.group` → `package_ref` (wizard passes `group/pkg` for now).
- Tests: yolo flat plan = every entry in name order; `profile_plan` keeps only resolved
  names; bootstrap finds `andre` in a flat fixture and offers nothing when `target:` is
  not home; e2e scenario 01 copied as `15-flat-basic` (flat yml, child `HOME` = temp)
  asserting per-package `ok` lines and links; execute result rows render `name`.
- Smoke: `andre --yolo` on a flat config with no `packages:` exits 1 with
  `no packages folder`.

### G3 — package list widget, status cache, review
New `ui/package_list.rs` (§3.0) with tree.rs's row tests ported (minus fold/header);
`status_cache.rs` keyed by name with entry ignores; `components/confirm.rs` flat (§3.7);
`execute::build_execution_results(&[String], ..)`; callers (packages Enter, profiles
Enter, menu bootstrap) pass names. Helpers: `make_flat_test_app()`.
- Render: review list title `stow 3 packages`, rows `◐ shell` without indent, no `▾`;
  detail `$ stow -d ~/…/dotfiles -t ~` on a flat fixture.
- Unit: `toggle_all` marks all then unmarks all, never inherited rows; `marked()` order;
  status cache passes `global.ignore` (a file matching it does not make a package partial).
- todo.txt: remove the "Pass the group's ignore list to status" item.

### G4 — packages browser, menu facts, profiles screen
`components/packages/*` flat (§3.4, §3.5) with `*`; menu facts/descriptions (§3.1);
profiles list/detail status lookups by name; footer `packages_hints` without fold;
help packages topic text.
- Render: no `▾`/`▸` on the packages screen; row `▌■ ◐ shell`; empty state
  `no packages folder` with `6 settings`; menu packages detail `folder` and `links to`,
  no `group` anywhere on the menu.
- Interaction: `*` marks all 9 then unmarks; Enter with no marks reviews the cursor
  package; `←` leaves the cursor where it is; clicking `6 settings` pushes settings;
  mouse second click marks (as before).

### G5 — profile edit, status, delete tree
`components/profiles/edit*.rs` and `components/status/*` on `package_list`; delete
`ui/tree.rs`, `Mark::Some`, `icons.checkbox_some`, `icons.fold_closed`; rename
`fold_open` → `caret`; help keys/glyph topics (§3.11); footer status hints without fold.
- Render: profile edit title `packages 3 of 9`, `■ ✓ git  via base` flat; missing ref row
  `? oldpkg … missing`; status rows unindented; help glyphs has no `▣`/`▾`.
- Interaction: `*` on profile edit marks own rows and leaves inherited; `w` writes plain
  names; unmarking a missing ref drops it on save; status `r` still rescans.
- `grep -rn 'ui::tree\|fold_closed\|checkbox_some' andre/src` is empty.

### G6 — settings: packages row; group editor removed
Settings flat-only (§3.10); delete every group type/overlay/hint and
`ProfileSet::{rename_group, remove_group}`; `IgnoreScope` keeps `Global` only (or is
removed if a bool suffices); `--onboard` writes `global: {}` and opens on `packages`.
Settings tests move to flat fixtures.
- Interaction: Enter on `packages` opens `packages folder` browse; choosing `dotfiles`
  then `w` writes `packages:` and leaves no `groups:` key; Esc dirty → leave dialog;
  `+` and `x` do nothing on settings.
- Render: section `config` lists `packages` and `global ignore`; no `groups`; detail
  `links to  ~`.
- Smoke/config_persistence: `--onboard -c <tmp>/andre.yml` creates a file that loads
  with the `NoPackagesFolder` warning only.

### G7 — wizard without the group step
`components/wizard/*`: `STEPS = ["name","source","ignore","profiles","review"]`,
`Step::Group` and its state gone; listing = `package_entries` names; picker opens at
`target_dir`; `package_ref` = name; review/done text by name (§3.6). Flat-only: no
packages folder → step 1 detail headline in `err` `set a packages folder in settings`
and Enter disabled. Help new-package topic. Step-bar tests renamed.
- Render: bar `[1 name] › 2 source …`; review title `review · new package nvim`; move
  lines `→ nvim/.config/nvim/`.
- tui_e2e: empty package and live-files package end-to-end on a flat temp HOME; profile
  file gains `nvim` (not `home/nvim`); `.gitignore` written.
- Mouse: clicking `✓ name` on step 3 returns to step 1.

### G8 — unstow without target select
Delete `components/target_select.rs` and its footer/crumb entries; the menu pushes
`UnstowBrowseComponent::at(ctx.target_dir())`; browse loses `group_name` and resolves
the owning package from the packages folder (§2.2); detail and review text (§3.9).
- Render: crumb `andre  main › unstow`; list title `~`; detail `package   shell`,
  `points to src/dotfiles/shell/.bashrc`; review `Restow shell`.
- Interaction: menu `4` opens browse directly; Esc returns to the menu; mark two links,
  Enter, `y` removes both (temp HOME); `←` at the target stays.
- todo.txt: drop `target select` from the detail-wheel item; drop the settings group
  dialog from the mouse-rows item.

### G9a — fixtures: core, e2e, demo, docs files
All `andre-core` test YAML (unit + `tests/integration.rs`, `tests/helpers`), every
`tests/e2e/*/andre.yml` (`packages: packages`, `target: target` — 06 keeps
`target: target/bin`), `03-multi-group-config` replaced by `03-two-profiles` (two
profiles, `--yolo --profile` links only one), `15-flat-basic` folded into 01, `demo/`
(`packages: packages`, `target: target`; profile refs `nvim`, `shell`, `git`; leave the
untracked `demo/profiles/test.yml` alone), `tests/manual/andre.yml` + README.
- Test: a unit test loads `demo/andre.yml` and asserts `target_dir` is inside `demo/`
  (hermetic demo guard).
- `grep -rln 'groups:' andre-core tests demo andre.yml.example` lists only the G1 "both
  keys" test.

### G9b — fixtures: TUI tests
`andre/tests/{helpers,render,interaction,mouse,tui_e2e,config_*,smoke,status_cache}.rs`
and every `#[cfg(test)]` fixture in `andre/src` move to flat YAML;
`make_flat_test_app` becomes `make_test_app`; assertions `home/shell` → `shell` etc.
No production code changes except test-only helpers.
- `grep -rn 'groups:' andre/` is empty; `grep -rn '"home/\|config/kitty\|work/aws' andre/tests`
  is empty.

### G10 — remove groups
Delete `Config.groups`, `Group`, every `*group*` method, `ConfigWarning::NoGroups`,
group branch in `package_entries`, `split_package_ref`, `LinkOp.name`,
`NewPackage.package_ref`, the interim both-keys check; add the §1.3 `groups:` error and
name-only refs. Docs sweep: README (features, config example, menu table), REFERENCE
(navigation flow, menu, shortcuts, e2e fixture list, fresh machine with
`packages: ../../..` example), AGENTS.md (Path Resolution → `packages_dir`/`target_dir`/
`package(name)`; Popup pattern example `GroupDialog` → `Membership`), ROADMAP if it names
groups. CHANGELOG entry finalised (§4.0).
- Tests: `groups:` file → `InvalidConfig` whose text contains `packages:` and `target:`;
  smoke: the binary prints it and exits 1; profile `[home/nvim]` → load issue with
  `plain package names`.
- Gate: `grep -rniw 'groups\?' andre/src andre-core/src` hits only
  `ui/status_bar.rs` (span group helpers), `ui/footer.rs` (left/right group comments) and
  `components/ignore/preview.rs` (`unclosed group` regex message).
- `make coverage-check` passes (floors unchanged).

## 5. Open questions (need the user)

| # | Question | Recommendation |
|---|----------|----------------|
| Q1 | Land with the short interim (groups keep loading until G10, 11 small batches) or as one big switch (fewer, much larger batches; no interim code)? | **Interim.** The interim code is ~one `match` in `package_entries` plus the both-keys check, deleted in G10; batches stay F4/F6-sized and green. |
| Q2 | Add `*` (mark / unmark all) to the packages browser and profile edit? | **Yes.** Group headers were the only "everything" control in the TUI; without it running all packages needs a profile or `--yolo`. |
| Q3 | Folders in the packages folder that are not packages (`docs/`, `scripts/` in a repo root): skip only dot-folders, or add an `exclude:` list? | **Dot-folders only**; docs say to keep packages in their own folder (`packages: ~/dotfiles/packages`) when the repo holds other folders. Add `exclude:` later if asked. |
| Q4 | Show `target` in settings? | **No.** It stays an advanced key for tests and demos; settings shows `links to` as a fact. A row invites the per-target setups we just removed. |

**User answers (final):** Q1 interim — yes. Q2 `*` mark/unmark all — yes. Q3 dot-folders
only: every other subfolder of the packages folder is a package ("a package is a package";
may be revisited later, keep the rule in `discover_packages` only). Q4 no target row —
settings shows `links to` as a read-only fact.
