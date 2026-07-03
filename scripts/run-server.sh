#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT=3443

if pids=$(lsof -tiTCP:"$PORT" -sTCP:LISTEN 2>/dev/null); then
  echo "Stopping existing server on port $PORT (PID: ${pids//$'\n'/ })..."
  kill $pids 2>/dev/null || true
  for _ in 1 2 3 4 5; do
    lsof -tiTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1 || break
    sleep 0.2
  done
  if lsof -tiTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "Error: port $PORT is still in use. Stop the process manually:"
    echo "  lsof -tiTCP:$PORT -sTCP:LISTEN | xargs kill"
    exit 1
  fi
fi

ARCH="$(uname -m)"
OS="$(uname -s)"

echo ""
echo "Host: ${OS} / ${ARCH}"

if [ "${OS}" = "Linux" ] && [ "${ARCH}" = "aarch64" ]; then
  echo "ARM64 Kali: linux-arm64 and windows build natively (no Docker). x86_64 Linux needs cross+Docker."
elif [ "${OS}" = "Linux" ] && [ "${ARCH}" = "x86_64" ]; then
  echo "x86_64 Kali: linux and windows build natively. ARM64 targets need cross+Docker."
fi

if ! command -v cross &>/dev/null; then
  echo ""
  echo "⚠  'cross' is not installed. Cross-arch builds (Windows, ARM64 from x86_64, etc.) will not work."
  echo "   Native builds for your host arch will still work fine."
  echo "   To enable cross-arch builds, run:  ./scripts/setup-cross.sh"
  echo ""
fi

if command -v cross &>/dev/null; then
  if ! docker info &>/dev/null 2>&1; then
    echo ""
    echo "⚠  Docker is not running. Cross-arch builds will not work."
    echo "   Native builds for your host arch will still work fine."
    echo "   Start Docker:  sudo systemctl start docker"
    echo ""
  fi
fi

export C2_PSK="${C2_PSK:-educational-c2-psk-key}"

echo "Building React dashboard..."
if [ -d "$ROOT/dashboard-react" ]; then
  (cd "$ROOT/dashboard-react" && npm install --silent && npm run build)
fi

echo "Building server..."
cargo build -p server --release

echo "Starting C2 server on https://0.0.0.0:${PORT}"
echo "Dashboard: https://$(hostname -I | awk '{print $1}' 2>/dev/null || echo localhost):${PORT}"
exec cargo run -p server --release
