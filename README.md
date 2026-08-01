# 3tched Control Plane

Native, deterministic control plane for Artix Linux infrastructure.

## Architecture

- **Host**: Artix Linux + runit service supervision, controlled with `sv` (NOT systemd, NOT s6)
- **Network**: OVS switching fabric via `rovs` crate suite — native OVSDB JSON-RPC, no CLI subprocess
- **Containers**: Incus — privacy services (Xray, mail) run inside containers with Unix sockets + OpenFlow routing
- **Storage**: CozoDB (graph-relational-vector) + Btrfs vectorized footprint transport
- **AI**: Factory LLM backend with per-container memory (CozoDB + Qdrant semantic search)

## Workspace

31 crates under `crates/`:

| Crate              | Role                                                 |
| ------------------ | ---------------------------------------------------- |
| `op-web`           | Unified HTTP server + chat UI                        |
| `op-chat`          | Chat actor with memory loop, forced tool pipeline    |
| `op-llm`           | LLM provider management                              |
| `op-network`       | OVSDB + OpenFlow + rtnetlink (native protocols only) |
| `op-cognitive-mcp` | CozoDB memory store, Qdrant semantic shuttle         |
| `op-cozo-store`    | CozoDB embedded graph database                       |
| `op-grpc-bridge`   | gRPC service bridge                                  |
| `op-identity`      | WireGuard identity + magic link registration         |
| `op-state-store`   | Plugin schema engine                                 |

## Quick Start

```bash
# Build everything
cargo build --workspace --release

# Build frontend
cd lovable && npm ci && npm run build

# Run web server
cargo run --release -p op-web
```

## Key Principles

1. **Native protocols only** — OVSDB JSON-RPC, not `ovs-vsctl`; Generic Netlink, not `ip` commands
2. **Container-scoped memory** — chatbot and users each get containers; memory is isolated per container
3. **Schema-driven** — `PluginSchema` is the single source of truth for all state
4. **Zero-CLI** — all tools use programmatic APIs; no shell subprocesses in the control plane

## Documentation

- `CLAUDE.md` — agent guidance + project coding standards
- `docs/` — architecture docs
- `deploy/runit/` — runit service definitions + `recompile-and-update.sh`

## License

Apache-2.0
