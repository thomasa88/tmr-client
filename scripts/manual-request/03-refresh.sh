#!/bin/bash -e

# Usage: ./03-refresh.sh [credentials_file]

SCRIPT_DIR="$(dirname "$0")"
source "$SCRIPT_DIR/functions.sh"

CRED_FILE="$1"
if [[ -z "$CRED_FILE" ]]; then
  CRED_FILE="credentials.json"
fi

CLIENT_ID=$(jq -r .client_id < "$CRED_FILE")
CLIENT_SECRET=$(jq -r .client_secret < "$CRED_FILE")
REFRESH_TOKEN=$(jq -r .refresh_token < "$CRED_FILE")

echo "Credentials file: $CRED_FILE"
echo "Client ID: $CLIENT_ID"
echo "Client secret: $CLIENT_SECRET"
echo "Original refresh token: $REFRESH_TOKEN"
echo

if [[ -z "$REFRESH_TOKEN" ]]; then
  echo "Usage: $0 <client_id> <client_secret> <refresh_token>"
  exit 1
fi

TOKEN_URL=https://mcp.montrose.io/token
DATA="grant_type=refresh_token&refresh_token=${REFRESH_TOKEN}&scope=mcp&client_id=${CLIENT_ID}&client_secret=${CLIENT_SECRET}"
REG_RESP=$(call_curl "$DATA" \
  -X POST \
  -H "Content-Type: application/x-www-form-urlencoded" \
  $TOKEN_URL)

if [[ $(echo "$REG_RESP" | jq -r .error) != "null" ]]; then
  echo "Error refreshing tokens: $(echo "$REG_RESP" | jq -r .error)"
  exit 1
fi

ACCESS_TOKEN=$(echo "$REG_RESP" | jq -r '.access_token')
REFRESH_TOKEN=$(echo "$REG_RESP" | jq -r '.refresh_token')

echo
echo "New access token: $ACCESS_TOKEN"
echo "New refresh token: $REFRESH_TOKEN"

save_credentials

echo
echo "Saved credentials file: credentials.json"
echo "Old credentials file: credentials.json.1"
