#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"

# Reset target dir
rm -rf "$SCRIPT_DIR/target"
mkdir -p "$SCRIPT_DIR/target"

# === Legend scenarios ===

# 1. Installed (Stowed) — symlinks created via actual stow
#    Stow git package to create its symlinks
(cd "$SCRIPT_DIR" && stow -d packages -t target git 2>/dev/null) || true

# 2. Ready (Unstowed) — emacs is in source, no symlinks in target
#    (leave emacs alone — no action needed)

# 3. Partial — shell: create .bashrc symlink but not .zshrc
ln -s "$SCRIPT_DIR/packages/shell/.bashrc" "$SCRIPT_DIR/target/.bashrc"

# 4. Blocked (Conflict) — stow nvim first, then replace one symlink with real file
(cd "$SCRIPT_DIR" && stow -d packages -t target nvim 2>/dev/null) || true
rm -f "$SCRIPT_DIR/target/.config/nvim"
echo "real config file — blocks stow" > "$SCRIPT_DIR/target/.config/nvim"

# 5. Not found (Missing) — delete vim source package
#    (move backup outside packages so it doesn't show as a package)
cp -r "$SCRIPT_DIR/packages/vim" "$SCRIPT_DIR/.vim-backup"
rm -rf "$SCRIPT_DIR/packages/vim"

# 6. ssh stays as Ready (no symlinks yet)
# 7. tmux stays as Ready

echo "=== Smoke fixture ready ==="
echo ""
echo "Legend scenarios:"
echo "  ✓ Installed → git"
echo "  → Ready     → emacs, ssh, tmux"
echo "  ◐ Partial   → shell (only .bashrc linked)"
echo "  ⚠ Blocked   → nvim (real file at .config/nvim)"
echo "  ? Not found → vim (source deleted)"
echo ""
echo "Run: andre --config $SCRIPT_DIR/andre.yml"
echo ""
echo "After testing, restore with:"
echo "  mv $SCRIPT_DIR/.vim-backup $SCRIPT_DIR/packages/vim"
echo "  $SCRIPT_DIR/view-and-reset.sh --reset"
