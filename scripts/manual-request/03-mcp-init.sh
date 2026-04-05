#!/bin/bash -e

# Usage: ./03-mcp-init.sh [credentials_file]

CLIENT_NAME="Test client"

CRED_FILE="$1"
if [[ -z "$CRED_FILE" ]]; then
  CRED_FILE="credentials.json"
fi

jq < "$CRED_FILE"
ACCESS_TOKEN=$(jq -r .access_token < "$CRED_FILE")

echo "Access token: $ACCESS_TOKEN"

# resource = MCP_ENDPOINT_URL?
MCP_ENDPOINT_URL=https://mcp.montrose.io
INIT_RESP=$(curl -s -v \
  -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H 'Accept: application/json, text/event-stream' \
  -d '{
    "jsonrpc": "2.0",
    "id": 0,
    "method": "initialize",
    "params": {
        "capabilities": {},
        "clientInfo": {
            "name": "test-client-script",
            "version": "0.1.0"
        },
        "protocolVersion": "2025-06-18"
    }
}' "$MCP_ENDPOINT_URL" 2> >(grep -v '^[*{}]' >&2))
echo "$INIT_RESP"
