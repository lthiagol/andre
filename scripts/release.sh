#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?Usage: $0 <version> (e.g., 0.2.0)}"
TAG="v${VERSION}"

echo "=== Releasing andre ${TAG} ==="

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
    echo "Error: Must be on main branch (currently on $BRANCH)"
    exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
    echo "Error: Working tree must be clean"
    exit 1
fi

STOW_TUI_VER=$(cargo metadata --format-version 1 --no-deps 2>/dev/null |
    python3 -c "import sys,json; d=json.load(sys.stdin); print([p['version'] for p in d['packages'] if p['name']=='andre'][0])")
CORE_VER=$(cargo metadata --format-version 1 --no-deps 2>/dev/null |
    python3 -c "import sys,json; d=json.load(sys.stdin); print([p['version'] for p in d['packages'] if p['name']=='andre-core'][0])")

if [ "$STOW_TUI_VER" != "$VERSION" ] || [ "$CORE_VER" != "$VERSION" ]; then
    echo "andre: $STOW_TUI_VER, andre-core: $CORE_VER, expected: $VERSION"
    echo "Updating versions..."

    if [[ "$(uname)" == "Darwin" ]]; then
        sed -i '' "s/^version = '.*'/version = '${VERSION}'/" andre/Cargo.toml
        sed -i '' "s/^version = '.*'/version = '${VERSION}'/" andre-core/Cargo.toml
    else
        sed -i "s/^version = '.*'/version = '${VERSION}'/" andre/Cargo.toml
        sed -i "s/^version = '.*'/version = '${VERSION}'/" andre-core/Cargo.toml
    fi

    cargo check --all --quiet 2>/dev/null || { echo "Version bump broke build"; exit 1; }

    git add andre/Cargo.toml andre-core/Cargo.toml
    git commit -m "chore: bump version to ${VERSION}"
    echo "Version bumped and committed"
else
    echo "Version ${VERSION} confirmed in both Cargo.toml files"
fi

echo "Creating tag ${TAG}..."
git tag -a "${TAG}" -m "Release ${TAG}"
git push origin main
git push origin "${TAG}"

echo ""
echo "=== Release ${TAG} pushed ==="
echo "CI handles: binaries, GitHub Release, Homebrew tap update."
echo "Monitor: https://github.com/lthiagol/andre/actions"
echo "Done!     brew install lthiagol/tap/andre"
