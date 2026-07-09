# Handoff — identity_sled + netmaker-pro + EMQX ExHook (2026-07-09)

Host: `3tched` (Artix, s6-rc). Branch: `claude/refine-local-plan-0xcmyp`. Nothing committed.

## TL;DR of where we are

1. **netmaker-pro recovered and running** in incus (was on disk, absent from DB).
2. **EMQX ExHook ported v1→v2** and built/deployed; container-side wiring to the host bridge is staged but NOT connected the right way yet (must go through the plugin socket surface, not a raw incus proxy).
3. **identity_sled plugin built and deployed** (schema + dispatch), but it is **blocked on an identity-architecture correction** (below) — the sled is still not being written because the identity source was pointed at the wrong place.

## ⚠️ Load-bearing correction from the user (must re-architect before finishing)

**This box has NO WireGuard interface for identity.** I was wrong to point `get_local_pubkey` / `WG_INTERFACE` at the local `netmaker` interface.

- The WG **server/hub for identity lives on the decoy VPS** (`ubuntu@129.153.134.63`, wg0 `:51821`, hub pubkey `6mx4ycJeDMEDUknDY+sVlus1PQOEGG9/XrGFBuB1GFY=`). This box (3tched) appears there as peer `iNqgBk7pBC/iXUmF+v4PNvvdAxjP5qWjxzMYYRE8+hw=` / `10.0.0.2`.
- The real flow: **session is forwarded from the VPS to 3tched's xray, and xray injects the identity** (verified from the single hop off the VPS). Identity is NOT read from a local `wg show`.
- Open design idea the user floated: **the VPS could send a request and xray injects identity on the decoy server itself** (where the WG interface actually is), rather than on 3tched.

**Consequence for the code I wrote:** `WireGuardIdentity::get_local_pubkey()` shelling to `wg show <iface> public-key` is correct as a *generic* helper but **wrong as the identity source for this host**. The identity sled for "container zero" must be seeded from the **xray-injected identity header off the one VPS hop**, not from a local WG interface (there is none). The `WG_PUBKEY` env override I set on `op-cognitive-mcp` (to the hub peer key) is a stopgap, not the design — revert/replace it with the xray-injection path.

## What was actually changed (files)

Rust (built clean, release, deployed to `/usr/local/bin`):
- `crates/op-identity/src/wireguard.rs` — `get_local_pubkey()` no longer a stub (shells `wg show <iface> public-key`, env override kept). **Keep the helper, but it is not 3tched's identity source — see correction.**
- `crates/op-plugins/src/state_plugins/identity_sled.rs` (NEW) — the plugin. `ContainerIdentitySled` (session_id = container name = derived from WG pubkey, host = container zero), `blob_ref` (sealed blob that seeds the container), `SledBtrfsDevice` (persistence = Cozo-registered btrfs device, `btrfs device add`, no layers), snowball `SessionEvent` ledger. Methods: get_identity / write_identity / touch_session / record_session_event / get_session_history. Registered in `mod.rs` + `plugin_scaffold_helpers.rs`.
- `crates/op-grpc-bridge/src/identity_sled_dispatch.rs` (NEW) + arm in `mutation_engine.rs` `dispatch_method_call`. write_identity derives session_id (never trusts a supplied one); refreshes the legacy raw sled at `/dev/shm/plugin_schema.dat` only for container-zero (AnnaScribe compat).
- `crates/op-grpc-bridge/proto/emqx_exhook_v2.proto` (NEW, copied from container's emqx_exhook-5.0.21) + `crates/op-grpc-bridge/src/emqx_hook_provider.rs` (NEW, v2 HookProvider, audit tap → MutationEngine, auth/authz/publish = IGNORE) + `build.rs`/`lib.rs`/`grpc_server.rs` wiring. Mounted WITHOUT the ghostbridge interceptor (EMQX carries no identity headers).
- `deploy/s6/gemma/up` — dropped `foreground{}` wrapper so `shell_up`'s real exit code reaches s6 (was silently reporting success).

Deployed + restarted: `op-cognitive-mcp`, `op-grpc-bridge-zeroclaw`. **Still to build/deploy: `op-web` (the `opdbus` bin = the 10.200.0.1:50051 gRPC server that carries the identity_sled dispatch)** — build was in flight at handoff (`cargo build --release -p op-web`).

Env changes made on the host (RECONSIDER per correction):
- `/etc/s6/sv/op-cognitive-mcp/env/WG_INTERFACE` = `netmaker` (was `wg0`) — **wrong per correction; there is no local WG identity iface**.
- `/etc/s6/sv/op-cognitive-mcp/env/WG_PUBKEY` = 3tched's hub peer key — stopgap.

## netmaker-pro recovery (done, durable)

- Rootfs + backup.yaml were intact on pool `default`; absent from incus DB. `incus admin recover` was blocked by (a) stale duplicate assistant/cozo/mail-3tched/qdrant dirs on pool `default` — live volumes live on `btrfs-pool`; quarantined at `/var/lib/incus/storage-pools/default/quarantine-stale/`; and (b) an orphan `test-launch` `storage_volumes` row (deleted id=63 via `incus admin sql global`). Recovery also re-imported junk test2/test3/test-launch/ztest (deletable).
- **Trap:** `incus list -c nsP` P column is PROFILES, not pool. Don't move live volumes based on it.
- netmaker-pro RUNNING: from-source netmaker v1.6 + EMQX 5.8.9 + netclient/nmctl/caddy; API healthy on :8081 inside the container. `netmaker.service` hard-`Requires=emqx.service` (exists; started manually). `netmaker-first-boot.service` (network create + enrollment key) NOT yet run.

## EMQX ExHook wiring — the RIGHT way (task still open)

- Container `/etc/emqx/emqx.conf` has an `exhook` block pointing at `http://127.0.0.1:9000` (staged).
- I first added a raw `incus config device add ... proxy` — **user rejected: NEVER raw incus proxy devices.** Removed.
- Correct path: register the container→host egress through the **plugin socket surface**: `zcall shared_unix_socket create_unix_socket --arguments '{"name":"<sessionid>","ports":[...]}'` (shared socket `/run/opdbus/container.socket`). The host bridge (op-grpc-bridge-zeroclaw) serves `emqx.exhook.v2.HookProvider` un-intercepted. Still need to finish this and confirm hooks fire.

## Immediate next steps

1. **Re-architect identity source** per the correction: seed container-zero's identity_sled from the xray-injected header (one VPS hop), not a local `wg show`. Decide VPS-side vs 3tched-side xray injection. Undo the `WG_INTERFACE`/`WG_PUBKEY` env stopgaps on op-cognitive-mcp once done.
2. Finish `op-web`/`opdbus` build + deploy (identity_sled dispatch lives in the 50051 server).
3. Verify: `zcall identity_sled write_identity ...` → row in state; then `zcall shared_unix_socket get_config` should stop returning `A.N.N.A. Scribe: Missing Ghostbridge Identity Sled`.
4. Wire EMQX exhook egress via `shared_unix_socket create_unix_socket`; run netmaker-first-boot; expose API 8081 / broker 8083 via the same socket surface for xray.
5. **Gemma → Grok (xAI)** for routing + UI generation (user). Drop the "MindStudio" label — that was just a Claude-written agent wrapper missing a Grok key, not a product dependency. zeroclaw has native provider `xai` (alias grok). Key may now be present on host; **not wired into `op-gemma` yet** (still local derive + empty UI gallery).

## Still-open from the original plan (`~/.claude/plans/sleepy-noodling-torvalds.md`)

- Encrypt identity-bearing blobs (chacha20poly1305 + Argon2(PSK,salt=pubkey) → blake3 domain-sep) in op-blob.
- AnnaScribe off the raw `/dev/shm/plugin_schema.dat` mmap → read the sealed (encrypted) blob.
- Cozo persistence wiring: `sessions` / `session_memories` relations already added to op-cozo-store; dispatch currently keeps events in the state cache (capped 256) — wire the durable Cozo store.
- Provisioning-script unification: one WG keypair per container (not container-key + separate opblob "account" key); btrfs `device_add` is a stub (OD-32).

SIGNALS.md rows for this session: OD-33 (netmaker/exhook), OD-34 (identity_sled/get_local_pubkey/gemma up).
