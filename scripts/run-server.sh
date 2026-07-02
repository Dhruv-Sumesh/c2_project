#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT=3443
# Display address for status messages — override if binding to a specific interface.
C2_BIND_ADDR="${C2_BIND_ADDR:-localhost}"

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

# ── Pre-flight: cross-compilation dependencies ─────────────────────────────
# These are optional — native/"binary" builds always work with plain cargo.
# Cross-compilation (Windows/Linux from a different host) requires cross + Docker.
CROSS_OK=true
if ! command -v cross &>/dev/null; then
  echo ""
  echo "⚠  'cross' is not installed. Cross-compilation (Windows/Linux) will not work."
  echo "   Native builds ('binary') will still work fine."
  echo "   To enable cross-compilation, run:  ./scripts/setup-cross.sh"
  echo ""
  CROSS_OK=false
fi

if [ "$CROSS_OK" = true ]; then
  if ! docker info &>/dev/null 2>&1; then
    echo ""
    echo "⚠  Docker is not running. Cross-compilation (Windows/Linux) will not work."
    echo "   Native builds ('binary') will still work fine."
    echo "   Start Docker and restart the server to enable cross-compilation."
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

echo "Starting C2 server on https://${C2_BIND_ADDR}:${PORT}"
echo "Beacon endpoint: POST https://${C2_BIND_ADDR}:${PORT}/api/beacon (AES-GCM encrypted)"
echo "Dashboard:       https://${C2_BIND_ADDR}:${PORT}"
exec cargo run -p server --release

