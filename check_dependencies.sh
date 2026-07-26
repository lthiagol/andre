#!/usr/bin/env bash

# Colors for output
if [ -t 1 ]; then
    GREEN='\033[0;32m'
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    GREEN=''
    RED=''
    YELLOW=''
    BLUE=''
    CYAN=''
    BOLD=''
    RESET=''
fi

echo -e "${BOLD}Checking dependencies for andre...${RESET}\n"

# Helper to compare semver versions (major.minor.patch)
# Returns 0 (true) if $1 >= $2, 1 (false) otherwise
version_ge() {
    local val1=$1
    local val2=$2
    
    # Clean version strings: keep only digits and dots
    val1=$(echo "$val1" | sed -E 's/[^0-9.]//g')
    val2=$(echo "$val2" | sed -E 's/[^0-9.]//g')

    # Split into major, minor, patch
    local v1_1 v1_2 v1_3 v2_1 v2_2 v2_3
    v1_1=$(echo "$val1" | cut -d. -f1)
    v1_2=$(echo "$val1" | cut -d. -f2)
    v1_3=$(echo "$val1" | cut -d. -f3)

    v2_1=$(echo "$val2" | cut -d. -f1)
    v2_2=$(echo "$val2" | cut -d. -f2)
    v2_3=$(echo "$val2" | cut -d. -f3)

    # Default empty values to 0
    v1_1=${v1_1:-0}
    v1_2=${v1_2:-0}
    v1_3=${v1_3:-0}

    v2_1=${v2_1:-0}
    v2_2=${v2_2:-0}
    v2_3=${v2_3:-0}

    if [ "$v1_1" -gt "$v2_1" ]; then return 0; fi
    if [ "$v1_1" -lt "$v2_1" ]; then return 1; fi

    if [ "$v1_2" -gt "$v2_2" ]; then return 0; fi
    if [ "$v1_2" -lt "$v2_2" ]; then return 1; fi

    if [ "$v1_3" -ge "$v2_3" ]; then return 0; fi
    return 1
}

# Track overall status
BUILD_MET=true
RUN_MET=true

echo -e "${BLUE}${BOLD}[Build Dependencies]${RESET}"

# 1. Check rustc (min 1.77)
RUSTC_VER=""
RUSTC_VIA_RUSTUP=false

if command -v rustc >/dev/null 2>&1; then
    RUSTC_VER=$(rustc --version 2>/dev/null | awk '{print $2}')
elif command -v rustup >/dev/null 2>&1; then
    # Try running via rustup
    if rustup run stable rustc --version >/dev/null 2>&1; then
        RUSTC_VER=$(rustup run stable rustc --version 2>/dev/null | awk '{print $2}')
        RUSTC_VIA_RUSTUP=true
    fi
fi

if [ -n "$RUSTC_VER" ]; then
    if version_ge "$RUSTC_VER" "1.77.0"; then
        if [ "$RUSTC_VIA_RUSTUP" = true ]; then
            echo -e "  ${GREEN}✔${RESET} rustc (>= 1.77): MET ($RUSTC_VER) - ${YELLOW}Warning: Found only via rustup, not in PATH${RESET}"
        else
            echo -e "  ${GREEN}✔${RESET} rustc (>= 1.77): MET ($RUSTC_VER)"
        fi
    else
        echo -e "  ${RED}✘${RESET} rustc (>= 1.77): UNMET ($RUSTC_VER is too old, requires 1.77+)"
        BUILD_MET=false
    fi
else
    echo -e "  ${RED}✘${RESET} rustc (>= 1.77): UNMET (Not found)"
    BUILD_MET=false
fi

# 2. Check cargo
CARGO_VER=""
CARGO_VIA_RUSTUP=false

if command -v cargo >/dev/null 2>&1; then
    CARGO_VER=$(cargo --version 2>/dev/null | awk '{print $2}')
elif command -v rustup >/dev/null 2>&1; then
    if rustup run stable cargo --version >/dev/null 2>&1; then
        CARGO_VER=$(rustup run stable cargo --version 2>/dev/null | awk '{print $2}')
        CARGO_VIA_RUSTUP=true
    fi
fi

if [ -n "$CARGO_VER" ]; then
    if [ "$CARGO_VIA_RUSTUP" = true ]; then
        echo -e "  ${GREEN}✔${RESET} cargo: MET ($CARGO_VER) - ${YELLOW}Warning: Found only via rustup, not in PATH${RESET}"
    else
        echo -e "  ${GREEN}✔${RESET} cargo: MET ($CARGO_VER)"
    fi
else
    echo -e "  ${RED}✘${RESET} cargo: UNMET (Not found)"
    BUILD_MET=false
fi

# 3. Check cc linker
if command -v cc >/dev/null 2>&1; then
    CC_VER=$(cc --version 2>/dev/null | head -n 1)
    echo -e "  ${GREEN}✔${RESET} cc (C compiler/linker): MET ($CC_VER)"
else
    echo -e "  ${RED}✘${RESET} cc (C compiler/linker): UNMET (Not found, required to link binary)"
    BUILD_MET=false
fi

# 4. Check make (optional but recommended for Makefile)
if command -v make >/dev/null 2>&1; then
    MAKE_VER=$(make --version 2>/dev/null | head -n 1)
    echo -e "  ${GREEN}✔${RESET} make (build helper): MET ($MAKE_VER)"
else
    echo -e "  ${YELLOW}⚠${RESET} make (build helper): UNMET (Optional, used for Makefile targets)"
fi

echo ""
echo -e "${BLUE}${BOLD}[Runtime Dependencies]${RESET}"

# 1. Check GNU Stow (OPTIONAL runtime engine)
# Native is the default link engine and needs no external binary. GNU Stow is
# only required when the user sets global.engine: stow in andre.yml.
STOW_VER=""
if command -v stow >/dev/null 2>&1; then
    STOW_VER=$(stow --version 2>&1 | grep -oE "version [0-9.]+" | awk '{print $2}')
    if [ -z "$STOW_VER" ]; then
        STOW_VER="unknown version"
    fi
    echo -e "  ${GREEN}✔${RESET} stow (GNU Stow, optional engine): available ($STOW_VER)"
else
    echo -e "  ${YELLOW}⚠${RESET} stow (GNU Stow, optional engine): not found (only needed when global.engine: stow; default engine is native)"
fi

echo ""
echo -e "${BOLD}Summary:${RESET}"
if [ "$BUILD_MET" = true ]; then
    echo -e "  Build dependencies: ${GREEN}MET${RESET}"
else
    echo -e "  Build dependencies: ${RED}UNMET${RESET}"
fi

if [ "$RUN_MET" = true ]; then
    echo -e "  Runtime dependencies: ${GREEN}MET${RESET}"
else
    echo -e "  Runtime dependencies: ${RED}UNMET${RESET}"
fi

# Recommendations / Actions
if [ "$BUILD_MET" = false ] || [ "$RUN_MET" = false ] || [ "$RUSTC_VIA_RUSTUP" = true ] || [ "$CARGO_VIA_RUSTUP" = true ]; then
    echo ""
    echo -e "${YELLOW}${BOLD}Recommendations/Troubleshooting:${RESET}"
fi

if [ "$RUSTC_VIA_RUSTUP" = true ] || [ "$CARGO_VIA_RUSTUP" = true ]; then
    echo -e "  - ${BOLD}Rust PATH is misconfigured:${RESET}"
    echo -e "    It seems rustup is installed, but the tools are not in your shell's PATH."
    echo -e "    Run: ${CYAN}rustup default stable${RESET} or add ${CYAN}\$HOME/.cargo/bin${RESET} to your PATH."
fi

if [ "$BUILD_MET" = false ]; then
    echo -e "  - ${BOLD}To build andre:${RESET}"
    echo -e "    Please install Rust (v1.77 or newer) and Cargo."
    echo -e "    Run: ${CYAN}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${RESET}"
fi

if [ "$RUN_MET" = false ]; then
    # Reserved for future hard runtime deps. Stow is optional (global.engine: stow).
    echo -e "  - ${BOLD}To run andre:${RESET}"
    echo -e "    A required runtime dependency is missing (see Runtime Dependencies above)."
fi

# Optional engine guidance (never fails the doctor exit code).
if ! command -v stow >/dev/null 2>&1; then
    echo ""
    echo -e "${YELLOW}${BOLD}Optional engine:${RESET}"
    echo -e "  GNU Stow is not installed. The default ${CYAN}native${RESET} engine needs no stow binary."
    echo -e "  Only install stow if you set ${CYAN}global.engine: stow${RESET} in andre.yml:"
    echo -e "    On macOS: ${CYAN}brew install stow${RESET}"
    echo -e "    On Debian/Ubuntu: ${CYAN}sudo apt install stow${RESET}"
fi

# Exit status
if [ "$BUILD_MET" = true ] && [ "$RUN_MET" = true ]; then
    exit 0
else
    exit 1
fi
