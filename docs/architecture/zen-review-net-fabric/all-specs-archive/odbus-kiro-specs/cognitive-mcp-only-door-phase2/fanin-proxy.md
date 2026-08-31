# Design: cognitive MCP fan-in proxy

One MCP server fronting the bridge, serving every client. Solves three problems that
turned out to be the same problem.

Status: design settled, not implemented.

---

## Problems this closes

**1. CozoDB lock contention.** `CognitiveMcpServer::new` opens a persistent CozoDB
(`server.rs:41`). Every MCP client configured with `op-cognitive-mcp --stdio` spawns
its own instance, and the second one dies:

```
RocksDB IO error: While lock file: .../memory.db/data/LOCK:
Resource temporarily unavailable
```

Observed live: PID 11846 held the lock, so a second client got nothing. Kiro and
Factory are both configured this way today, so whichever starts first wins silently.

**2. MCP clients cannot reach the cognitive surface through the bridge.** Verified
by measurement, not assumption:

| Surface | Tools | Cognitive tools |
|---|---|---|
| `op-web` `:8080/mcp/compact` | 4 meta over 345 | 0 |
| `op-web` `:8080/mcp/agents/message` | 323 | 0 |
| `op-mcp-server --mode full` | 141 | 0 |
| `op-cognitive-mcp` `:3003` | 409 | all |

`execute_tool{tool_name:"cognitive_memory"}` on the compact endpoint returns
`Tool not found`. So the only route to cognitive tools today is
`op-cognitive-mcp --stdio`, which bypasses the bridge entirely — no method gate, no
capability check, no event chain. That is the inverse of Phase 1's goal.

**3. Identity cannot be attached per client.** `enforce_bridge_capability` needs
`capability_matches && footprint_grants`. With no `capability_grants` block in the
cognitive_mcp schema, `grants_declared` is false and the fallback is
`identity.is_some() && capability_matches`. Sending a valid footprint plus
`x-opdbus-capability` still returned `grpc-status: 7`, because no
`GhostbridgeIdentity` is attached on the generated plugin-methods route. Fixing that
for arbitrary callers is open-ended; having exactly one known caller is not.

---

## Decision

Add a mode to `op-mcp-server` that sources its registry from the bridge, and run one
supervised instance serving `--http`. All MCP clients point at it.

```
Kiro ─┐
Factory ─┼─ HTTP/MCP ─► op-mcp-server (one instance, one identity)
other ─┘                     │  PluginV1.Call invoke_tool / list_tools
                             ▼
                        op-grpc-bridge  ── method gate, arg validation,
                             │             capability check, event chain
                             ▼
                     op-cognitive-mcp (sole CozoDB owner)
```

Why `op-mcp-server` rather than npx: the binary already implements every transport
needed (`--stdio --http --sse --ws --grpc --all`) and already speaks MCP. Fronting a
Rust bridge with a Node hop would add a process to the control plane to do something
the shipped binary already does. `npx` remains appropriate for genuinely external
servers (`filesystem`, `memory`, `sequential-thinking`) and the GUI.

`ExternalMcpConfig` is not the vehicle: it is stdio-only (`command` + `args`, no
`url`), so it spawns children rather than connecting to an endpoint.

---

## Auth belongs here

The proxy is the single authenticated caller. It holds the Ghostbridge footprint and
sends `x-opdbus-capability` per call (added in `eb14debe`); individual clients carry
no identity material.

This is strictly better than the status quo in three ways:

- It sidesteps the identity-attachment blocker — one known caller instead of
  arbitrary ones.
- It makes per-client capability scoping expressible. The current blanket
  `cognitive_mcp.invoke` grant unlocks all 409 tools including
  `agent_shell_executor_exec` and `agent_python_executor_run`; the proxy can gate
  per client without touching the sealed schema.
- Every call gains an event-chain entry, which the direct stdio path does not.

---

## Requirements

**FR-1 — bridge-sourced registry.** New mode (working name `--mode cognitive`)
populating the tool list from `PluginV1.Call` `list_tools` on
`/org/opdbus/v1/plugins/cognitive_mcp`. Verified working: returns all 409 tools.

**FR-2 — dispatch via invoke_tool.** MCP `tools/call` maps to `PluginV1.Call`
`invoke_tool` with `{tool_name, arguments}`. Verified working end to end:
`success: true`, non-zero `event_id`, `isError: false`.

**FR-3 — single identity.** Proxy attaches footprint and `x-opdbus-capability`.
Clients need neither.

**FR-4 — supervised, one instance.** runit service under `/etc/runit/sv/`, `--http`
bound to a chosen address. Exactly one instance, so exactly one backend connection.

**FR-5 — client migration.** `.kiro/settings/mcp.json` and `.mcp.json` drop the
`op-cognitive-mcp --stdio` entry in favour of the proxy URL. This is what actually
removes the lock contention.

**FR-6 — no new listener without auth.** The proxy's `--http` must not repeat the
`op-web:8080` situation, where `/mcp/*` is reachable with no credential
(`ip_security_middleware` resolves an `AccessZone` then calls `next.run()`
unconditionally; `AccessZone` is read only by `groups_admin.rs` and
`handlers/pair.rs`). Bind to loopback or the mesh address, and enforce.

**NFR-1 — no Python.** `jq` for JSON assertions.

**NFR-2 — Rust only for the proxy.** No Node in the control-plane path.

---

## Open questions

1. Which address does `--http` bind? Loopback covers Kiro and Factory on this host;
   mesh clients need `100.69.0.254`, which reopens the FR-6 auth question.
2. Does the 409-tool surface need filtering before exposure? The shell and python
   executors are in it.
3. Does `--mode cognitive` compose with the existing 141-tool registry, or replace
   it? Composing risks tool-name collisions between registries.

---

## Relationship to Phase 2

This supersedes Phase 2 FR-2's migration path, which assumed mesh clients could move
from `:3003` to `:8080/mcp/compact`. That assumption was wrong — the compact endpoint
carries none of the cognitive tools. The proxy is the replacement, and it must exist
before `:3003` can be removed for any consumer.
