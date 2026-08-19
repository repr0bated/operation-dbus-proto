/\[ -r \/etc\/op-dbus\/secrets\/voyage-api-key \]/a\
export COGNITIVE_MCP_DB_PATH=/var/lib/op-cognitive-mcp/memory.db\
[ -r /etc/op-dbus/secrets/notebooklm-cookie ] && export NOTEBOOKLM_COOKIE="$(cat /etc/op-dbus/secrets/notebooklm-cookie)"
