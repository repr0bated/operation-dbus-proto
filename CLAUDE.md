# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

3tched / OP-DBUS: a native, deterministic control plane for Artix Linux infrastructure (runit supervision as of the 2026-07-20/21 server move, NOT systemd — was s6 before that; s6-era docs/scripts in this tree are historical, not current). ~38-crate Rust workspace under `crates/`, plus a Vite/React/shadcn frontend. All source lives inside `crates/` workspace members — never a generic top-level `src/`. Other top-level dirs: `deploy/` (service defs + install scripts, both s6 and runit variants present during the migration), `schemas/` (JSON schemas loaded at runtime), `docs/`. This file is the authoritative agent guidance; where any doc and the tree disagree, the tree wins — but also verify against the *live host* (`sudo incus config device show`, `sv status`, etc.) since the tree itself lags fast-moving infra changes.

## Build, lint, test

All cargo commands run from the repo root (the workspace root).

```bash
cargo build --workspace                 # build everything
cargo check -p <crate>                  # fast check a single crate (do this before edits)
cargo test --workspace --all-targets --all-features
cargo test -p <crate> <test_name>       # single test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Frontend — there are **two** Vite apps; don't confuse them:

1. `crates/` (`vite_react_shadcn_ts`, source in `crates/src/`) — the primary UI dev tree; all npm commands below run here.
2. `crates/op-web/ui/` (`zeroclaw-gui-repo`) — the app **op-web actually embeds** (RustEmbed of `ui/dist/`). Its package.json has no `build` script (and build.rs's error message suggesting `npm run build` is stale); rebuild with `npx vite build` from that directory. Building `crates/` does NOT update what op-web serves.

```bash
cd crates
npm run dev / build / lint / typecheck
npm test                                # vitest run
npx vitest run src/test/<file>          # single test file
```

Build gotchas:

- `[patch.crates-io]` pins zbus to a **local checkout at `/home/jeremy/git/zbus`** — the workspace does not build without it.
- `op-web` **release** builds panic unless `crates/op-web/ui/dist/index.html` exists (RustEmbed); build that UI first (`npx vite build` in `crates/op-web/ui`). Dev builds compile with an empty asset set, so the panic only bites on `--release`.
- The root `op-dbus` package has no binary — real binaries live in member crates (`op-web` → `op-web-server`/`opdbus`, `op-grpc-bridge`, `op-cognitive-mcp`, `op-projection` → `projection_server`, `op-s6-systemctl` → `s6d`, etc.).
- `op-ml` is commented out of the workspace (ort API breakage).
- Rust-first: no new Python; scripts are shell.

## Architecture — the load-bearing invariants

**D-Bus is the only control plane.** Every plugin is a D-Bus object at `/org/opdbus/v1/plugins/<name>` under `org.opdbus.v1`; reads, writes, and tool calls go through it (`PluginService.CallMethod`, or the `zbusctl` operator CLI — installed at `~/.cargo/bin/zbusctl`, not built from this repo). No `Command::new(...)` subprocesses, no direct file reads for live state, no polling loops or D-Bus watchers in plugin/service code — bootstrap scripts are the only exception. New backend capabilities are **plugins registered in `crates/op-plugins/src/default_registry.rs`**, never new gRPC proto service packages.

**PluginSchema is the single source of truth.** D-Bus method signatures, MCP tool inputs, gRPC shapes, and UI field renderers all derive from the schema; validate inputs against it. Any derived value (e.g. a catalog hash) is computed in exactly one function, one place. Current layout: each plugin defines a `<name>_schema()` function in its own file under `crates/op-plugins/src/state_plugins/`, aggregated via `plugin_scaffold_helpers.rs`; a runtime JSON loader also reads `schemas/plugin/*.json` (repo root) via `schema_loader.rs`. NOTE: older docs claim all schemas live in a single `plugin_schema_defs.rs` — **that file does not exist**; single-file consolidation is planned work (`FACTORY-PROMPT-plugin-schema-uniformization.md`), not present reality.

**The sealed blob IS the plugin.** Present state lives in the sealed blob catalog in shared memory: `/dev/shm/opdbus/plugin-blobs/<plugin_id>.<schema_hash16>.blob` (`op_blob::catalog::DEFAULT_SHM_DIR`). A plugin exists ⟺ its blob is in the catalog; register = seal, deregister = remove. The sole writer is the blob sealer in `op-blob` — never SchemaEngine (`op-projection`'s `write_schemas_to_shm` only reports the published catalog hash via `op_identity::schema_bridge::schema_catalog_hash()`). Consumers read SHM directly (1:1 zero-copy) — they never re-hash, never consult the Rust registry for existence. The old `/dev/shm/opdbus/schemas` folder, `live-schema.json` monolith, and manifest are gone; docs that mention them are stale.

**Reactive, not polled.** The system does not watch, poll, or index. SHM is the authoritative present-state that components read; an arrival (e.g. an xray greeting connection) triggers action. Durability is the per-mutation immutable blockchain (`op-blockchain`) — there are no snapshot backups and **no SQL for state** (graph store is CozoDB).

**Transport & identity (zero-trust) — intent vs current live state.** The intended/documented model (see `unix_socket.rs`'s `SHARED_CONTAINER_SOCKET`) is: gRPC (tonic, TLS mandatory) over one shared Unix domain socket per container (`/run/ghostbridge/container.sock`, bind-mounted into the container as a disk device), with the tonic-web bridge demuxing by gRPC service/method via reflection — no per-service TCP ports, no NIC. Identity = WireGuard pubkey → Argon2(PSK, salt=pubkey) sessionid; a container's name IS its sessionid. The xray router injects identity headers (`X-Ghostbridge-Footprint`/`X-WireGuard-Pubkey`) — that header is meant to be the only gate; IP ACLs/ports are theater in this model. SESSION bus = the WG-identity-gated plugin surface; SYSTEM bus = local agents/mirror.

**Live reality after the 2026-07-20/21 provider migration diverges from this** — verified directly against `incus config device show <container>` on the current host (188.68.58.237), not assumed from docs:
- Only `assistant` follows the pure shared-socket model: no TCP proxy devices at all, just `/run/ghostbridge` bind-mounted in.
- `xray` is the **sole container with a real NIC** (`eth0`, bridged onto `ovsbr0`) — it has to be, since it terminates public TLS/SNI traffic; it also gets `/run/opdbus` bind-mounted in for the shared gRPC/D-Bus sockets.
- `qdrant` is a hybrid: has the `/run/ghostbridge` bind-mount *and* two per-port `incus proxy` (TCP-to-TCP) devices exposing its raw ports (6333/6334) on the internal `svc0` bridge address (`10.200.0.2`).
- `mail-3tched`, `cozo`, and `netmaker` do **not** use the shared-socket model at all — no `ghostbridge-socket` mount on any of them. Each instead has individually-configured `incus config device add <name> proxy listen=tcp:<host-addr>:<port> connect=tcp:127.0.0.1:<port>` mappings, one per exposed port (e.g. mail-3tched: 25/143/465/587/993/80, each its own device; netmaker: 8081/8083, each with both a `10.200.0.2` and a `127.0.0.1` variant). None of these three are netns-isolated behind a shared UDS — they're plain per-container TCP port-forwards, config'd by hand rather than through `unix_socket`'s `createunixsocket` path.
- Before trusting "all container I/O is UDS" for any specific container, check its actual device list first (`sudo incus config device show <name>`) — don't assume the documented model applies uniformly. This split is recent (post-migration) and may still be settling; re-verify if it's been more than a few days.

**MCP gateways (settled — do not redesign).** `op-cognitive-mcp` is the universal gateway for ALL external clients (tonic-web gRPC :50052 + server reflection for tool discovery). `compact-mcp` is loopback-only for the chatbot. Never create new shims or point external clients at `op-assistant-grpc`.

**Host tooling.** As of the 2026-07-20/21 server move, the host runs **runit**, not s6 — `service6`/`s6d`/`s6-*` no longer exist on this box and any doc or memory saying otherwise is stale. Service definitions live under `/etc/runit/sv/<name>/`, enabled by symlinking into `/etc/runit/runsvdir/default/`; manage them with the standard `sv start|stop|restart|status|check <name>` (see `3tched-artix-runit-install.sh` for the canonical patterns, e.g. `sv start "$d"` / `until sv check "$d"; do sleep 1; done`). No wrapper/gate script exists for runit the way `service6` was for s6 — `sv` is used directly. Still be conservative with service-affecting commands (start/stop/restart) regardless of supervisor — read state before acting, don't chain speculative changes. OVS is driven natively over OVSDB JSON-RPC via the rovs plugins (`op-openvswitch-daemon` was the deprecated D-Bus-passthrough predecessor to this — it has been removed from the tree; don't recreate it). Containers are Incus; expose sockets via `zbusctl createsocket`, not raw incus proxy devices — that guidance still holds going forward even though current live containers (`assistant`, `cozo`, `mail-3tched`, `netmaker`, `qdrant`, `xray`) mix the intended shared-UDS model with hand-configured per-port `incus proxy` devices set up during the rushed post-migration demo push. See the Transport & identity note above before assuming either pattern for a given container; don't treat the raw-proxy-device pattern as newly sanctioned just because it's common right now.

## Crate map (the ones you'll actually touch)

| Crate | Role |
|---|---|
| `op-plugins` | Plugin registry + all state plugins + per-plugin schema functions (the schema source) |
| `op-projection` | SchemaEngine — schema registration/audit; reads the published catalog hash (blob sealing lives in `op-blob`) |
| `op-web` | Axum HTTP server, embeds the UI, `opdbus` CLI |
| `op-grpc-bridge` | tonic gRPC bridge (TLS) between D-Bus surface and external clients |
| `op-cognitive-mcp` | External MCP gateway, CozoDB memory, Qdrant semantic search |
| `op-identity` | WireGuard identity, sessionid derivation, sled/shuttle |
| `op-blockchain` | Per-mutation immutable chain (the durability layer) |
| `op-blob` / `op-state-store` | Sealed blob catalog / schema engine storage |
| `op-network` | Native OVSDB, OpenFlow, rtnetlink (no CLI subprocesses) |
| `op-s6-systemctl` | `s6d` service-management CLI — **legacy, s6-era**; host now runs runit (`sv`), this crate/binary is not the live control path |
| `op-xray-daemon` | xray router daemon (identity injection, subdomain routing) |
| `op-gemma` | Gemma inference plugin (schema-driven UI generation) |

## Identifiers (subid taxonomy — mandatory)

Every D-Bus object, plugin, schema, mutation, event, and tool carries a `uuid` (machine identity, never changes) **and** a `subid`:

```
<category>.<component-type>.<subject>.<verb>[.<facet>][@vN]
```

- Exactly seven categories, no others: `src` (authoritative source/ingress), `prj` (D-Bus projection/mirror publication), `sch` (schema/contract/vocabulary), `mut` (write path), `obs` (read/query/discovery), `evt` (emitted signal/audit event/proof), `exp` (consumer-facing render — MCP tool, UI surface, gRPC view).
- Component-type reuses OSCAL vocabulary (`software`, `service`, `network`, `standard`, `validation`, `policy`, …).
- `mut.*` must carry `actor_id` + `capability_id`; `evt.*` must carry `event_id`/`event_hash`.
- `subid` is an OSCAL `prop` (`ns`/`name`/`value`), never embedded in `remarks`; compliance mappings (`control_refs[]` etc.) live in metadata arrays, never inside the subid string.
- Subids are immutable per subject — material meaning changes get a new subject with `@vN`. All subids are registered in the canonical registry (`crates/op-plugins/src/state_plugins/oscal_subid_registry.rs`); uniqueness is CI-enforced.
- Examples: `src.network.ovsdb.monitor@v1` · `mut.service.state-sync.apply-patch@v1` · `exp.service.plugin-projection.render@v1`

## Conventions

- Rust: edition 2021, rustfmt (4-space, width 100), specific imports, `anyhow::Result` for app errors + `thiserror` for custom enums, `simd_json` preferred over `serde_json`.
- TypeScript/React: 2-space, functional components only, strict mode. Additional conventions in `.factory/rules/typescript.md` and `.factory/rules/testing.md`.
- `SIGNALS.md` (repo root): proactively append suggestions/concerns/observations there rather than letting them evaporate in chat. `WISHLIST.md` is the canonical task board — don't create new TODO docs.
