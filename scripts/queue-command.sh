#!/usr/bin/env bash
set -euo pipefail

SERVER_URL="${C2_SERVER_URL:-http://localhost:3000}"
AGENT_ID="${1:-agent1}"
COMMAND="${2:-whoami}"

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <agent_id> <command>"
    echo "Example: $0 agent1 'whoami'"
    exit 1
fi

echo "Queueing command for agent $AGENT_ID: $COMMAND"

curl -X POST "${SERVER_URL}/api/command/queue" \
  -H "Content-Type: application/json" \
  -d "{
    \"agent_id\": \"$AGENT_ID\",
    \"command_type\": \"shell\",
    \"payload\": \"$COMMAND\"
  }" | jq .