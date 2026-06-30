#!/usr/bin/env bash
set -euo pipefail

SERVER_URL="${C2_SERVER_URL:-https://localhost:3443}"
AGENT_ID="${1:-}"
COMMAND="${2:-whoami}"
COMMAND_TYPE="${3:-shell}"

if [ -z "$AGENT_ID" ] || [ "$#" -lt 2 ]; then
    echo "Usage: $0 <agent_uuid> <command> [command_type]"
    echo ""
    echo "Examples:"
    echo "  $0 \$(cat agent_id.txt) whoami"
    echo "  $0 \$(cat agent_id.txt) 10 sleep"
    exit 1
fi

echo "Queueing command for agent $AGENT_ID: $COMMAND ($COMMAND_TYPE)"

curl -sk -X POST "${SERVER_URL}/api/command/queue" \
  -H "Content-Type: application/json" \
  -d "{
    \"agent_id\": \"$AGENT_ID\",
    \"command_type\": \"$COMMAND_TYPE\",
    \"payload\": \"$COMMAND\"
  }"
