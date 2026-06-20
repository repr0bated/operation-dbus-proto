# 📡 Ghostbridge Live! — Model Signals

**Any model working on this project is encouraged to append here.** Suggestions, concerns,
observations — don't let them evaporate into chat. This is the place to surface what you notice.

## How to post
Append a row under the right type. Keep it one entry per signal.
- **Type:** `💡 SUGGESTION` · `⚠️ CONCERN` · `👁️ OBSERVATION`
- **Fill:** date · model (e.g. `Opus 4.8`, `Gemma`, subagent name) · the signal · optional `→ OD-##` link to a task it relates to.
- **Status:** `open` → `ack` (human saw it) → `actioned` / `wontfix`. Humans set this; models leave it `open`.
- Promote anything worth doing into `WISHLIST.md` as an `OD-##` and link it back.

---

## ⚠️ Concerns

| Date | Model | Concern | Link | Status |
|------|-------|---------|------|--------|
| 2026-06-13 | Opus 4.8 | NotebookLM static cookies expire in minutes (no live browser to refresh `SIDCC`/`__Secure-1PSIDTS`); any long batch dies. Notebook ops should run laptop-side or in short bursts. | OD-20 | open |
| 2026-06-13 | Opus 4.8 | `opdbus.v1` is unowned — `op-xray-daemon` has the path fix compiled but **no s6 service**, so nothing registers. This blocks registration, accountability loop, and Gemma routing all at once. | GBL/OD-05 | open |
| 2026-06-13 | Opus 4.8 | Gemma (ollama :11434) and Qdrant (:6333) are both **down**. Vectorization pipeline has no sink; routing brain has no engine. Bring-up can't be tested until these are up. | OD-06/OD-23 | open |
| 2026-06-13 | Opus 4.8 | Live xray runs off **static** `/etc/xray/config.json` and sources missing `/etc/ghostbridge/xray.env`. Dynamic generation is coded but never wired at runtime. | OD-05 | open |
| 2026-06-13 | Opus 4.8 | **provision-workspace-subscriber.sh writes registration via cognitive-mcp over HTTP `http://100.90.37.254:3003/mcp` + Bearer token** — the SSE/HTTP path we're replacing with gRPC+reflection. Signup registration still rides the old transport. | OD-10/OD-24 | open |
| 2026-06-13 | Opus 4.8 | **GhostBridge Netmaker peer registration is a TODO stub** (provision script lines 92-93) — `--ghostbridge` containers never actually register their WG peer. Registration chain is broken at the network layer. | OD-08/OD-24 | open |
| 2026-06-13 | Opus 4.8 | **Two identity stores that may not reconcile:** provisioning writes pubkey/token to **CozoDB** (`cognitive_memory` namespaces); runtime accountability reads the **sled** (`/dev/shm`) + Qdrant trace_id. Need to confirm signup also etches the sled, or the registration→accountability link is severed. | OD-24 | open |
| 2026-06-13 | Opus 4.8 | **Two auth models:** provisioning uses Bearer `UUID v5(pubkey)` token; runtime interceptor checks ghostbridge header *presence* + sled footprint, not the token. These need reconciling or a container can be "registered" yet fail every runtime gate. | OD-24 | open |

## 💡 Suggestions

| Date | Model | Suggestion | Link | Status |
|------|-------|-----------|------|--------|
| 2026-06-13 | Opus 4.8 | Graph store for the learning graph: start with Qdrant payload edges (one store, runs now), promote to a dedicated graph DB only when you need real traversal. | OD-23 | open |
| 2026-06-13 | Opus 4.8 | Fire the vectorization export on a **git hook** for `WISHLIST.md`/`SIGNALS.md` changes rather than a daily cron — event-driven matches the rest of the system. | OD-23 | open |
| 2026-06-13 | Opus 4.8 | A.N.N.A. = `authorizing-official` + `content-approver` (real-time check/approve), not a one-time gateway notary. Re-map the analogy so she IS the interceptor at every door. | OD-09 | open |

## ✔️ Resolved Decisions

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-13 | **Key derivation:** `derived_key = Argon2(secret = PSK, salt = WG_pubkey)`. PSK rides WireGuard's built-in `PresharedKey` slot. DB stores: WG **public** key (canonical identity anchor), MCP token = `UUID v5(pubkey)`, and `blake3(psk)` **only** (never raw PSK). WG **private** key + raw PSK stay user-side only. MAC is **provision-time hardware binding only** (hashed), never used at runtime — it's L2 and invisible over the L3 WG tunnel. Email stored only if client flagged (GhostBridge off). | Salt must be server-observable over WG at runtime → pubkey (handshake-proven, unique, stable). Secret must not be derivable from public data → PSK. IP-octet salt rejected (first octets shared = no salt; last/full /32 = IP-stability fragility). MAC rejected as runtime salt (not exposed over WG). |
| 2026-06-13 | **session_id = the derived key** = `Argon2(PSK, salt=pubkey)` — **stable / persistent**, NOT per-visit. Server verifies by storing `blake3(session_id)` at provision and checking `blake3(presented)==stored` (server lacks raw PSK so can't recompute). Per-activity differentiation is delegated: **subid taxonomy** = what/which within the session; **trace_id (UUID v4, sled)** = per-event unique → Qdrant. | Persistent sessions are desirable; reproducible identity anchor across reconnects. BLAKE is deterministic (same input=same hash) — old freshness came only from a timestamp; that role now belongs to subid+trace_id, the layer built for it. No nonce needed. |
| 2026-06-13 | **The timestamp lives on the SLED, not the session_id.** Sleds are **datestamped**; the **current sled = the one with the latest datestamp**. The interceptor's "Temporal Hash Mismatch" check validates a request's footprint against the *current* (latest-datestamp) sled — a footprint from an older sled fails. So temporal currency = sled datestamp; identity = stable session_id. | Puts the temporal anchor where mutations actually happen (the sled / schema state), not on the identity token. Answers "is this tonight's wristband?" by sled datestamp. |

## 👁️ Observations

| Date | Model | Observation | Link | Status |
|------|-------|-------------|------|--------|
| 2026-06-13 | Opus 4.8 | The notebook "overlap" the user keeps hitting is structural: ~10 notebooks are re-export snapshots of the same evolving project. The board + manifest + this log are the fix — one writable source of truth instead of N read-only snapshots. | OD-20 | open |
| 2026-06-13 | Opus 4.8 | "Blacklight = Blake" is a genuinely strong mnemonic for the sled footprint check — worth leading the Inception §4 with it. | OD-21 | open |
| 2026-06-19 | Opus 4.8 | Context-aware coding (code_search/code_context) is live and serving, but the `repomix_rag` index looks stale/coarse: a query for "ghostbridge interceptor authentication" top-ranked an unrelated WG tunnel run-script (score 0.176) instead of the actual `crates/op-cognitive-mcp/src/interceptor.rs`. The index appears to be old repomix snapshots, not the current working tree. Re-ingest current sources to make retrieval useful for live coding. | OD-22 | open |
| 2026-06-19 | Opus 4.8 | **Domain forwarding restored by migrating xray off the `wg-xray` container onto the host** (see `xray-router` memory). xray now binds `*:443` directly and routes by SNI; `qdrant.* → 200` verified end-to-end; service is boot-persistent (`s6 set enable xray` + default bundle). BUT most subdomains still return HTTP 000 because their **backends are down**, not the forwarder: `op-web :8080` (the default route) — nothing listening; `assistant :18789` — nothing; `mail :10143` — nothing; and `api/dashboard/broker` (:28081/28082/21883) accept on the host forkproxy but reset because the **`netmaker` app container is STOPPED**. Next: bring up op-web (8080) + the netmaker app so those subdomains actually serve. | OD-23 | resolved (mostly) |
| 2026-06-19 | Opus 4.8 | **op-web + SNI-dispatcher split done.** Enabled `op-web-srv`/`op-web-log` (boot-persistent) → default/op-web/chat → 200. Resolved an h2-vs-http/1.1 ALPN conflict (qdrant gRPC needs h2; netmaker web UIs are http/1.1-only) with a `dokodemo-door` SNI dispatcher on :443 → `tls-h2:8444` (qdrant) / `tls-h1:8445` (web). Verified: qdrant→200(h2), dashboard→200(h1, was 000), op-web/chat→200. **Two backends still down (service health, not forwarding):** `assistant :18789` (ghostbridge gRPC bridge not running) and the **`netmaker` API container crash-loops** on LXC mount error `Failed to mount "none" onto "/usr/lib/lxc/rootfs/run"` (netmaker-ui + netmaker-mq stay up). Next: start the assistant/ghostbridge gRPC bridge; debug the netmaker container's `/run` mount. | OD-24 | open |
| 2026-06-19 | Opus 4.8 | **Adopted schemars-derived plugin schemas as the standard** (`crates/op-plugins/src/state_plugins/schemars_adapter.rs` + `docs/schema-from-structs.md`). Plugins define config as `#[derive(schemars::JsonSchema)]` structs and derive the `PluginSchema` instead of hand-building `FieldSchema` maps; the adapter walks the schema JSON (decoupled from schemars' typed API, no `zeroclaw-config` coupling). `unix_socket` flipped live with a `#[cfg(test)]` golden-reference equivalence test. **Scope reality (non-obvious — NOT a pending bulk-convert TODO):** of 47 schema plugins, 42 have a candidate struct but most are **opaque** (`Value` fields like `cron.jobs: Value`) while the hand-rolled schema spells out the detail — so converting = fully typing the struct first (runtime-touching), not find-and-replace; 5 have no struct at all (`oscal_subid_registry`, `lxc`, `procfs`, +2). Decision: schemars for all NEW plugins; migrate existing ones deliberately/per-plugin when typing the struct is independently worthwhile. | OD-25 | resolved |
| 2026-06-19 | Droid / factory-droid | schemars-derived plugin schemas with OSCAL subids are now the standard across the converted plugin set. The recursive `schema_diffs` equivalence test and `all_subids_are_valid` test guard every converted plugin. Remaining unconverted plugins still use their original hand-rolled schemas and should be migrated deliberately when typing their structs is independently worthwhile. | OD-25 | resolved |
