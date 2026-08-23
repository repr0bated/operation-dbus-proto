#!/bin/sh
# Install and start the op-mcp-agents runit service (mirrors op-mcp-compact).
set -eu
SRC="$(cd "$(dirname "$0")/op-mcp-agents" && pwd)"
SV=/etc/runit/sv/op-mcp-agents

install -d -m 0755 "$SV/env" "$SV/log" /var/log/op-dbus/op-mcp-agents
install -m 0755 "$SRC/run" "$SV/run"
install -m 0755 "$SRC/log/run" "$SV/log/run"
install -m 0644 "$SRC/env/OP_MCP_MODE" "$SV/env/OP_MCP_MODE"
install -m 0644 "$SRC/env/OP_MCP_HTTP" "$SV/env/OP_MCP_HTTP"
install -m 0644 "$SRC/env/OP_MCP_LOG_LEVEL" "$SV/env/OP_MCP_LOG_LEVEL"
install -m 0644 "$SRC/env/WG_INTERFACE" "$SV/env/WG_INTERFACE"
ln -sfn "$SV" /etc/runit/runsvdir/default/op-mcp-agents

# Drop any ad-hoc user instance holding :11437
fuser -k 11437/tcp 2>/dev/null || true
sleep 1
sv start op-mcp-agents
sv status op-mcp-agents
