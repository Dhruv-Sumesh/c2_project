#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

export C2_SERVER_URL="${C2_SERVER_URL:-https://localhost:3443}"
export C2_PSK="${C2_PSK:-educational-c2-psk-key}"

echo "Building agent..."
cargo build -p agent --release

echo "Starting agent with HTTPS encrypted beacon to ${C2_SERVER_URL}"
echo "Agent ID: agent_id.txt | Session key: session_key.txt"
exec cargo run -p agent --release
