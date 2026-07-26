#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"

show_tree() {
    if command -v eza &>/dev/null; then
        eza --tree "$SCRIPT_DIR/target/"
    elif command -v tree &>/dev/null; then
        tree "$SCRIPT_DIR/target/"
    else
        find "$SCRIPT_DIR/target" -not -type d | sort
    fi
}

show_stats() {
    local count
    count=$(find "$SCRIPT_DIR/target" -type l 2>/dev/null | wc -l | tr -d ' ')
    echo "Symlinks in target: $count"
    count=$(find "$SCRIPT_DIR/target" -type f -not -type l 2>/dev/null | wc -l | tr -d ' ')
    echo "Regular files in target: $count"
}

do_reset() {
    find "$SCRIPT_DIR/target" -mindepth 1 -delete
    mkdir -p "$SCRIPT_DIR/target/.ssh"
    echo '# pre-existing bashrc -- can be adopted by the shell package' > "$SCRIPT_DIR/target/.bashrc"
    echo '# pre-existing ssh config -- can be adopted by the ssh package' > "$SCRIPT_DIR/target/.ssh/config"
    echo "Target directory cleared and pre-existing files restored."
}

case "${1:-}" in
    --reset|-r)
        do_reset
        ;;
    --help|-h)
        echo "Usage: $0 [--reset|-r]"
        echo ""
        echo "  (no args)  Show target directory contents"
        echo "  --reset    Clear all contents from target/"
        ;;
    *)
        echo "=== Target directory contents ==="
        show_tree
        echo ""
        show_stats
        echo ""
        echo "Run '$0 --reset' to clear target/"
        ;;
esac
