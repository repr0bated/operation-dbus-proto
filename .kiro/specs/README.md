# `.kiro/specs` index

Quick status map. Prefer the **Active (locked topology)** pair for mesh / identity / public surface work.

## Active (locked topology) — use these

| Spec | Role |
|------|------|
| [`3tched-ghostbridge-control-plane/`](./3tched-ghostbridge-control-plane/) | CF public surface, mail half-and-half, mesh privacy, OpenFlow IP:port, REALITY optional camouflage, thin email enroll channel. **v1.3** |
| [`netmaker-xray-identity-handoff/`](./netmaker-xray-identity-handoff/) | Oracle decoy WG termination, signed assertion, HumanPrincipal, bridge sole validator. Local cargo-test mission. |

**Topology lock:** human WG terminates at Oracle decoy only; NetMaker = transport; no host `wg-lan`; no SNI front on public `:443`; no CF tunnels into CP; gRPC/MCP at `10.0.0.3:8090` mesh-private. The address update is owned by [`standalone-emqx-identity-mcp/`](./standalone-emqx-identity-mcp/); it does not add a second endpoint.

| [`subscriber-registration-flow/`](./subscriber-registration-flow/) | Anonymous voucher-based purchase → WG config delivery via CF Pages. No user email. Payment–WG separation via bearer voucher. |

## Active (code / product missions)

| Spec | Notes |
|------|-------|
| [`zeroclaw-router-wiring/`](./zeroclaw-router-wiring/) | **Final** — router ZeroClaw → op-web `:8080/v1` as machine Ghostbridge mesh client; gates in tasks.md |
| [`unified-authenticated-mcp-cognitive-control-plane/`](./unified-authenticated-mcp-cognitive-control-plane/) | **Canonical** — one authenticated MCP/cognitive/blob/memory/context ingress at `op-grpc-bridge` TLS `:8090`; supersedes the four cognitive-MCP/blob specs below |
| [`standalone-emqx-identity-mcp/`](./standalone-emqx-identity-mcp/) | **Active overlay** — standalone local EMQX plugin/hooks, real identity E2E, singleton-chatbot compact view, and external-agent HOT/tool-set projections over the same `10.0.0.3:8090/mcp` ingress |
| [`control-plane-chatbot-identity/`](./control-plane-chatbot-identity/) | **Decision record 2026-09-04** — chatbot = service principal on the HOT+`toolsets` MCP door (FR-12..14), delegation via an `agents` toolset backed by `op-agents` (no direct execution, no internet, local gemma default); op-web in-process compact meta-tools retired for the chatbot; UI chat runs as the human; model pickers for both via `tched_router`; fail-closed guardrails against the chatbot becoming the fallback identity. |
| `accountability-audit-trail/` | UI audit trail |
| `dbus-service-manager/` | D-Bus service manager |
| `netmaker-custom-json-render-ui/` | json-render networking UI — **must stay mesh-private for gRPC** |
| `op-web/` | op-web UI design/tasks |
| `op-services/` | services crate |
| `remove-projection-static-tree/` | Projection removal (shipped; see tasks checkmarks) |
| `runit-sv-migration/` | s6 → runit (requirements-only) |
| `schemars-to-reflection-plugin-pipeline/` | Plugin schema pipeline |

## Superseded / do not implement

| Spec | Points to |
|------|-----------|
| `op-dbus-mirror-event-session-refactor/` | → `remove-projection-static-tree/` (see `SUPERSEDED.md`) |
| `cognitive-mcp-bridge-only-door/` (deleted) | → `unified-authenticated-mcp-cognitive-control-plane/` |
| `cognitive-mcp-only-door-phase2/` (deleted) | → `unified-authenticated-mcp-cognitive-control-plane/` |
| `unified-blob-catalog-mcp/` (deleted) | → `unified-authenticated-mcp-cognitive-control-plane/` |
| `voyage-plugin-cognitive-mcp-boundaries/` (deleted) | → `unified-authenticated-mcp-cognitive-control-plane/` |

## Incomplete stubs (not actionable as-is)

| Spec | Gap |
|------|-----|
| `op-web-ui/` | Empty placeholder — use `op-web/` |
| `dead-signal-and-tool-audit/` | design-only postmortem |
| `dead-signal-and-tool-cleanup/` | requirements-only follow-up |
| `crates/op-web/ui/.kiro/specs/3tched-ghostbridge/` | Empty nest — use `3tched-ghostbridge-control-plane/` |

## Rejected identity path (historical)

`claude-redo/netmaker-xray-identity-handoff/` (outside this tree) — repudiated; see handoff `boundaries.md`.
