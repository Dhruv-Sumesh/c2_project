#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Changed from WS_URL to SERVER_URL and using HTTP endpoint
export C2_SERVER_URL="${C2_SERVER_URL:-http://localhost:3000}"
export C2_AGENT_TOKEN="${C2_AGENT_TOKEN:-default-token-change-me}"

echo "Building agent..."
cargo build -p agent --release

echo "Starting agent with beacon to ${C2_SERVER_URL}"
exec cargo run -p agent --release