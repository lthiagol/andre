<p align='center'>
  <img src='banner.svg' alt='andre logo' width='500px'>
</p>

# andre — Detailed Reference

**A.N.D.R.E.** — *Another Neat Dotfile Repository Engine*. A terminal UI for managing dotfiles.

## 📋 Table of Contents

- [What is GNU Stow?](#what-is-gnu-stow)
- [How to Use andre](#how-to-use-andre)
- [Features](#features)
- [Installation & Build Dependencies](#installation--build-dependencies)
- [Quick Start](#quick-start)
- [Main Menu](#main-menu)
- [Full Configuration Reference](#full-configuration-reference)
- [All Keyboard Shortcuts](#all-keyboard-shortcuts)
- [Testing](#testing)
- [Themes](#themes)
- [Icons](#icons)
- [Contributing](#contributing)
- [License](#license)

<a name="what-is-gnu-stow"></a>
## 🎯 What is GNU Stow?

**GNU Stow** is a symlink farm manager. It takes separate packages of software and/or data located in separate directories and makes them appear to be installed in the same place.

For dotfiles management, this is incredibly powerful: keep all your configuration files (e.g., `.bashrc`, `.zshrc`, `config.toml`) in a single version-controlled directory and use Stow to create symlinks in your home directory. This keeps `$HOME` clean while managing configs in one place.

### How Stow Works

Stow takes a **source** directory containing packages and a **target** directory where symlinks should appear. Given:

```
~/dotfiles/
  packages/
    shell/
      .bashrc
      .zshrc
```

Running `stow -d ~/dotfiles/packages -t ~ shell` creates:
```
~/.bashrc → ~/dotfiles/packages/shell/.bashrc
~/.zshrc  → ~/dotfiles/packages/shell/.zshrc
```

Key operations:
- **stow** — Create symlinks from source to target
- **restow** — Remove and re-create symlinks (after source changes)
- **unstow** — Remove symlinks without touching source files

<a name="how-to-use-andre"></a>
## 🔧 How to Use andre

andre puts an interactive terminal UI on the GNU Stow dotfiles model. Launch with:

```bash
andre
```

### Navigation Flow

1. **Main Menu** — Choose an action (profiles, packages, new package, unstow, verify status, etc.)
2. **Packages** — Mark groups and packages (`Space`) and press `Enter` to run them
3. **Confirmation** — Review the stow commands before execution
4. **Execution** — See live output with progress feedback
5. **Status** — View results after execution

On the packages screen, `Space` marks a package (on a group row, every package of the group),
`Enter` runs the marks, or the cursor's package or group, `←` on a package moves to its group's header, `←` on a header folds the group, `→` unfolds it, and `p` opens
the profile membership dialog for the cursor's package. In that dialog `Space` ticks a profile
and `Enter` saves; `Esc` cancels. `i` opens the ignore editor for the cursor's package: `Tab`
switches between `.gitignore` and `.stow-local-ignore`, `+` adds a line, `Enter` or `e` edits
the cursor's line, `x` deletes it and `w` writes the changed files. `Esc` with unsaved edits
asks whether to save.

Global shortcuts (`v`/`d`/`t`/`n`/`a`/`o`/`Ctrl+C`) work from any screen. The
top bar shows their state (a green mark when on, a red one when off; verbosity
has no mark and shows as a meter of 5 segments, `verbose ▰▰▱▱▱`) and the bar is
centred; the bottom bar shows the current screen's keys, the active theme
(`theme <name>`, changed with `t`) and `q quit`.

<a name="features"></a>
## ✨ Features

- **Multi-group config** — Define multiple stow targets (e.g., `~`, `~/.config`) each with its own source directory
- **Interactive package selection** — Browse packages per group, select/unselect with Space, smart pre-selection based on action
- **Live settings toggles** — Toggle dry-run, verbosity, adopt, no-folding on the fly from any screen
- **Config editor built-in** — Edit `andre.yml` from within the TUI: add/remove/edit groups, change global defaults
- **Headless mode (`--yolo`)** — Auto-stow all packages without interaction, for scripts and automation
- **Config folder and profiles** — One config folder (`$XDG_CONFIG_HOME/andre/`, or `--config-folder`) holds `andre.yml` and named package profiles
- **Real stow status display** — See which packages are stowed, unstowed, conflicted, or missing at a glance

<a name="installation--build-dependencies"></a>
## 🛠 Installation

Install methods are **Homebrew** and **from source** (no new prebuilt binaries
are published; the v1.0.0-rc1 release retains its historical assets). GNU Stow is **optional**: the default `engine: native` needs no
external binary. Install [GNU Stow](https://www.gnu.org/software/stow/) only if
you set `global.engine: stow` (or need `adopt` / `dotfiles`).

### Homebrew (macOS / Linux — recommended)

```bash
brew install lthiagol/tap/andre
```

Installs Rust. Builds from source on every platform: Apple Silicon and Intel
Macs, and Linux via [Homebrew on Linux](https://docs.brew.sh/Homebrew-on-Linux).

### Build from source

| Prerequisite | Purpose |
|:---|:---|
| **Rust** (1.77+) | Compiler (build) |
| **GNU Stow** | *Optional* — only needed when `global.engine: stow` is set |

**Install Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Install Stow (only if you use `global.engine: stow`):**
```bash
# macOS
brew install stow

# Linux (Debian/Ubuntu)
sudo apt install stow
```

**Build & install:**
```bash
git clone https://github.com/lthiagol/andre
cd andre
make install
```

Or via cargo:

```bash
cargo install --git https://github.com/lthiagol/andre
```

`make install` puts the binary at `/usr/local/bin/andre`; `cargo install` puts it in `~/.cargo/bin` instead.

<a name="quick-start"></a>
## 🚀 Quick Start

Create an `andre.yml` in your dotfiles directory:

```yaml
# Minimal working example
global:
  theme: default

  ignore:
    - .git
    - .DS_Store

  stow:
    verbosity: 0
    dry_run: false
    no_folding: false
    adopt: false
    action: stow

groups:
  dotfiles:
    source: packages/dotfiles
    target: ~
```

andre finds the config in `$XDG_CONFIG_HOME/andre/` (default `~/.config/andre/`). To use another file or folder:

```bash
andre --config ~/dotfiles/andre.yml
andre --config-folder ~/src/dots
```

Launch without arguments:

```bash
andre
```

### CLI Arguments

| Flag | Description |
|:---|:---|
| `--conf`, `--config <path>` | Path to YAML config file (default: auto-discovery) |
| `--config-folder <dir>` | Config folder holding `andre.yml` and `profiles/` (default: auto-discovery). Cannot be combined with `--config` |
| `--onboard` | Create an empty config at `--config` or `--config-folder` if none exists, then open the setup screen |
| `--yolo` | Headless mode: auto-stow all packages without interaction |
| `--help` | Show usage information |

<a name="main-menu"></a>
## 🗺️ Main Menu

The main menu is a list on the left and a detail panel on the right. The detail
panel describes the selected action and shows its facts: group and package
counts, the stowed count (`–` until every package's status has been checked),
and the target directories. Keys: `↑`/`↓` (or `j`/`k`) move, `1`–`8` jump to and
open an item, `Enter` opens the selected item, `q` quits. The full banner shows
on the main menu only; every other screen has a one-row `andre` title with the
breadcrumb. The cursor row on every list is filled with the theme's selection
colour and marked with a `▌` bar. Panel titles are lowercase and do not repeat
the breadcrumb. Content is capped at 120 columns and centred on wider terminals.

| Option | Icon (nerd / unicode) | What it does |
|--------|------|-------------|
| **profiles** | profiles / `◈` | Named sets of packages; run the current action on one (see Profiles) |
| **packages** | link / `⇢` | Select groups and packages, then run the current action (stow, restow or unstow) |
| **new package** | new package / `+` | Create a package from files that already sit in a target |
| **unstow** | unlink / `≠` | Remove symlinks interactively, one link at a time |
| **verify status** | check / `✓` | View stow status across all groups |
| **settings** | gear / `≡` | Edit config, add/remove/edit groups, change global options |
| **help** | question / `?` | Keyboard reference |
| **quit** | power / `×` | Exit andre |

<a name="full-configuration-reference"></a>
## ⚙️ Full Configuration Reference

### Config Discovery

andre loads one config file (`andre.yaml` or `andre.yml`), chosen in this order:

| Priority | Source | Searched |
|----------|--------|----------|
| 1 | `--config` / `-c <file>` | that file only |
| 2 | `--config-folder <dir>` | `<dir>/andre.yaml`, then `<dir>/andre.yml` |
| 3 | default | `$XDG_CONFIG_HOME/andre/` (`~/.config/andre/`); on macOS also `~/Library/Application Support/andre/` |

- A flag that is given is the only place searched. If it has no config, andre
  exits with an error and does not fall back to the next step.
- `--config` and `--config-folder` together are a usage error (exit 2).
- The current directory, `~/dotfiles/` and `~/.dotfiles/` are **not** searched
  (see CHANGELOG, *Changed*). If one of them holds a config, the not-found error names
  it and suggests `--config-folder <that dir>`.
- Profiles are read from `profiles/` next to the `andre.yml` that was loaded.

### Profiles

A profile is a named set of packages. Each one is a file in the `profiles/`
folder beside `andre.yml`:

```
<config-folder>/
├── andre.yml
└── profiles/
    ├── personal.yml
    └── agent-work.yml
```

```yaml
# profiles/agent-work.yml
description: Work servers that run agents   # optional
includes: [agent-base]                      # optional: other profile names
packages:                                   # optional: "group/package" refs
  - home/git
  - home/shell
  - config/nvim
```

- **Name**: the file name without `.yml`. Lowercase letters, digits, `-` and `_`; must start
  with a lowercase letter or digit. A `.yaml` file or a bad name is skipped with a
  `profile warning` at startup; other files in the folder are ignored.
- **Unknown keys** fail the whole file (it is skipped with a warning), as in `andre.yml`.
- **Includes** are resolved recursively. The result is the profile's own packages,
  then each include's packages in the order listed, with duplicates removed. Each
  entry records where it came from (own, or via which include).
- **Cycles** (`a` includes `b`, `b` includes `a`) and **unknown includes** are errors
  for that profile. The message names the cycle path or the missing profile.
- **Missing packages**: a ref to a group or package that does not exist is kept and
  shown as `missing`, with a warning at startup. It is not fatal.
- Renaming a package or group through andre rewrites the refs in every profile;
  deleting a package removes its refs. Deleting a profile removes the includes that
  name it.
- Each profile file is written whole, through a temp file in the same folder. If a
  write fails, the files already changed by that operation are restored, and the
  profiles in memory are left as they were.

### Ignore Files

A package can hold two ignore files at its root.

**`.stow-local-ignore`** lists files that are never linked. It follows GNU Stow's
rules (`manual: Ignore Lists`):

- One Perl-style regex per line. Blank lines and `#` comments are skipped. Text after
  a whitespace-preceded `#` is a comment, and `\#` is a literal `#`.
- A pattern containing `/` matches the path below the package root, at any depth:
  `bar/keep` matches `foo/bar/keep`. Start it with `^/` to anchor it to the root:
  `^/README\.md` matches only the top-level file.
- A pattern without `/` must match the whole file or folder name: `.*\.swp` matches
  `draft.swp`, and `cache` matches any folder called `cache`.
- A folder that matches is not entered, so nothing below it is linked.
- If the file exists, it **replaces** the built-in list for that package (and
  `~/.stow-global-ignore`). Without it, the built-in list applies. The built-in list
  is GNU Stow's own: `RCS`, `CVS`, `.svn`, `_darcs`, `.hg`, `.git`, `.gitignore`,
  `.gitmodules`, `.cvsignore`, `*,v`, `*~`, `#*#`, and the top-level `README*`,
  `LICENSE*`, `COPYING*`.
- The file itself is never linked. A `.stow-local-ignore` in a sub-folder is linked
  like any other file.
- The native engine uses Rust regex syntax. Look-around and backreferences are
  rejected with an error that names the line; use `global.engine: stow` for those.

**`.gitignore`** keeps files out of git. andre never reads it to decide what is
linked. The secrets template andre can write when a package is created covers `.env`,
`.env.*`, `*.env`, `*.key`, `*.pem`, `*history*`, `*.swp`, `secrets/`, `.cache/`,
`__pycache__/`, `.DS_Store`. Such files are still linked unless `.stow-local-ignore`
excludes them. The built-in list excludes `.gitignore` itself; a replacement list
must keep `\.gitignore` for the same effect (the template does).

### Path Resolution

All source and target paths are resolved **relative to the config file's directory**.

- **Absolute**: `/home/user/dotfiles` — used as-is
- **User-relative**: `~/dotfiles` or `$HOME/dotfiles` — resolved to home directory
- **Config-relative**: `dotfiles/home` — resolved relative to config file's directory
- **Environment**: `$MY_DOTFILES/home` — environment variable substitution

### Complete Template

```yaml
global:
  theme: catppuccin-mocha

  # Icon set: 'nerd' (default, needs a Nerd Font) or 'unicode'
  icons: nerd

  # Link engine: 'native' (default, no GNU stow binary required) or 'stow'
  # (shells out to GNU stow; needed for adopt, dotfiles, and regex `ignore` patterns;
  # per-package .stow-local-ignore regexes work with both engines, see Ignore Files)
  engine: native

  # Patterns to ignore globally: substring match with engine: native, regex with
  # engine: stow
  ignore:
    - .git
    - .DS_Store

  # Default stow options
  stow:
    verbosity: 1
    dry_run: false
    no_folding: false
    adopt: false
    action: stow

groups:
  home:
    source: dotfiles/home
    target: ~
    # Optional: per-group ignore patterns (merged with global.ignore)
    # ignore:
    #   - '*.log'
  config:
    source: dotfiles/config
    target: ~/.config
```

### Configuration Flags

| Flag | Purpose | Values |
|:---|:---|:---|
| `engine` | Link engine used to create/remove symlinks | `native` (default — built-in engine, no GNU stow binary needed), `stow` (shells out to GNU stow) |
| `verbosity` | Stow verbosity level | 0 (quiet) to 5 |
| `dry_run` | Simulate without making changes | `true`, `false` |
| `no_folding` | Create individual symlinks (no directory folding) | `true`, `false` |
| `adopt` | Move existing target files into the package | `true`, `false` |
| `action` | Default stow action | `stow`, `restow`, `unstow` |
| `icons` | Glyph set for toggles and menus (Settings → `icons`) | `nerd` (default, needs a Nerd Font), `unicode` (common Unicode symbols, no Nerd Font needed) |

<a name="all-keyboard-shortcuts"></a>
## ⌨️ All Keyboard Shortcuts

### Navigation

| Key | Action |
|:---|:---|
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `Enter` | Confirm / Select |
| `Space` | Toggle selection |
| `Escape` | Back / Cancel |

### Profiles

Main menu item `profiles` lists the profiles on the left (name and resolved package
count) and the selected profile on the right: description, includes, its packages
(`via <profile>` marks the ones that come through an include) and their status.

| Key | Action |
|:---|:---|
| `Enter` | Run the current action on the whole profile (review screen) |
| `+` | New profile (dialog; the name must be `a-z 0-9 _ -`, starting with a letter or digit) |
| `r` | Rename the profile; includes that name it are rewritten |
| `x` | Delete the profile after a confirm. Profiles that include it lose that include (the dialog names them). No files are unlinked |
| `e` | Edit the profile. The left pane marks packages (`Space`; `Enter` does nothing there; `←` `→` fold a group). `Tab` moves to the form: the description (`Enter` edits it, `Enter` or `Esc` ends the edit) and the includes (`Space` checks one). Packages that come through an include are marked `via <profile>` and cannot be toggled. `w` saves. `Esc` with unsaved edits asks: `y` save, `n` discard, `Esc` stay. An include that would make a cycle is refused with a toast |
| `c` | Status of the profile's packages (the status screen, listing only them) |
| `Esc` | Back |

### Global Settings Toggles

| Key | Action |
|:---|:---|
| `s` | Cycle the action: stow, restow, unstow |
| `v` | Cycle verbosity (0-5) |
| `d` | Toggle dry-run |
| `n` | Toggle no-folding |
| `a` | Toggle adopt |
| `o` | Toggle dotfiles mode |
| `t` | Cycle theme |

### Dialogs

| Key | Action |
|:---|:---|
| `y` | Confirm (confirm/adopt/unstow screens) |
| `n` | Cancel (confirm/adopt/unstow screens) |

### Application

| Key | Action |
|:---|:---|
| `q` or `Ctrl+C` | Quit |

All global shortcuts (`v`/`d`/`n`/`a`/`t`/`o`/`Ctrl+C`) work from **any screen**.

<a name="testing"></a>
## 🧪 Testing

Tests are organized across two crates and five tiers.

### Quick Start

```bash
cargo test --all                        # all tests
cargo test -p andre-core             # core library only
cargo test -p andre                  # TUI binary only
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

### Test Structure

```
andre/
├── andre-core/               # Core library tests
│   ├── src/                     # Unit tests (in each module)
│   └── tests/
│       ├── e2e.rs               # E2E: --yolo binary with fixtures
│       ├── integration.rs       # Integration: real stow + temp dirs
│       └── helpers/mod.rs       # TestEnv helper for E2E
│
├── andre/                    # TUI binary tests
│   ├── src/                     # Unit tests (e.g., execute.rs)
│   └── tests/
│       ├── interaction.rs       # Keyboard dispatch through components
│       ├── render.rs            # Terminal rendering snapshots
│       ├── config_discovery.rs  # Config file search paths
│       ├── config_persistence.rs# Save/load/edit round-trips
│       ├── status_cache.rs      # Cache behavior after stow operations
│       ├── tui_e2e.rs           # Full TUI flow with tokio runtime
│       └── helpers/mod.rs       # Shared test utilities
│
└── tests/                       # Non-code test assets
    ├── e2e/                     # 14 fixture scenarios
    └── manual/                  # Manual testing playground
```

### Testing Tiers

#### Unit Tests — `andre-core/src/`
Test individual functions in isolation.

| Module | What's tested |
|--------|---------------|
| `config.rs` | Load, save, defaults, groups, validation |
| `stow.rs` | Argument building, status checks, symlink unlinking |
| `adopt.rs` | Adoption planning, execution, target listing |
| `state.rs` | Theme cycling, action strings, state defaults |
| `packages.rs` | Package discovery, filtering, preview |
| `path.rs` | Path resolution (absolute, relative, `~`, `$ENV`) |

#### Integration Tests — `andre-core/tests/integration.rs`
Tests core library functions with real `stow` binary and temp dirs.

#### Interaction Tests — `andre/tests/interaction.rs`
Simulates keyboard input through the component stack. No terminal required.

#### TUI E2E Tests — `andre/tests/tui_e2e.rs`
Full end-to-end flow with tokio runtime and real `stow` execution.

#### E2E Tests — `andre-core/tests/e2e.rs`
Runs `andre --yolo` binary against fixture scenarios.

### E2E Fixtures

Located at `tests/e2e/<NN>-<name>/`. Each contains `andre.yml`, optionally `packages/` and `target/` dirs.

| # | Scenario | What it Verifies |
|---|----------|------------------|
| 01 | basic-stow | Basic symlink creation |
| 02 | multi-package-one-group | Group-level batch stow |
| 03 | multi-group-config | Cross-group stow |
| 04 | nested-directories | Directory structure preservation |
| 05 | hidden-dotfiles | Dotfile handling |
| 06 | executables | Permission preservation |
| 07 | ignore-patterns | File exclusion |
| 08 | defer-existing-target | Conflict: defer behavior |
| 09 | override-existing-target | Conflict: override behavior |
| 10 | adopt-existing-files | File migration into package |
| 11 | no-folding | Individual symlinks |
| 12 | restow | Restow cycle |
| 13 | dotfiles | Dotfile prefix stripping |
| 14 | config-relative-paths | Path resolution from config dir |

### Manual Test Setup

A realistic playground with 7 packages and pre-existing target files is available at `tests/manual/`.

```bash
cargo build --release
cd tests/manual && ../../target/release/andre
```

See `tests/manual/README.md` for full details.

<a name="themes"></a>
## 🎨 Themes

Cycle through themes with `t`:

| Theme | Description |
|:---|:---|
| **Default** | Dark base with amber accents. Paints no background, so it follows the terminal; it is built for dark terminals (use Catppuccin Latte on a light one) |
| **Dracula** | Classic dark purple theme |
| **Catppuccin Mocha** | Dark, color-rich |
| **Catppuccin Latte** | Light theme |
| **Catppuccin Frappe** | Dark, muted pastels |
| **Catppuccin Macchiato** | Dark, medium contrast |
| **Nord** | Cool blue-gray palette |
| **Gruvbox** | Warm retro palette |

<a name="icons"></a>
## 🔣 Icons

Set `global.icons` to `nerd` (the default) or `unicode`, or change it on the
Settings screen: the `icons` row cycles with Enter, and the change applies once
you save the config.

| Set | Toggles on / off | When to use |
|:---|:---|:---|
| **nerd** | Nerd Font glyphs | Your terminal font is a [Nerd Font](https://www.nerdfonts.com/) |
| **unicode** | `✓` / `✗` | Any other font. Common symbols, one column wide each |

The set also draws the checkboxes on the group, package and file lists (`■` / `□`
in unicode) and the package status glyphs (`✓` stowed, `→` unstowed, `◐` partial,
`✗` conflict, `?` missing, `∅` empty in unicode; `!` invalid in both sets).

An unknown value in the config triggers a warning and falls back to `nerd`.

<a name="contributing"></a>
## 🤝 Contributing

### Development Workflow

1. Check `todo.txt` for the current open work items
2. Follow the conventions in `AGENTS.md`
3. Write tests for new functionality
4. Ensure clippy and fmt pass before submitting
5. Add a `CHANGELOG.md` entry under `Unreleased` and remove the finished item from `todo.txt`

### Building

```bash
make build        # release build
make test         # lint + all tests
make coverage     # code coverage report
```

### Testing

```bash
cargo test --all
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
make coverage-check    # coverage thresholds (requires cargo-llvm-cov)
```

CI runs on Linux (`make ci`). Coverage thresholds are enforced locally via
`make coverage-check`: **andre-core ≥ 90%**, **andre ≥ 38%**.

### Commit Style

Conventional-ish prefixes: `feat:`, `fix:`, `test:`, `docs:`, `chore:`, `refactor:`.

<a name="license"></a>
## 📝 License

MIT
