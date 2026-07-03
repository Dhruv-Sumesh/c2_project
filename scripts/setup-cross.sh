#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[✗]${NC} $*"; }

if ! command -v rustup &>/dev/null; then
    error "rustup not found. Install Rust first: https://rustup.rs"
    exit 1
fi
info "Rust toolchain found: $(rustc --version)"

ARCH="$(uname -m)"
OS="$(uname -s)"

echo ""
echo "Host: ${OS} / ${ARCH}"

if [ "${OS}" = "Linux" ]; then
    echo ""
    echo "Installing musl toolchain packages..."
    if command -v apt-get &>/dev/null; then
        sudo apt-get install -y musl-tools gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu \
            gcc-arm-linux-gnueabihf binutils-arm-linux-gnueabihf \
            mingw-w64 2>/dev/null && info "System cross-compile toolchains installed" || \
            warn "Could not install all system packages. Some targets may require cross+Docker."
    else
        warn "Non-Debian Linux: install musl-tools and mingw-w64 manually."
    fi
fi

echo ""
echo "Adding Rust cross-compilation targets..."

rustup target add x86_64-pc-windows-gnu 2>/dev/null && \
    info "Target added: x86_64-pc-windows-gnu (Windows)" || \
    warn "Target x86_64-pc-windows-gnu may already be installed"

rustup target add x86_64-unknown-linux-musl 2>/dev/null && \
    info "Target added: x86_64-unknown-linux-musl (Linux x86_64, static)" || \
    warn "Target x86_64-unknown-linux-musl may already be installed"

rustup target add aarch64-unknown-linux-musl 2>/dev/null && \
    info "Target added: aarch64-unknown-linux-musl (Linux ARM64, static)" || \
    warn "Target aarch64-unknown-linux-musl may already be installed"

rustup target add armv7-unknown-linux-musleabihf 2>/dev/null && \
    info "Target added: armv7-unknown-linux-musleabihf (Linux ARM32, static)" || \
    warn "Target armv7-unknown-linux-musleabihf may already be installed"

echo ""
echo "Checking for Docker (needed for cross-compilation across architectures)..."
if ! command -v docker &>/dev/null; then
    warn "Docker not found. Cross-arch builds (e.g. Windows, or ARM64 from x86_64) will need it."
    echo "  Install: sudo apt-get install docker.io && sudo usermod -aG docker \$USER"
elif ! docker info &>/dev/null 2>&1; then
    warn "Docker installed but not running. Cross-arch builds will fail."
    echo "  Start:   sudo systemctl start docker"
    echo "  Group:   sudo usermod -aG docker \$USER && newgrp docker"
else
    info "Docker is running: $(docker --version)"
fi

echo ""
if command -v cross &>/dev/null; then
    info "cross is already installed: $(cross --version 2>/dev/null || echo 'installed')"
    echo ""
    read -rp "Reinstall/update cross? [y/N] " answer
    if [[ "${answer,,}" != "y" ]]; then
        echo ""
        info "Setup complete!"
        exit 0
    fi
fi

echo "Installing cross (Docker-based cargo cross-compiler)..."
cargo install cross --git https://github.com/cross-rs/cross 2>&1 | tail -5
info "cross installed successfully"

echo ""
echo "Pre-pulling cross Docker images (speeds up first build)..."

docker pull ghcr.io/cross-rs/x86_64-pc-windows-gnu:main 2>/dev/null && \
    info "Pulled Windows cross-compilation image" || \
    warn "Could not pre-pull Windows image (will be pulled on first build)"

docker pull ghcr.io/cross-rs/x86_64-unknown-linux-musl:main 2>/dev/null && \
    info "Pulled Linux x86_64 (musl) cross-compilation image" || \
    warn "Could not pre-pull Linux x86_64 image (will be pulled on first build)"

docker pull ghcr.io/cross-rs/aarch64-unknown-linux-musl:main 2>/dev/null && \
    info "Pulled Linux ARM64 (musl) cross-compilation image" || \
    warn "Could not pre-pull Linux ARM64 image (will be pulled on first build)"

docker pull ghcr.io/cross-rs/armv7-unknown-linux-musleabihf:main 2>/dev/null && \
    info "Pulled Linux ARM32 (musl) cross-compilation image" || \
    warn "Could not pre-pull Linux ARM32 image (will be pulled on first build)"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
info "Setup complete! Cross-compilation is ready."
echo ""
echo "  Build targets and strategy:"
echo ""
if [ "${ARCH}" = "aarch64" ] && [ "${OS}" = "Linux" ]; then
    echo "  You are on ARM64 Linux (Kali ARM):"
    echo "    linux-arm64  → native cargo (fastest, no Docker)"
    echo "    linux        → needs cross + Docker (QEMU x86_64 emulation)"
    echo "    windows      → needs cross + Docker"
elif [ "${ARCH}" = "x86_64" ] && [ "${OS}" = "Linux" ]; then
    echo "  You are on x86_64 Linux (Kali AMD):"
    echo "    linux        → native cargo (fastest, no Docker)"
    echo "    linux-arm64  → needs cross + Docker"
    echo "    windows      → needs cross + Docker"
else
    echo "    linux        → x86_64-unknown-linux-musl"
    echo "    linux-arm64  → aarch64-unknown-linux-musl"
    echo "    linux-arm32  → armv7-unknown-linux-musleabihf"
    echo "    windows      → x86_64-pc-windows-gnu"
    echo "    binary       → native host binary (no Docker)"
fi
echo ""
echo "  Start the server:  ./scripts/run-server.sh"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
