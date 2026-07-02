#!/usr/bin/env bash
# ============================================================================
# setup-cross.sh — One-time setup for cross-compilation dependencies.
#
# Installs:
#   1. Rust cross-compilation targets (x86_64-pc-windows-gnu, x86_64-unknown-linux-musl)
#   2. `cross` — a Docker-based cargo wrapper for portable cross-compilation
#
# Prerequisites:
#   - Rust toolchain (rustup + cargo)
#   - Docker (must be installed and running)
#
# Works on: macOS, Linux (Debian/Kali/Ubuntu), and other platforms with Docker.
# ============================================================================
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[✗]${NC} $*"; }

# ── Check Rust toolchain ────────────────────────────────────────────────────
if ! command -v rustup &>/dev/null; then
    error "rustup not found. Install Rust first: https://rustup.rs"
    exit 1
fi
info "Rust toolchain found: $(rustc --version)"

# ── Check Docker ────────────────────────────────────────────────────────────
if ! command -v docker &>/dev/null; then
    error "Docker not found."
    echo ""
    echo "  Install Docker:"
    echo "    macOS:  https://docs.docker.com/desktop/install/mac-install/"
    echo "    Linux:  sudo apt-get install docker.io && sudo usermod -aG docker \$USER"
    echo ""
    echo "  After installing, make sure Docker is running before re-running this script."
    exit 1
fi

if ! docker info &>/dev/null 2>&1; then
    error "Docker is installed but not running (or your user lacks permission)."
    echo ""
    echo "  Try:"
    echo "    sudo systemctl start docker    # Start Docker daemon"
    echo "    sudo usermod -aG docker \$USER  # Add yourself to the docker group"
    echo "    newgrp docker                  # Activate group without logout"
    echo ""
    exit 1
fi
info "Docker is running: $(docker --version)"

# ── Add Rust cross-compilation targets ──────────────────────────────────────
echo ""
echo "Adding Rust cross-compilation targets..."

rustup target add x86_64-pc-windows-gnu 2>/dev/null && \
    info "Target added: x86_64-pc-windows-gnu (Windows)" || \
    warn "Target x86_64-pc-windows-gnu may already be installed"

rustup target add x86_64-unknown-linux-musl 2>/dev/null && \
    info "Target added: x86_64-unknown-linux-musl (Linux, static)" || \
    warn "Target x86_64-unknown-linux-musl may already be installed"

rustup target add aarch64-unknown-linux-musl 2>/dev/null && \
    info "Target added: aarch64-unknown-linux-musl (Linux ARM64, static)" || \
    warn "Target aarch64-unknown-linux-musl may already be installed"

rustup target add armv7-unknown-linux-musleabihf 2>/dev/null && \
    info "Target added: armv7-unknown-linux-musleabihf (Linux ARM32, static)" || \
    warn "Target armv7-unknown-linux-musleabihf may already be installed"

# ── Install cross ──────────────────────────────────────────────────────────
echo ""
if command -v cross &>/dev/null; then
    info "cross is already installed: $(cross --version 2>/dev/null || echo 'unknown version')"
    echo ""
    read -rp "Reinstall/update cross? [y/N] " answer
    if [[ "${answer,,}" != "y" ]]; then
        echo ""
        info "Setup complete! You can now build agents for any platform."
        exit 0
    fi
fi

echo "Installing cross (this may take a minute)..."
cargo install cross --git https://github.com/cross-rs/cross 2>&1 | tail -5
info "cross installed successfully: $(cross --version 2>/dev/null || echo 'installed')"

# ── Pull Docker images (optional pre-warm) ─────────────────────────────────
echo ""
echo "Pre-pulling cross Docker images (speeds up first build)..."
echo "  This may take a few minutes on first run..."

docker pull ghcr.io/cross-rs/x86_64-pc-windows-gnu:main 2>/dev/null && \
    info "Pulled Windows cross-compilation image" || \
    warn "Could not pre-pull Windows image (will be pulled on first build)"

docker pull ghcr.io/cross-rs/x86_64-unknown-linux-musl:main 2>/dev/null && \
    info "Pulled Linux (musl) cross-compilation image" || \
    warn "Could not pre-pull Linux image (will be pulled on first build)"

docker pull ghcr.io/cross-rs/aarch64-unknown-linux-musl:main 2>/dev/null && \
    info "Pulled Linux ARM64 (musl) cross-compilation image" || \
    warn "Could not pre-pull Linux ARM64 image (will be pulled on first build)"

docker pull ghcr.io/cross-rs/armv7-unknown-linux-musleabihf:main 2>/dev/null && \
    info "Pulled Linux ARM32 (musl) cross-compilation image" || \
    warn "Could not pre-pull Linux ARM32 image (will be pulled on first build)"

# ── Done ────────────────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
info "Setup complete! Cross-compilation is ready."
echo ""
echo "  Supported build targets:"
echo "    • windows      → x86_64-pc-windows-gnu      (via cross + Docker)"
echo "    • linux        → x86_64-unknown-linux-musl  (via cross + Docker, or native cargo on Linux)"
echo "    • linux-arm64  → aarch64-unknown-linux-musl (via cross + Docker, or native cargo on ARM64 Linux)"
echo "    • linux-arm32  → armv7-unknown-linux-musleabihf (via cross + Docker, or native cargo on ARM32 Linux)"
echo "    • binary       → native host binary          (plain cargo, no Docker needed)"
echo ""
echo "  Start the server:  ./scripts/run-server.sh"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
