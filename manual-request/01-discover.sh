#!/bin/bash

# URL='https://mcp.montrose.io/.well-known/openid-configuration'
# echo $URL; curl --no-progress-meter "$URL" | jq

URL='https://mcp.montrose.io/.well-known/oauth-authorization-server'
echo $URL; curl --no-progress-meter "$URL" | jq

URL='https://mcp.montrose.io/.well-known/oauth-protected-resource'
echo $URL; curl --no-progress-meter "$URL" | jq
