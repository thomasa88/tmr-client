#!/bin/bash -e

# Usage: ./03-refresh.sh [credentials_file]

CLIENT_NAME="Test client"

CRED_FILE="$1"
if [[ -z "$CRED_FILE" ]]; then
  CRED_FILE="credentials.json"
fi

jq < "$CRED_FILE"
CLIENT_ID=$(jq -r .client_id < "$CRED_FILE")
CLIENT_SECRET=$(jq -r .client_secret < "$CRED_FILE")
REFRESH_TOKEN=$(jq -r .refresh_token < "$CRED_FILE")

echo "Credentials file: $CRED_FILE"
echo "Client ID: $CLIENT_ID"
echo "Client secret: $CLIENT_SECRET"
echo "Original refresh token: $REFRESH_TOKEN"

if [[ -z "$REFRESH_TOKEN" ]]; then
  echo "Usage: $0 <client_id> <client_secret> <refresh_token>"
  exit 1
fi

CLIENT_NAME="Test client"

TOKEN_URL=https://mcp.montrose.io/token
REG_RESP=$(curl -s -v \
  -X POST \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=refresh_token&refresh_token=${REFRESH_TOKEN}&scope=mcp&client_id=${CLIENT_ID}&client_secret=${CLIENT_SECRET}" \
  $TOKEN_URL 2> >(grep -v '^[*{}]' >&2))
echo $REG_RESP | jq

if [[ $(echo "$REG_RESP" | jq -r .error) != "null" ]]; then
  echo "Error refreshing tokens: $(echo "$REG_RESP" | jq -r .error)"
  exit 1
fi

ACCESS_TOKEN=$(echo "$REG_RESP" | jq -r '.access_token')
REFRESH_TOKEN=$(echo "$REG_RESP" | jq -r '.refresh_token')

echo "New access token: $ACCESS_TOKEN"
echo "New refresh token: $REFRESH_TOKEN"

# Rotate old credential files
mv credentials.json.3 credentials.json.4 2>/dev/null || true
mv credentials.json.2 credentials.json.3 2>/dev/null || true
mv credentials.json.1 credentials.json.2 2>/dev/null || true
mv credentials.json credentials.json.1 2>/dev/null || true

echo "{
  \"client_id\": \"$CLIENT_ID\",
  \"client_secret\": \"$CLIENT_SECRET\",
  \"access_token\": \"$ACCESS_TOKEN\",
  \"refresh_token\": \"$REFRESH_TOKEN\"
}" > credentials.json

echo
echo "Saved credentials file: credentials.json"
echo "Old credentials file: credentials.json.1"
