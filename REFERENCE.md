<p align='center'>
  <img src='banner.svg' alt='andre logo' width='500px'>
</p>

# andre — Detailed Reference

A TUI wrapper for [GNU Stow](https://www.gnu.org/software/stow/).

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

andre wraps Stow in an interactive TUI. Launch with:

```bash
andre
```

### Navigation Flow

1. **Main Menu** — Choose an action (stow, unstow, restow, status, etc.)
2. **Group Selection** — Pick which config groups to operate on
3. **Package Selection** — Pick individual packages within each group
4. **Confirmation** — Review the stow commands before execution
5. **Execution** — See live output with progress feedback
6. **Status** — View results after execution

Global shortcuts (`v`/`d`/`t`/`n`/`a`/`o`/`Ctrl+C`) work from any screen.

<a name="features"></a>
## ✨ Features

- **Multi-group config** — Define multiple stow targets (e.g., `~`, `~/.config`) each with its own source directory
- **Interactive package selection** — Browse packages per group, select/unselect with Space, smart pre-selection based on action
- **Live settings toggles** — Toggle dry-run, verbosity, adopt, no-folding on the fly from any screen
- **Config editor built-in** — Edit `andre.yml` from within the TUI: add/remove/edit groups, change global defaults
- **Headless mode (`--yolo`)** — Auto-stow all packages without interaction, for scripts and automation
- **Config file discovery** — Auto-finds `andre.yml` in standard locations (`$PWD`, `~/dotfiles/`, `~/.dotfiles/`, `$XDG_CONFIG_HOME`)
- **Real stow status display** — See which packages are stowed, unstowed, conflicted, or missing at a glance

<a name="installation--build-dependencies"></a>
## 🛠 Installation

### Quick download (macOS / Linux)

Grab the binary for your platform from the [latest release](https://github.com/lthiagol/andre/releases):

**macOS (Apple Silicon):**
```bash
curl -LO https://github.com/lthiagol/andre/releases/download/v0.2.0/andre-aarch64-macos
chmod +x andre-aarch64-macos
sudo mv andre-aarch64-macos /usr/local/bin/andre
```

**Linux (x86_64):**
```bash
curl -LO https://github.com/lthiagol/andre/releases/download/v0.2.0/andre-x86_64-linux
chmod +x andre-x86_64-linux
sudo mv andre-x86_64-linux /usr/local/bin/andre
```

Make sure [GNU Stow](https://www.gnu.org/software/stow/) is installed on your system.

### Homebrew (macOS — recommended)

```bash
brew install lthiagol/tap/andre
```

Automatically installs GNU Stow and Rust. Works on Apple Silicon and Intel Macs (builds from source on Intel).

### Build from source

| Prerequisite | Purpose |
|:---|:---|
| **GNU Stow** | Core symlink engine (runtime) |
| **Rust** (1.77+) | Compiler (build) |

**Install Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Install Stow:**
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

The binary is installed to `/usr/local/bin/andre`.

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

andre auto-discovers the config in several locations (current directory, `~/dotfiles/`, `~/.dotfiles/`, `$XDG_CONFIG_HOME/andre/`). To use a specific path:

```bash
andre --config ~/dotfiles/andre.yml
```

Launch without arguments:

```bash
andre
```

### CLI Arguments

| Flag | Description |
|:---|:---|
| `--conf`, `--config <path>` | Path to YAML config (default: auto-discovery) |
| `--yolo` | Headless mode: auto-stow all packages without interaction |
| `--help` | Show usage information |

<a name="main-menu"></a>
## 🗺️ Main Menu

| Option | What it does |
|--------|-------------|
| **Stow packages** | Select groups and packages, then stow to targets |
| **Unstow packages** | Remove symlinks for selected packages |
| **Restow packages** | Remove and re-create symlinks (after package changes) |
| **Status** | View stow status across all groups |
| **Adopt files** | Move existing target files into a package |
| **Unstow individual links** | Remove specific symlinks one by one |
| **Settings** | Edit config, add/remove/edit groups, change global options |
| **Quit** | Exit andre |

<a name="full-configuration-reference"></a>
## ⚙️ Full Configuration Reference

### Config Discovery

The config file (`andre.yaml` or `andre.yml`) is searched in this order:

| Priority | Location |
|----------|----------|
| 1 | `--config` / `-c` flag (exact path) |
| 2 | Current working directory |
| 3 | `~/dotfiles/` |
| 4 | `~/.dotfiles/` |
| 5 | `$XDG_CONFIG_HOME/andre/` |

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

  # Patterns to ignore globally (supports glob syntax)
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
| `verbosity` | Stow verbosity level | 0 (quiet) to 5 |
| `dry_run` | Simulate without making changes | `true`, `false` |
| `no_folding` | Create individual symlinks (no directory folding) | `true`, `false` |
| `adopt` | Move existing target files into the package | `true`, `false` |
| `action` | Default stow action | `stow`, `restow`, `unstow` |

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

### Global Settings Toggles

| Key | Action |
|:---|:---|
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
| `q` or `Ctrl+C` | Quit / Cancel execution (first press = graceful, second = force) |

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
│       └── helpers.rs           # TestEnv helper for E2E
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
│       └── helpers.rs           # Shared test utilities
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
| **Default** | Dark base with amber accents |
| **Dracula** | Classic dark purple theme |
| **Catppuccin Mocha** | Dark, color-rich |
| **Catppuccin Latte** | Light theme |
| **Catppuccin Frappe** | Dark, muted pastels |
| **Catppuccin Macchiato** | Dark, medium contrast |
| **Nord** | Cool blue-gray palette |
| **Gruvbox** | Warm retro palette |

<a name="contributing"></a>
## 🤝 Contributing

### Development Workflow

1. Read `master-plan/STATUS.md` for current milestone status
2. Each milestone has its own document in `master-plan/milestones/` with tasks and verification criteria
3. Follow the conventions in `AGENTS.md`
4. Write tests for new functionality
5. Ensure clippy and fmt pass before submitting

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

CI runs on both Linux and macOS. Linux runs fmt + clippy + test + coverage report. macOS runs clippy + test only. Coverage thresholds are enforced: **andre-core ≥ 88%**, **andre ≥ 38%** (baseline minus 2%).

### Commit Style

Milestone commits use format: `milestone: complete [NN]-[name]`

<a name="license"></a>
## 📝 License

MIT
