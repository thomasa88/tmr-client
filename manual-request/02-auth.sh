#!/bin/bash -e

CLIENT_NAME="Test client"

URL=https://mcp.montrose.io/register
REG_RESP=$(curl -s -v \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{
	"client_name": "$CLIENT_NAME",
	"redirect_uris": [ "http://localhost:9000/cb" ],
	"grant_types": [ "authorization_code", "refresh_token" ],
  "token_endpoint_auth_method": "none",
	"response_types": [ "code" ],
  "scope": "mcp"
}' \
  $URL)
echo $REG_RESP | jq

CLIENT_ID=$(echo $REG_RESP | jq -r .client_id)
CLIENT_SECRET=$(echo $REG_RESP | jq -r .client_secret)
REDIRECT_URI=http://localhost:9000/cb
SCOPE=mcp

# PKCE
# From https://zuplo.com/docs/articles/manual-mcp-oauth-testing
CODE_VERIFIER=$(openssl rand -base64 32 | tr '/+' '_-' | tr -d '=')
CODE_CHALLENGE=$(echo -n "$CODE_VERIFIER" | openssl dgst -sha256 -binary | openssl base64 | tr '/+' '_-' | tr -d '=')
STATE=$(openssl rand -hex 16)

AUTH_URL="https://identity.carnegie.se/oauth/v2/oauth-authorize?response_type=code&client_id=${CLIENT_ID}&redirect_uri=${REDIRECT_URI}&scope=${SCOPE}&state=${STATE}&code_challenge=${CODE_CHALLENGE}&code_challenge_method=S256"

echo "Open the following URL in your browser to authorize the client:"
echo "$AUTH_URL"
read -p "Enter the URL which the browser was redirected to: " RESPONSE_URL
# Extract the authorization code from the response URL
AUTH_CODE=$(echo "$RESPONSE_URL" | grep -oP 'code=\K[^&]+')
# Should be verified
# RESP_STATE=$(echo "$RESPONSE_URL" | grep -oP 'state=\K[^&]+')
echo "Authorization code: $AUTH_CODE"

TOKEN_URL=https://mcp.montrose.io/token
# reource=https://mcp.montrose.io
TOKEN_RESP=$(curl -s -v \
  -X POST \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=authorization_code&code=${AUTH_CODE}&redirect_uri=${REDIRECT_URI}&code_verifier=${CODE_VERIFIER}&client_id=${CLIENT_ID}&client_secret=${CLIENT_SECRET}" \
  $TOKEN_URL)
echo $TOKEN_RESP | jq

if [[ $(echo "$TOKEN_RESP" | jq -r .error) != "null" ]]; then
  echo "Error getting tokens: $(echo "$TOKEN_RESP" | jq -r .error)"
  exit 1
fi

ACCESS_TOKEN=$(echo "$TOKEN_RESP" | jq -r '.access_token')
REFRESH_TOKEN=$(echo "$TOKEN_RESP" | jq -r '.refresh_token')

echo "Client ID: $CLIENT_ID"
echo "Client secret: $CLIENT_SECRET"
echo "Access token: $ACCESS_TOKEN"
echo "Refresh token: $REFRESH_TOKEN"

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
