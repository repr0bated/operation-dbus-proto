# 3tched Control Plane

Native, deterministic control plane for Artix Linux infrastructure.

## Architecture

- **Host**: Artix Linux + runit service supervision, controlled with `sv` (NOT systemd, NOT s6)
- **Network**: OVS switching fabric via `rovs` crate suite — native OVSDB JSON-RPC, no CLI subprocess
- **Containers**: Incus — privacy services (Xray, mail) run inside containers with Unix sockets + OpenFlow routing
- **Storage**: CozoDB (graph-relational-vector) + Btrfs vectorized footprint transport
- **AI**: Factory LLM backend with per-container memory (CozoDB + Qdrant semantic search)

## Workspace

~40 crates under `crates/` (see AGENTS.md for the current workspace map):

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
# Build everything (dev)
cargo build --workspace

# Release build (needs the cstdint shim for vendored RocksDB)
CXXFLAGS="-include cstdint" cargo build --workspace --release

# Build the embedded UI that op-web serves (required for release op-web)
cd crates/op-web/ui && npx vite build

# Run web server (dev — empty UI assets are fine in dev)
cargo run -p op-web
```

## Key Principles

1. **Native protocols only** — OVSDB JSON-RPC, not `ovs-vsctl`; Generic Netlink, not `ip` commands
2. **Container-scoped memory** — chatbot and users each get containers; memory is isolated per container
3. **Schema-driven** — `PluginSchema` is the single source of truth for all state
4. **Zero-CLI** — all tools use programmatic APIs; no shell subprocesses in the control plane

## Documentation

- `AGENTS.md` — agent guidance, build gotchas, host-service policy
- `SIGNALS.md` — live model observations (append-only)
- `WISHLIST.md` — task board (OD-## ids)
- `docs/` — architecture docs
- `deploy/runit/` — runit service definitions

## License

Apache-2.0
