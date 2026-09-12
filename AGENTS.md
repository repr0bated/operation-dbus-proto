# AGENTS.md

This file provides guidance to the AI agent when working with code in this repository.

## Mandatory skill preload — every agent and model

At the start of every session, before any analysis, planning, edits, or subagent work, read these four files completely:

- `.agents/skills/grpc-expert/SKILL.md`
- `.agents/skills/grpc-protocol-expert/SKILL.md`
- `.agents/skills/json-render/SKILL.md`
- `.agents/skills/ovs-db-analysis/SKILL.md`

Mandatory even when the task does not mention these domains. Every subagent and model handoff must repeat the preload for its own session — a parent's preload does not count for a child. For OP-DBUS PluginSchema, generated plugin services, reflection, and seal/freeze/hot work, `grpc-expert` takes precedence over the generic `grpc-protocol-expert`; use the generic skill for protocol, channel, TLS, streaming, and observability concerns.

## Host-service policy (runit)

This host runs **runit** as PID 1. Manage services with `sudo sv <command> <service>` (e.g., `sudo sv restart op-grpc-bridge`). `sv` needs root to read `supervise/ok`.

- `/etc/runit/sv/<service>/run` is the service definition.
- Enabled by a symlink in `/etc/runit/runsvdir/default`.
- Update path: edit the `run` script or install a new binary, then `sudo sv restart <service>`.

Do not edit `/run/runit/service` directly. Do not invoke `runsv`/`runsvdir` by hand. **s6 is legacy**: `service6`, `s6-rc`, `s6-*`, `/run/service` are stale — treat any doc assuming them as historical. Note: `/usr/local/bin/systemctl` on this host is a shim that returns plausible answers — using it masks the deprecation; always use `sv`. Do not use `systemctl` or other foreign service-manager CLIs. Container/app deployment uses D-Bus via `busctl`.

## Xray configuration policy — mandatory

**Xray's live config must exist only at `/etc/xray/xray_config.json` inside the container.** Never point Xray at `/dev/shm/xray_config.json`, `/usr/local/etc/xray/config.json`, or any other disk-backed live path. The static bootstrap config is materialized at boot; the future model-generated generator replaces the same file atomically and reloads through D-Bus. Models must not write or reload Xray directly.

## Deployment (btrfs send/receive)

Release is a snapshot sent to the target — defined by `deploy/btrfs-layout.sh` (base/modules/snapshots/staging). Never hand-copy binaries onto a running host as a deploy step.

```sh
CXXFLAGS="-include cstdint" cargo build --workspace --release
sudo target/release/opblob seal-shm            # explicitly reseal after the build
sudo target/release/opblob persist             # retain the catalog across reboot
sudo deploy/runit/build-golden.sh             # golden subvolume + live install
sudo deploy/runit/build-golden.sh --dry-run   # review first
```

After builds, explicitly run the freshly built `opblob seal-shm`; the bridge's startup refresh is not a substitute for this step. Use the service's schema-shaping environment when sealing: on this host, `sudo env COGNITIVE_MCP_QDRANT_URL=http://10.200.0.2:6334 target/release/opblob seal-shm` matches `deploy/runit/op-grpc-bridge/run`. Verify the live and persisted catalogs match after restart. Review catalog removals and keep a recoverable copy before a full reseal, which sweeps plugin IDs absent from the build.

`--golden-only` skips the running host; `--live-only` skips the subvolume. Network-critical services (OVS, uplink, DHCP, session bus) are never auto-restarted — the script reports them for deliberate console action.

## Workspace shape and build

~40 Rust crates under `crates/` (workspace root). The root `op-dbus` package has **no `[[bin]]`** — real deployables live in member crates (`op-grpc-bridge`, `op-web` → `op-web-server`/`opdbus`, `op-cognitive-mcp`, `op-projection` → `projection_server`, `op-runit-systemctl` → `svd`, etc.). `[patch.crates-io]` patches only `rovs-openflow` (vendored fork for OF1.5 `packet_type` match at `vendor/rovs-openflow`). `op-ml` is commented out of the workspace (ort API breakage, see `#ml-fixme`).

**Rust-first: no new Python.** New automation, probes, and helpers go in Rust (`cargo run --bin <name> -p <crate> -- <args>`, a small binary in the relevant crate, or a `cargo-script`-style one-shot) or in shell + POSIX tools (`jq`, `curl`, `busctl`, `sv`). Do not add new `.py` files to the repo. Pre-existing scripts under `scripts/` (`export-llm-sessions-to-notebooklm-sources.py`, `or-fusion-archive.py`, `notebook-sources-cleanup.py`, etc.) are grandfathered — maintain them in place, don't propagate the pattern.

All cargo commands run from repo root:

```bash
cargo build --workspace
cargo check -p <crate>
cargo test --workspace --all-targets --all-features
cargo test -p <crate> <test_name>
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

**Build gotchas**:
- **Release builds need `CXXFLAGS="-include cstdint"`** — vendored RocksDB in `cozorocks` fails on modern GCC without it.

**Frontend — single Vite app, external repo**:

The active UI lives in **`/srv/git/vercel-json-render-ui`** (not in this repo). It is a Vite + React app that op-web serves as static files from `/usr/local/share/op-dbus/dashboard/vercel-json-render/` at `http://10.0.0.1:8080/vercel-json-render/`.

```bash
# Build and deploy (from the UI repo)
cd /srv/git/vercel-json-render-ui && ./scripts/deploy-op-web.sh

# Dev server
cd /srv/git/vercel-json-render-ui && npm run dev
```

The script runs `npm run build -- --base /vercel-json-render/`, then rsyncs `dist/` into the static directory. op-web does **not** embed the UI via RustEmbed — it serves the deployed static files.

**Deprecated** (do not use for new work):
- `crates/op-web/ui/` (`zeroclaw-gui-repo`) — old RustEmbed path, no longer the active UI.
- `/srv/git/operation-dashboard-ui-07` — old operator console, superseded by `vercel-json-render-ui`.
- `crates/` dev dashboard (`crates/src/`) — legacy dev tooling.

**CI does NOT gate `cargo test` or `cargo clippy`** — never assume "CI green" means tests pass. The only CI code review is the Codex full-repo-review action (best-effort LLM pass, not a deterministic check).

## Conventional commits

`feat` / `fix` / `docs` / etc. with scoped `(<crate>)` and `!` for breaking changes (e.g., `feat(op-plugins)!:`, `fix(mcp):`). Convention only — no enforced hook.

## Verify before marking done

Run `/verify` before reporting work complete. CI won't catch fmt drift, clippy warnings, or test regressions, so the agent must. `/verify` runs `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets --all-features -- -D warnings` + `cargo test --workspace --all-targets --all-features`.

## Architecture — the load-bearing invariants (read these before touching plugins, state, or identity)

- **D-Bus is the only control plane.** Every plugin is a D-Bus object at `/org/opdbus/v1/plugins/<name>` under `org.opdbus.v1`. No `Command::new(...)` subprocesses, no direct file reads for live state, no polling loops in plugin/service code. New backend capabilities register as plugins in `crates/op-plugins/src/default_registry.rs` — never as new gRPC proto service packages.
- **PluginSchema is the single source of truth.** D-Bus method signatures, MCP tool inputs, gRPC shapes, and UI field renderers all derive from it. Validate inputs against the schema. Each plugin defines a `<name>_schema()` function under `crates/op-plugins/src/state_plugins/`, aggregated via `plugin_scaffold_helpers.rs`; a runtime JSON loader also reads `schemas/plugin/*.json` via `schema_loader.rs`. Older docs claiming a single `plugin_schema_defs.rs` are stale — that file does not exist.
- **The sealed blob IS the plugin.** Present state lives in the sealed blob catalog at `/dev/shm/opdbus/plugin-blobs/<plugin_id>.<schema_hash16>.blob`. A plugin exists ⟺ its blob is in the catalog. The sole writer is the blob sealer in `op-blob`. Consumers read SHM directly (zero-copy) — they never re-hash and never consult the Rust registry for existence. The old `/dev/shm/opdbus/schemas` folder and `live-schema.json` are gone.
- **MCP gateway (settled).** One public MCP door: `:8090` on `op-grpc-bridge` (cognitive MCP is in-process there; no standalone `op-cognitive-mcp` runit service). `compact_mcp` is a retired plugin id. Compact *mode* (4 meta-tools) is in-process in op-web for the singleton control-plane chatbot; it is not an MCP plugin and not a second listener. Never create new shims or point external clients at `op-assistant-grpc`.
- **External binaries are upstream; odBus scaffolding projects them in.** When a capability is backed by an external binary (e.g. `/usr/bin/zeroclaw`), the binary is autonomous and keeps its own name. The odBus side — plugin (`tched_router`), captured schema (`schemas/tched-router/`), sealed blob — is scaffolding that extends the binary's functionality into D-Bus/gRPC/MCP. Scaffolding uses the odBus plugin name, not the binary's name. `Command::new("zeroclaw")` calls the binary; everything around it is `tched_router`. This naming rule applies to function names, method descriptions, transport strings, schema IDs, env vars, file names, and proto packages. The only things that keep the binary's name are: the `Command::new()` invocation, the source attribution (`zeroclaw config schema`), and upstream Rust type paths (`zeroclaw::Config`).
- **Btrfs is transport, not backup/restore.** `btrfs send/receive` is the deploy mechanism — `build-golden.sh` creates a golden subvolume, `btrfs send` ships it to the target. Subvolume count must stay at **5 or fewer**: 1 golden for send/receive, 2-3 rotating for cache. The snowball timing/vectors/state are data directories inside the golden subvolume, not separate btrfs subvolumes. `deploy/btrfs-layout.sh` (with its base/modules/snapshots/staging hierarchy and 5 nested module subvolumes) models a backup system and is stale design — do not add new subvolumes based on it.
- **No SQLite — use CozoDB.** CozoDB (`op-cozo-store`) is the embedded database for this project. Do not add `rusqlite` or other SQLite dependencies. The `btrfs_cache.rs` SQLite index is wrong and should migrate to CozoDB.

## Signals and task board

- **Append observations to `SIGNALS.md`** rather than letting them evaporate in chat. Three signal types: `💡 SUGGESTION` · `⚠️ CONCERN` · `👁️ OBSERVATION`. Each row: date · model · signal · optional `→ OD-##` link. Models leave status `open`. Promote anything actionable to `WISHLIST.md` as an `OD-##` and link it back.
- **`WISHLIST.md` is the canonical task board** — do not create new TODO docs. Priority buckets: Critical/Urgent → Current → Next → Future → When I Have Staff. Dispatch syntax: "dispatch <ID> to <agent>" spawns an agent with the task context and updates status here.
- **Read recent `SIGNALS.md` rows before non-trivial work** in the area they touch — the file records load-bearing gotchas (stale vendored deps, live/deployed drift, the live-vs-intent identity topology) that source alone will not surface.

## Stale documentation

- **README.md is stale**: crate count (31) is wrong, `cd lovable` is a deleted directory, `sudo service6` references are retired. Prefer AGENTS.md and source over README claims.
- Many older deploy scripts and HANDOFF docs still reference s6 — treat them as archival; runit/`sv` is current.
- Anything mentioning `/dev/shm/plugin_schema.dat`, `live-schema.json`, or `/dev/shm/opdbus/schemas` is describing a retired design.
- Anything mentioning `OIB1`, `oib1:`, `blob_ref`, `SessionIdentityBlob`, or `x-opdbus-identity-blob-bin` is pre-2026-09-01 and stale; the session-identity envelope is now `SID1` / `SealedId`.
- Anything referencing `crates/op-web/ui/` as the active UI, or `operation-dashboard-ui-07` as the operator console — the active UI is `/srv/git/vercel-json-render-ui`, deployed via its `scripts/deploy-op-web.sh`.
- `deploy/btrfs-layout.sh` models a backup/restore subvolume hierarchy (base/modules/snapshots/staging + 5 nested module subvolumes). Btrfs is transport, not backup — see architecture invariants above.

## Subagent and model selection

Prefer the **least-expensive subagent that preserves quality** for builds and tests:

- `Explore` (fast read-only) for locating code, grepping symbols, answering "where is X."
- `Plan` for architecture/design before writing code.
- `general-purpose` only when the task needs multi-step reasoning or execution that Explore/Plan cannot cover.
- Reserve expensive model overrides (Opus-class) for tasks that genuinely require them — complex cross-crate reasoning, security review, or ambiguous architectural decisions. Standard edits, build fixes, and test work belong on the default agent.
- Do not spawn a subagent for a single-file edit or a direct grep you can do yourself.
