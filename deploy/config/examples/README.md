# op-dbus MCP Server Configuration Examples

This directory contains example MCP server configurations for various AI clients.

## MCP Endpoints

The op-dbus MCP server exposes three main modes at `https://op-dbus.ghostbridge.tech`:

| Mode | SSE Endpoint | POST Endpoint | Description |
|------|--------------|---------------|-------------|
| **Compact** | `/mcp/compact` | `/mcp/compact/message` | 4 meta-tools for efficient tool discovery |
| **Standard** | `/mcp/sse` | `/mcp/message` | All tools exposed directly |
| **Agents** | `/mcp/agents` | `/mcp/agents/message` | Specialized AI agents |

### Compact Mode (Recommended for AI Clients)
Exposes 4 meta-tools instead of the full tool list:
- `list_tools` - Browse tools with pagination
- `search_tools` - Search tools by keyword
- `get_tool_schema` - Get schema for a specific tool
- `execute_tool` - Execute any underlying tool

### Standard Mode
Exposes all tools directly via `tools/list`. Best for clients that can handle large tool lists.

### Agents Mode
Specialized AI agents for enhanced capabilities:
- `memory` - Key-value memory (remember, recall, forget)
- `context_manager` - Context persistence (save, load, list, delete)
- `sequential_thinking` - Step-by-step reasoning
- `mem0` - Semantic memory (add, search, get_all, delete, update)
- `search_specialist` - Code/docs/web search
- `deployment` - Service deployment (deploy, rollback, status)
- `python_pro` - Python analysis and refactoring
- `debugger` - Error analysis and tracing
- `prompt_engineer` - Prompt generation and optimization

## Configuration Files

| File | Client | Notes |
|------|--------|-------|
| `cursor-mcp.json` | Cursor IDE | Uses `url` key |
| `codex-mcp.json` | Codex Client | Uses `mcpServers` with SSE transport |
| `vscode-mcp.json` | VS Code (Copilot Chat) | Uses `mcp.servers` structure |
| `claude-desktop-mcp.json` | Claude Desktop | Prefers stdio transport |
| `openai-mcp.json` | OpenAI tools | Array-based servers list |
| `generic-mcp.json` | Reference | Complete with all endpoints |

## Installation

### System-wide (Recommended)
```bash
sudo mkdir -p /etc/mcp
sudo cp *.json /etc/mcp/
```

### Cursor
```bash
# Project-level
mkdir -p .cursor
cp cursor-mcp.json .cursor/mcp.json

# User-level
cp cursor-mcp.json ~/.cursor/mcp.json
```

### VS Code
Add the contents of `vscode-mcp.json` to your VS Code `settings.json`:
```json
{
  "mcp": {
    "servers": { ... }
  }
}
```

### Claude Desktop
Claude Desktop prefers stdio-based MCP servers. Options:
1. Use the native `op-mcp-server` binary (recommended)
2. Use mcp-proxy to bridge SSE to stdio

## Testing Connection

```bash
# Test health endpoint
curl https://op-dbus.ghostbridge.tech/api/health

# Test MCP initialize
curl -X POST https://op-dbus.ghostbridge.tech/mcp/message \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'

# Test tools/list
curl -X POST https://op-dbus.ghostbridge.tech/mcp/message \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

## See Also

- [MCP Protocol Specification](https://spec.modelcontextprotocol.io/)
- [op-dbus Documentation](../../../docs/)
