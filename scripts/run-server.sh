#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT=3000

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

export C2_PSK="${C2_PSK:-educational-c2-psk-key}"

echo "Building React dashboard..."
if [ -d "$ROOT/dashboard-react" ]; then
  (cd "$ROOT/dashboard-react" && npm install --silent && npm run build)
fi

echo "Building server..."
cargo build -p server --release

echo "Starting C2 server on http://localhost:$PORT"
echo "Beacon endpoint: POST http://localhost:$PORT/api/beacon (AES-GCM encrypted)"
echo "Dashboard:       http://localhost:$PORT"
exec cargo run -p server --release
