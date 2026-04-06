CLIENT_NAME="Test client"

parse_curl() {
  local section="$1"
  local resp="$2"
  # State machine that finds each section of the output
  echo "$resp" | awk '
    /^>/ && !section          { section="reqhead" }
    /^[{}]/                   { if (section=="reqhead") { section="reqtls" }
                                else if (section=="resphead") { section="resptls" } }
    /^</ && section=="reqtls" { section="resphead"}
    section=="respbody_next"  { section="respbody" }
    /^\* Connection.*to host/ && section=="resptls" { section="respbody_next" }
    section=="'$section'"     { print }
'
}

# Runs curl with the given data and curl arguments and prints the request and
# response in a readable format.
# $1: Data to send in the request body
# $2..$n: Additional curl arguments
# Returns: The response body
call_curl() {
  local data="$1"
  shift
  local resp=$(curl -s -v -d "$data" "$@" 2>&1)

  parse_curl "reqhead" "$resp" >&2
  echo "$DATA" | while read; do echo "> $REPLY"; done >&2
  parse_curl "resphead" "$resp" >&2
  local resp_body_raw=$(parse_curl "respbody" "$resp")
  local resp_body=$(echo "$resp_body_raw" | jq 2>/dev/null || echo "$resp_body_raw")
  echo "$resp_body" | while read; do echo "< $REPLY"; done >&2

  echo "$resp_body"
}

# Saves credentials
#
# Old credentials are rotated to files with increasing number suffix.
#
# Expects the following variables to be set:
# - CLIENT_ID
# - CLIENT_SECRET
# - ACCESS_TOKEN
# - REFRESH_TOKEN
save_credentials() {
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
}
