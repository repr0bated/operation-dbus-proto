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
