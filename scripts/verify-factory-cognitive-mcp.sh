#!/bin/sh
# Factory/Droid uses the host-local MCP adapter, which fans into the canonical
# bridge-owned Cognitive runtime. Keep this wrapper for existing operator
# muscle memory while enforcing the same ingress contract as every client.

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
factory_config="$repo_root/deploy/config/factory-mcp.json"

"$repo_root/scripts/verify-cognitive-ingress.sh"

jq -e '
  .mcpServers["cognitive-mcp"].type == "stdio" and
  .mcpServers["cognitive-mcp"].command == "/usr/local/bin/op-mcp-server" and
  .mcpServers["cognitive-mcp"].args == ["--stdio", "-m", "cognitive"]
' "$factory_config" >/dev/null

printf 'PASS  Factory/Droid is configured for the canonical stdio adapter\n'
