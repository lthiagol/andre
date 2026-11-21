# Changelog

All notable changes to andre are recorded here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

Work lands on the `wip` branch with an entry under **Unreleased**. Version
bumps and release dates are decided when `wip` promotes to `stable`.

## [Unreleased]

### Added

- `global.icons` config setting: `nerd` (default, Nerd Font glyphs) or
  `unicode` (common Unicode symbols, no Nerd Font needed). An unknown value warns
  and falls back to `nerd`. Settings gets an `icons` row (Enter cycles it,
  saved with the rest of the config). Documented in `REFERENCE.md` (new
  Icons section) and `andre.yml.example`. The glyph tables live in
  `andre/src/ui/icons.rs`.

### Changed

- Theme colour roles are renamed by purpose (no colour changes): `background`→`bg`,
  `secondary`→`text`, `muted`→`text_dim`, `primary`→`accent`, `highlight`→`value`,
  `indicator`→`key`, `success`→`ok`, `warning`→`warn`, `error`→`err`. `ThemeColors`
  gains `on_accent` (the action pill's text, the old background value in every theme)
  and `banner` (the five banner letters, the current built-in colours). The
  `selection` glyph fallback names follow the rename. Rendering is unchanged.
- Main menu is a list and a detail panel. The list on the left shows each action
  with an icon, its label and a jump digit (`1`–`7`) at the right edge. The
  selected row is filled edge to edge with a new `selection` theme colour and
  marked with a `▌` accent in the primary colour on the normal background; the
  `>>` marker is gone. The list panel is titled `actions`. The Settings
  breadcrumb reads `Settings` (was `Config`). The first item is
  `stow packages` and follows the current action. The labels are `adopt files`,
  `unstow`, `verify status`, `settings`, `help` and `quit`. The detail panel on
  the right is titled with the selected label and shows a description plus
  facts: groups and packages, the stowed count (`–` until every package's status
  is cached; the status cache is read, never refreshed, by the menu, and a
  stow/restow/unstow run, an adoption or an unlink clears it), target
  directories, the config path, engine and theme, or the key reminder. The facts
  are built in `MainMenuComponent::update` and rebuilt only when the selection,
  the action, the theme, the config or the status cache changes. Render only
  reads them. The menu's icons come from `ui::icons`, which follows
  `global.icons`. Tests: `components::menu` unit tests (facts, rebuild rule,
  label per action) and `andre/tests/render.rs` (selected-row fill, readable
  label contrast in all eight themes, detail facts after a cache change).
  The `selection` colour is picked per theme so the selected label reads at
  4.5:1 or better, and the selected icon and accent bar at 3:1 or better. The
  detail text is inset one column from the panel border.
- Layout: the 15/70/15 horizontal squeeze is gone. Every screen uses the full
  content width minus one column of margin on each side, capped at 120 columns
  and centred on wider terminals. Screens get the vertical space the banner
  used to take: the full seven-row banner shows on the main menu only, and every
  other screen shows a one-row `andre` title with the breadcrumb
  (`ui::banner::render_compact_banner`). The top bar's row is no longer fixed;
  the render tests find it from the drawn banner separator.
- Top bar redesigned as one flat, lowercase row: the current action as a pill
  on the left (`stow`, `restow`, `unstow`), then the verbosity level, then a
  muted `│` divider and a chip per boolean flag (`✗ dry-run`, `✗ no-fold`,
  `✗ adopt`, `✗ dotfiles`; nerd set uses the Font Awesome check and times).
  Verbosity is a level, so its chip has no mark: `verbose ▰▰▱▱▱` is a meter of
  `MAX_VERBOSITY` segments, `▰` filled in primary and bold up to the level and
  `▱` muted for the rest (glyphs in `Icons::meter_full` / `meter_empty`). Each
  flag chip's mark is green (success) when on and red
  (error) when off. Each chip's hotkey letter is drawn in the indicator colour
  inside its label, so the separate key letters are gone. On labels are bright,
  off labels muted. The whole group (pill and chips) is centred in the bar's
  width, with any odd column on the right. A `debug` chip shows in debug mode.
  Labels shrink to hotkey letters when the full bar does not fit (`v ▰▱▱▱▱`),
  and to `v<level>` (`v2`) when even that does not fit. The top bar no longer
  shows the theme. The bar is built by the pure function
  `ui::status_bar::top_bar_line`, which has unit tests.
- Bottom bar is one row: the current screen's key hints on the left, then
  `theme <name>` and `q quit` at the right edge (`t` in the indicator colour,
  the name in the highlight colour). The theme chip shrinks to `theme` when the
  full name does not fit beside the hints, and is dropped when not even that
  fits. `q quit` is always kept. The global-key line is gone (those keys now
  live in the top bar). The freed row goes back to the content area. Both bars
  keep a one-column margin on each side. The bar is built by the pure function
  `ui::footer::footer_line`, which has unit tests.
- `ui::footer::screen_hints` holds the per-screen hint table (same hints as
  before).
- Dependency upgrade: `ratatui` 0.26 → 0.29 and `crossterm` 0.27 → 0.28 (the
  version ratatui 0.29 builds on). No intended visual change; the existing
  render snapshot tests pass unchanged. API fallout:
  - `Frame::size()` (deprecated) → `Frame::area()` in `ui/mod.rs`.
  - `Rect::inner` takes `Margin` by value (was `&Margin`) in `ui/mod.rs`.
  - `Buffer::get(x, y)` (deprecated) → `buf[(x, y)]` in the `render` and
    `interaction` test helpers. Same panic on out-of-bounds, same `&Cell`.
  - `instability` pinned to 0.3.12 in `Cargo.lock` so the build keeps a single
    `syn` major version.
- Settings directory picker, unstow/adopt browse screens, and the stow status
  and package lists now keep their rows out of paint (refactor; no intended
  behaviour change except as noted):
  - `components/browser.rs`: one `Browser` (path, cursor, cached listing,
    `refresh`/`enter_dir`/`go_up`/`step`) backs the unstow and adopt browse
    screens and the settings directory dialogs. The settings dialogs list their
    directory when they open and after each move, so the popup paints the
    cached listing.
  - The adopt and unstow target pickers are one `TargetSelectComponent`
    (`TargetSelectComponent::adopt()` / `::unstow()`). The
    `UnstowTargetSelectComponent` and `AdoptTargetSelectComponent` types and
    their `target_select.rs` files are gone; screen ids are unchanged.
  - `UnstowBrowseComponent::new` / `AdoptFileSelectComponent::new` delegate to
    `with_path`. `UnstowBrowseComponent::selected_path` (never read) is removed.
  - `settings/dialogs.rs`: one handler per dialog variant that edits the dialog
    in place and returns a `Next` (stay / go to / close); text entry and group
    picking are shared helpers. Keys, rename and ignore results are unchanged.
  - `FileBrowserWidget` no longer has the `dirs_only` fallback that listed
    directories during paint; callers pass cached entries.
  - The package select caches its rows. They are built in `update()` and
    rebuilt only when the new `StatusCache::generation()` changes; Space
    updates the selected row in place. Packages added on disk while the screen
    is open appear on the next visit, not live.
  - The status screen builds its rows once, on entry, after the status
    prefetch; paint reads them and no longer changes component state. Packages
    added on disk while the screen is open appear on the next visit.
  - The review (`Review Commands`) and unlink screens build their rows when
    they are created: group paths and the link target are read there, and
    paint only draws the result. `ui::ConfirmRow` and `ui::UnstowLink` carry
    the precomputed text. The review screen keeps the global flag keys live,
    so it rebuilds its `stow` command preview in `update()` when a flag
    changes; the preview always shows the flags that `y` will run with.
  - `input::step_cursor` is the one cursor-wrap implementation. The list keys,
    the unstow and adopt browse screens, the settings pickers and the adopt
    skip-unselectable loop all use it. The settings dialogs share
    `list_cursor_key` and `home_browser`.
  - The package select scans each selected group once per rebuild (one
    `discover_packages` call, one prefetch, rows from the same scan).
  - New tests: the settings directory picker re-lists after Right and Left, and
    Enter picks from the directory entered; a status visit builds its rows from
    the fresh prefetch and keeps them for the rest of the visit; the unlink
    screen records its link target when it is created; the review list records
    its resolved previews and its stow preview follows a dry-run toggle made
    while the screen is open.
  - Coverage (`make coverage-check`, floors unchanged): andre-core 95.86%
    (2550/2660 lines), andre 69.21% (3713/5365 lines).

- CI runner images pinned to `ubuntu-24.04` in both workflows (what GitHub's
  `ubuntu-latest` currently maps to) for stability. The test gates stay
  Linux-only by decision: the TUI is exercised locally on macOS, so no macOS
  CI gate is added. `ubuntu-26.04` exists as a label and can be adopted
  deliberately when wanted.

- Binary release builds removed from CI. The `release-assets` job (which built
  the four `andre-<arch>-<os>` assets on `macos-latest`/`ubuntu-latest`/
  `ubuntu-24.04-arm` and uploaded them to the GitHub Release) is gone; a
  minimal `release` job now only creates the GitHub Release with generated
  notes and keeps the rc pre-release/"Latest" reconciliation. The supported
  install methods are Homebrew (the tap formula builds from the source
  tarball) and from source (`make install` / `cargo install`).
- README.md and REFERENCE.md installation sections rewritten accordingly: the
  "Quick download" binary sections were removed, Homebrew is documented for
  macOS/Linux, and `brew-install` in the Makefile now installs the plain
  `dist/andre` build. The `release-assets`, `build-intel`, `build-arm`, and
  `build-local-macos` Makefile targets were removed.
- Removed the dead `HOMEBREW_TAP_TOKEN` secret from the repo's GitHub Actions
  secrets (it held a classic PAT GitHub already rejected with `401`; the tap
  job authenticates with `ANDRE_TAP_SSH_KEY` + `ANDRE_TAP_PR_TOKEN`). The
  explanatory comment in `stable-ci.yml` was rewritten accordingly.
- `ExecutionUpdate::Progress` no longer carries an unread `_package` string,
  and `ExecutionState` has only the `Running` and `Completed` states that are
  actually reached. `spawn_execution` replaces `AsyncExecutor::spawn` and
  returns just the update receiver. `ExecutionUpdate::Progress` also drops its
  `total` field: the Execute screen fixes the total (commands + skipped) at
  construction, and the sent count was never read.
- In-flight execution cannot be cancelled, as before. `q` and `Ctrl+C` quit
  the app; the Execute screen has no cancel key. README.md and REFERENCE.md
  key tables now say "Quit" instead of "Quit / Cancel execution".
- `andre-core` test helpers moved from `tests/helpers.rs` to
  `tests/helpers/mod.rs`; the helper self-tests now run inside the
  `integration` binary. `run_awesome_stow`/`awesome_stow_binary` are renamed
  `run_andre`/`andre_binary`.
- `StowEngine::apply` passes `false` for dry-run to `execute_stow` explicitly
  (the dry-run case already returned early, so `-n` is never emitted).
- Clippy hygiene: redundant clones and closures removed, and
  `MainMenuComponent::activate` is now an associated fn. Fully-qualified
  method paths from `clippy --fix` (`std::vec::Vec::len`,
  `std::string::String::as_str`, `<[std::string::String]>::to_vec`,
  `std::string::ToString::to_string`) are shortened to their prelude forms. The engine
  dry-run test's `filter_map(std::result::Result::ok)` is now `.flatten()`,
  and the `config.rs` tests use `".git".to_string()`. No behavior change.
- `Config::groups` is a `BTreeMap`, so groups are listed and written to
  `andre.yml` in name order. Saves no longer reshuffle the group block, and
  every screen shares one order; the ad-hoc `sort()` calls in the group list
  widgets are gone.
- `Theme::ALL` and `StowAction::ALL` are the single list behind the Settings
  pickers and `Theme::next`. `Theme::from_str` accepts each `Theme::name()`
  case-insensitively, with `_` as an alias for `-`.
- `PendingSettings::to_config` builds the on-disk config from every pending
  setting, and `apply_to` commits the same config to core. `save_config` has
  one success path and one error path. Saves now always write `global.theme`
  (including `theme: default`), and a theme changed with the global `t` key
  before Settings was opened is persisted with the next save.
- Doc drift from the helper and executor moves fixed: AGENTS.md and
  REFERENCE.md point at `helpers/mod.rs` (not `helpers.rs`), and the
  dependency table names `execute::spawn_execution` instead of
  `AsyncExecutor`.
- The stow options (action, verbosity, dry-run, no-folding, adopt, dotfiles)
  travel as one `StowFlags` value from config through settings, `LinkOp`, the
  stow argument builder, `run_job`, `execute_stow`, and `execute_adoption`,
  replacing long runs of positional `bool`/`u8` parameters. `Config::stow_flags`
  and `Config::set_stow_flags` replace the six `effective_*` and `set_*` pairs.
- Stringly-typed values are enums: `StowAction` is used throughout (the
  confirm screen no longer keeps its own action string), and the new
  `EngineKind` (`Native` / `Stow`) replaces `"native"`/`"stow"` strings in
  `Config::effective_engine` and `engine_for`.
- Settings flag edits and the `v`/`d`/`n`/`a`/`o` shortcuts share one
  `StowFlags::toggle` path. `MAX_VERBOSITY` (5) replaces the hard-coded `% 6`
  cycle and `> 5` range check.
- Settings ignore and group-path edits go through `Config` methods
  (`set_group_ignore`, `add_global_ignore`, `remove_global_ignore`,
  `update_group_source`, `update_group_target`). Removing the last ignore of a
  group now drops the `ignore` key on save instead of writing `ignore: []`.
- Group path lookups (`resolve_group_source`/`_target`, `get_group_source`/
  `_target`) share one private resolver.
- `App::new`, `App::with_home`, and `App::with_debug` no longer take a `yolo`
  argument, and `AppState::yolo_mode` is removed (it was written but never
  read). `--yolo` is still handled in `main`.
- Confirm and execution share one `build_execution_results`, which returns
  `LinkOp`s directly instead of an anonymous tuple, and the four result arms
  collapse into one `ExecutionResult` construction.
- `Transition::name` returns `&'static str`, and `AppContext::log` takes a
  closure so debug messages are only formatted while debugging is on.
- One cursor style for every list. The new `ui::selection::row` helper draws the
  cursor row on the group, package, target, file browser, settings and settings
  popup lists: a `▌` accent in the primary colour on the normal background, and
  the row filled edge to edge with the theme's `selection` colour. Unselected rows
  get the same one-column gutter, so text lines up. The main menu uses the helper
  too. The `>>`, `-->` and `> ` markers are gone. The status and confirm screens
  have no cursor, so they keep their scrolling lists. The execution list keeps its
  progress highlight. On the cursor row, text takes the label colour (secondary)
  and the accent is primary, so every theme keeps the row readable. A checkbox or
  status glyph keeps its own colour there where that colour reads at 3:1 on the
  fill (WCAG 1.4.11); where it does not, the glyph falls back to the label colour.
  The glyph shape still tells the states apart. The fallbacks are: Dracula error
  and muted; Catppuccin Mocha muted; Catppuccin Latte success, warning and muted;
  Catppuccin Frappe error and muted; Catppuccin Macchiato muted; Nord error and
  muted; Gruvbox success, warning, error and muted. Tests: `ui::selection` unit
  tests, and `andre/tests/render.rs` checks the fill, the accent, the absence of
  the old markers and the glyph contrast on every list screen, in all eight themes.
- Checkboxes come from `ui::icons` and follow `global.icons`: `■` / `□` in
  unicode, the Font Awesome check-square and square glyphs in nerd. The package
  status glyphs (stowed, unstowed, partial, conflict, missing, empty) also come
  from `ui::icons`, so they follow the icon set too. Each status keeps its own
  glyph and its own colour. In unicode the conflict glyph is `✗` (was `⚠`,
  which falls outside the blocks the unicode set is limited to).
- The package select and the status screen share one status vocabulary in
  `ui::status_style`: the same colour, the same lowercase word (`stowed`,
  `unstowed`, `partial`, `conflict`, `missing`, `empty`, from `StowStatus::label`)
  and the same legend. The package select's `Installed`, `Ready`, `Blocked` and
  `Not found` are gone. Unstowed is primary on both screens (it was muted on the
  status screen). Test: `test_package_and_status_screens_share_glyph_colour_and_word`.
- Package rows read `<checkbox> <status glyph> <package>  <group>  <status word>
  <detail>`. The package is in the label colour and the group is muted. Each
  column is padded to its longest entry when the rows are built, so the groups,
  status words and details each start in one column. The package column is capped
  at 32 columns and the group column at 16; longer names are cut with `…`, so every
  status word stays on screen at 80x24. The detail follows a status word padded to
  the longest one. It is cut to the room left on the row with a trailing `…`, and
  dropped when fewer than two columns are left.
- Legends (package select, status, file browser) use the ` legend ` panel title
  like every other panel, and drop the `Legend:` header row. The package select
  legend had seven rows in eight lines, so `Empty` was cut off; it now lists all six
  statuses. The status legend is eight lines (was nine) and the file browser legend
  is five (was six).
- Panel titles are lowercase and no longer repeat the breadcrumb, which the
  compact banner already shows: `select groups`, `select packages`,
  `select target for adoption`, `select target for unstow`, `select files to adopt`,
  `unstow symlinks`, `stow status`, `settings`, `review commands`, `execution`,
  `confirm adoption`, `adoption plan preview`, `adoption result`, `package name`,
  `help` and `unlink`. Settings popup titles follow the same rule (`pick theme`,
  `edit: <group>`, `ignore patterns`). The settings title was `Config`; it is
  `settings`.
- Settings rows: the value column starts at the same column on every row, and
  the info lines (config path, resolve path, saved message) are not indented.

### Removed

- `StowStatus::icon` in `andre-core`. The glyphs now live in `ui::icons`, where
  the icon set can choose them.

- Dead configuration state: `AppState::effective_action` and
  `AppState::yolo_mode`. The per-option `Config::effective_*` and `set_*`
  accessors were replaced by `stow_flags`/`set_stow_flags` (see Changed).
- Dead cancel/abort machinery: `AsyncExecutor::abort`, `App::executor`, the
  executor payload on `Transition::Replace`, and the `ExecutionState::Pending`,
  `Cancelling`, and `ErrorPrompt` variants with their render arms.
- Unused public API: `yolo::run_all` and `YoloResult` (`--yolo` still reports
  through `main`; its exit code is unchanged), `build_stow_args_multi`,
  `ui::packages::build_flat_items`, `ui::adopt_browse`, `ui::unstow_browse`,
  `settings::rows::row_at_index`, `StatusCache::invalidate_group`,
  `App::handle_key`, `App::is_quitting`, `AppContext::show_toast_for`,
  `LogLevel::Info`, the `DryRun`/`NoFolding`/`Adopt`/`Dotfiles` picker fields,
  and `GroupEditField::Ignore`.
- Unused `andre-core` test helpers: `run_stow`, `assert_no_symlinks`,
  `assert_target_exists`, `count_symlinks`, `cleanup_symlinks`.

### Fixed

- Scrollbars never drew on any list screen (package select, group select, status,
  settings, confirm, execute, help). Each one reserved a single column that was
  narrower than the scrollbar's minimum, so the column stayed blank and no thumb
  showed. The bar now draws over the right border of its panel: the track is in the
  border colour and the thumb in the indicator colour, and it appears only when the
  rows overflow. The list keeps the full panel width, so its right border lines up
  with the legend's. Pre-existing; affects released versions. Tests:
  `andre/tests/render.rs` (thumb present when overflowing, absent when fitting,
  border column matches the legend).
- Package select and status sized their page from the whole panel, not the list
  beside the legend, so paging jumped too far and the thumb could hide when rows
  overflowed. Both now use `status_layout`, the same split paint uses. Tests:
  `andre/tests/render.rs` (status thumb at 80x24) and `prepare_sizes_the_viewport`
  unit tests in `components/status` and `components/stow/package_select`.
- On the Unstow and Adopt target screens, Enter could open a different group
  than the highlighted row when a config has two or more groups. The group
  list is now name-ordered everywhere, so Enter opens the group shown under
  the cursor.
- Renaming a group from Settings (Edit > Name) did not rename it. Typing
  rewrote the key the group was looked up by, so Enter found nothing; a name
  that matched another group re-added that group and dropped its ignores. A
  rename now keeps the group's source, target and per-group ignores. An empty
  or already-taken name keeps the prompt open, and Esc returns to the field
  menu under the original name.
- Theme changes made in Settings (the `t` key or the theme picker) were never
  written to `andre.yml`. They now persist on save.
- The Settings theme picker offered Catppuccin themes as `catppuccin mocha`
  (with a space). The config parser rejected that, so picking one was silently
  ignored. The picker now lists the same names the config uses, and each one
  round-trips.
- A failed config save no longer prints to stderr while the TUI owns the
  terminal; the error toast is the only report.
- Stow status scrolled past its last row when every row fit on screen, so Up
  needed several presses before the list moved. Scrolling now stops at the last
  row. Test: `components/status` `test_scroll_stops_at_the_last_row_when_rows_fit`.
- Counts in titles read `(1 items)` and `(1 packages)`. A new `ui::plural` helper
  picks the singular for one, so the titles read `(1 item)` and `(1 package)`.
  Affects the file browser title (unstow symlinks) and the stow status title.
  Tests: `ui` `test_plural_uses_singular_only_for_one`, `ui/status`
  `test_status_title_counts_packages_with_the_right_noun`, and
  `ui/widgets/file_browser` `test_entries_label_uses_the_right_noun_for_0_1_and_2`.

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
