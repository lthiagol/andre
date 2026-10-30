#!/usr/bin/env bash
set -euo pipefail

# Release helper for the andre branch model: `wip` -> `stable`.
#
#   scripts/release.sh <version>            release from `wip` (tag + push)
#   scripts/release.sh --promote <version>  fast-forward `stable` onto the
#                                           released `wip` commit
#
# There is no `main` branch in this repository. Work lands on `wip`; the
# promotion into `stable` is a separate, explicit step that runs only after
# CI is green on the promoted commit.

RELEASE_BRANCH="wip"
STABLE_BRANCH="stable"

usage() {
    cat <<EOF
Usage:
  $0 <version>              Release from '$RELEASE_BRANCH' (default: 1.0.0-rc1 style, e.g. 1.0.0-rc1)
  $0 --promote <version>    Fast-forward '$STABLE_BRANCH' onto the released commit

Notes:
  - Version bumps both andre/Cargo.toml and andre-core/Cargo.toml.
  - 'make coverage-check' needs the llvm-tools-preview rustup component:
        rustup component add llvm-tools-preview
    Without it cargo-llvm-cov fails with "failed to find llvm-tools-preview".
EOF
}

MODE="release"
if [ "${1:-}" = "--promote" ]; then
    MODE="promote"
    shift
fi

if [ "$#" -lt 1 ]; then
    usage
    echo "Error: missing version argument" >&2
    exit 1
fi

VERSION="$1"
TAG="v${VERSION}"

echo "=== Releasing andre ${TAG} (${MODE}) ==="

if [ -n "$(git status --porcelain)" ]; then
    echo "Error: Working tree must be clean"
    exit 1
fi

if [ "$MODE" = "promote" ]; then
    # Promote the already-released commit into `stable`.
    BRANCH=$(git rev-parse --abbrev-ref HEAD)
    if [ "$BRANCH" != "$RELEASE_BRANCH" ]; then
        echo "Error: Promotion must run from $RELEASE_BRANCH (currently on $BRANCH)"
        exit 1
    fi

    if ! git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
        echo "Error: Tag ${TAG} does not exist — release it first"
        exit 1
    fi

    RELEASE_COMMIT=$(git rev-list -n 1 "${TAG}")
    echo "Promoting ${TAG} (${RELEASE_COMMIT}) into ${STABLE_BRANCH}..."

    git checkout "${STABLE_BRANCH}"
    git merge --ff-only "${RELEASE_COMMIT}"
    git push origin "${STABLE_BRANCH}"

    echo ""
    echo "=== ${STABLE_BRANCH} promoted to ${TAG} ==="
    echo "CI handles: binaries, GitHub Release, Homebrew tap update."
    echo "Monitor: https://github.com/lthiagol/andre/actions"
    exit 0
fi

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "$RELEASE_BRANCH" ]; then
    echo "Error: Must be on $RELEASE_BRANCH branch (currently on $BRANCH)"
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
git push origin "${RELEASE_BRANCH}"
git push origin "${TAG}"

echo ""
echo "=== Release ${TAG} pushed from ${RELEASE_BRANCH} ==="
echo "CI handles: binaries, GitHub Release, Homebrew tap update."
echo "Monitor: https://github.com/lthiagol/andre/actions"
echo ""
echo "Next step (separate, explicit): promote ${RELEASE_BRANCH} into ${STABLE_BRANCH}"
echo "  scripts/release.sh --promote ${VERSION}"
echo "Coverage gate: 'make coverage-check' needs 'rustup component add llvm-tools-preview'"
echo "Done!     brew install lthiagol/tap/andre"
