#!/bin/bash -e

# Script used to detect changes in the Montrose MCP API

cd "$(dirname "$0")"

cargo r --example=devel-introspect > new_api.txt

if diff --color -u api.txt new_api.txt; then
    echo "API is unchanged"
    exit 0
else
    echo "API has changed"
    exit 1
fi
