# `.kiro/specs` index

Quick status map. Prefer the **Active (locked topology)** pair for mesh / identity / public surface work.

## Active (locked topology) — use these

| Spec | Role |
|------|------|
| [`3tched-ghostbridge-control-plane/`](./3tched-ghostbridge-control-plane/) | CF public surface, mail half-and-half, mesh privacy, OpenFlow IP:port, REALITY optional camouflage, thin email enroll channel. **v1.3** |
| [`netmaker-xray-identity-handoff/`](./netmaker-xray-identity-handoff/) | Oracle decoy WG termination, signed assertion, HumanPrincipal, bridge sole validator. Local cargo-test mission. |

**Topology lock:** human WG terminates at Oracle decoy only; NetMaker = transport; no host `wg-lan`; no SNI front on public `:443`; no CF tunnels into CP; gRPC at `10.0.0.2:8090` mesh-private.

## Active (code / product missions)

| Spec | Notes |
|------|-------|
| [`zeroclaw-router-wiring/`](./zeroclaw-router-wiring/) | **Final** — router ZeroClaw → op-web `:8080/v1` as machine Ghostbridge mesh client; gates in tasks.md |
| `accountability-audit-trail/` | UI audit trail |
| `cognitive-mcp-bridge-only-door/` | Bridge-only door to cognitive MCP |
| `cognitive-mcp-only-door-phase2/` | Kill :3003/:50052; fan-in proxy design |
| `dbus-service-manager/` | D-Bus service manager |
| `netmaker-custom-json-render-ui/` | json-render networking UI — **must stay mesh-private for gRPC** |
| `op-web/` | op-web UI design/tasks |
| `op-services/` | services crate |
| `remove-projection-static-tree/` | Projection removal (shipped; see tasks checkmarks) |
| `runit-sv-migration/` | s6 → runit (requirements-only) |
| `schemars-to-reflection-plugin-pipeline/` | Plugin schema pipeline |
| `unified-blob-catalog-mcp/` | Blob catalog / MCP context |
| `voyage-plugin-cognitive-mcp-boundaries/` | Voyage / cognitive-mcp boundaries |

## Superseded / do not implement

| Spec | Points to |
|------|-----------|
| `op-dbus-mirror-event-session-refactor/` | → `remove-projection-static-tree/` (see `SUPERSEDED.md`) |

## Incomplete stubs (not actionable as-is)

| Spec | Gap |
|------|-----|
| `op-web-ui/` | Empty placeholder — use `op-web/` |
| `dead-signal-and-tool-audit/` | design-only postmortem |
| `dead-signal-and-tool-cleanup/` | requirements-only follow-up |
| `crates/op-web/ui/.kiro/specs/3tched-ghostbridge/` | Empty nest — use `3tched-ghostbridge-control-plane/` |

## Rejected identity path (historical)

`claude-redo/netmaker-xray-identity-handoff/` (outside this tree) — repudiated; see handoff `boundaries.md`.
