# Manual Test Setup

This directory contains a manual test setup for andre with a single `home` group, 7 stow packages, and pre-existing files for adoption testing.

## Structure

```
tests/manual/
├── andre.yml              # Single group: home (packages → target)
├── target/               # Simulates $HOME
│   ├── .bashrc           # Pre-existing (for adoption tests)
│   └── .ssh/
│       └── config        # Pre-existing (for adoption tests)
├── packages/
│   ├── shell/            # .bashrc, .zshrc, .aliasrc, .config/shell/...
│   ├── emacs/            # .emacs.d/, .doom.d/, .doomrc, .zshenv
│   ├── git/              # .gitconfig, .gitignore_global, .config/git/...
│   ├── nvim/             # .config/nvim/init.lua
│   ├── tmux/             # .tmux.conf
│   ├── ssh/              # .ssh/config (conflicts with target/.ssh/config)
│   └── vim/              # .vimrc
├── view-and-reset.sh     # Helper: inspect or clear target/
└── README.md
```

## Packages

| Package | Files | Description |
|---------|-------|-------------|
| shell | 9 | Shell configs: bashrc, zshrc, aliasrc, multiple .config dirs |
| emacs | 10 | Emacs configs: init.el, doom.d, doomrc, zshenv |
| git | 8 | Git configs: gitconfig, gitignore, hooks, attributes |
| nvim | 1 | Neovim init.lua |
| tmux | 1 | Tmux config |
| ssh | 1 | SSH config (conflicts with pre-existing in target — adoption test) |
| vim | 1 | Vim config |

## Running Tests

```bash
# Build
cargo build --release

# Run TUI from this directory
cd tests/manual && ../../target/release/andre

# Or with explicit config path
../../target/release/andre --config andre.yml
```

## Testing Scenarios

### Basic stow
1. Run andre, select `home` group, select packages, confirm, execute
2. Run `./view-and-reset.sh` to see symlinks created

### Adoption
1. `target/.bashrc` and `target/.ssh/config` exist as real files
2. Select `shell` or `ssh` packages — they will conflict
3. Use the "Adopt files into a package" menu to move them into packages

### Unstow
4. After stowing, use "Unstow individual links" to remove symlinks one by one

### Reset
```bash
./view-and-reset.sh --reset
```

## Helper Script

`view-and-reset.sh` uses `eza` (preferred), falls back to `tree` or `find`.

```bash
./view-and-reset.sh          # Show target contents
./view-and-reset.sh --reset  # Clear target/
```
