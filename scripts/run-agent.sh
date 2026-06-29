#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

export C2_SERVER_WS_URL="${C2_SERVER_WS_URL:-ws://localhost:3000/api/agent/ws}"
export C2_PSK="${C2_PSK:-educational-c2-psk-key}"

echo "Building agent..."
cargo build -p agent --release

echo "Connecting agent to ${C2_SERVER_WS_URL}"
exec cargo run -p agent --release
