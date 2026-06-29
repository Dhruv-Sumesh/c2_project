#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

export C2_SERVER_URL="${C2_SERVER_URL:-http://localhost:3000}"
export C2_PSK="${C2_PSK:-educational-c2-psk-key}"

echo "Building agent..."
cargo build -p agent --release

echo "Starting agent with encrypted beacon to ${C2_SERVER_URL}"
echo "Using PSK: (set C2_PSK env to override default)"
exec cargo run -p agent --release
