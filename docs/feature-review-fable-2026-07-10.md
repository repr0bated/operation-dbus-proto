# Feature Review — 3tched / OP-DBUS

**Assessor:** Claude (Fable 5), operating as Claude Code
**Date:** 2026-07-10
**Companion to:** `architecture-assessment-fable-2026-07-10.md`

The architecture assessment answered *"is the design sound?"* This document answers a
different question: **"what does the system actually do today, feature by feature, and in
what state is each feature?"** The architecture review graded principles; this review
grades capabilities. The two documents deliberately share a discipline: where I say
*verified*, I exercised the feature myself on the live host — most of it on this exact
date, during a session that took the machine from a half-dead post-reboot state to a
mostly-running one. Where I say *designed* or *stubbed*, I read the code and did not see
it run. That distinction is the entire value of this document, so it is kept sharp
throughout.

A note on the day this was written, because it colors everything: the host had just
rebooted into an orphaned kernel (no module tree → no WireGuard → no identity tunnel), and
much of the session was spent finding which failures were *features being broken* versus
*features being starved of their substrate*. That turned out to be an unusually good
feature audit: a system reveals its seams when you watch it come up wrong.

---

## Grades

| Grade | Meaning |
|---|---|
| ✅ **Verified** | Exercised end-to-end this session; works as described |
| 🟡 **Works, with caveats** | Exercised and functional, but with defects or fragility I observed directly |
| 🔧 **Built, currently broken** | Real implementation exists; observed failing; cause known or suspected |
| 🧪 **Prototype** | Functional but explicitly interim-grade (in-memory state, dev fallbacks, labeled prototypes) |
| 📐 **Designed, not built** | Contract/types/docs exist; execution path is stubbed or absent |

## The feature map

| # | Feature | Grade | One-line status |
|---|---|---|---|
| 1 | Plugin substrate & sealed blobs | ✅ | 65 built-in schemas, 68 blobs live in SHM; auto-reseal on boot observed |
| 2 | Discovery & self-description | ✅ | `zcall list/methods/expand` + gRPC reflection; works even with the control plane down |
| 3 | Identity provisioning & sled | 🔧 | Verified end-to-end pre-reboot; today the sled registry is empty and hydration produced zero |
| 4 | Zero-trust capability gate | 🟡 | Instant, machine-readable denies — the best behavior in the system; but the post-gate path hangs |
| 5 | Mutation chain & notarization | 🔧 | Built and load-bearing; the prime suspect in the capability-call hang |
| 6 | Durability & projections | 🟡 | Blob/projection rebuild on boot observed; identity rehydration did not produce the expected sleds |
| 7 | gRPC topology (bridges/gateways) | 🟡 | Every plane answers, but the port contract has fractured across three generations of design |
| 8 | Public registration (magic link) | 🧪 | Deployed and verified today, page to provision; state is in-memory, email success is over-reported |
| 9 | Privacy network (WG/xray/netmaker) | 🔧 | xray up and listening; every WireGuard-dependent piece down pending the kernel fix |
| 10 | Dashboard UI (React SPA) | 🟡 | 30+ pages served; self-labeled design prototype; dev-mode serving with a 404 deep-link nit |
| 11 | Generated UI (json-render catalog) | 📐 | DSL, interpreter, and store are real and strict; the catalog stream consumer is a no-op stub |
| 12 | Native GUI (egui/weston/VNC) | 🟡 | Running and viewable at `127.0.0.1:5901` after replacing a structurally impossible wayvnc pairing |
| 13 | Semantic memory (qdrant/voyage) | 🟡 | 11 code-vector collections recovered and serving; embedding generation dark (missing API key) |
| 14 | MCP surface | 🟡 | Compact server + web routes live; broad surface, thin verification |
| 15 | Chat & LLM routing | 🟡 | Gateway and routing plugin run; gemma_brain fully excised; end-to-end chat unverified today |
| 16 | Email (SMTP magic-link) | 🧪 | Real sender with graceful dev fallback; failures swallowed into `success: true` |
| 17 | NotebookLM integration | 🔧 | CLI/notebook side excellent; host sync services de-configured (missing envdir) |
| 18 | Compliance identifiers (OSCAL) | 🟡 | Versioned capability IDs pervade every method; artifact emission still absent |
| 19 | Operations (s6/boot/deploy) | 🔧 | Supervision is good; the boot *set* is the system's weakest feature, and it failed exactly as predicted |
| 20 | Incus container management | 🟡 | Provisioning verified pre-reboot; REST client hardened; one unrecoverable container corpse |

---

## 1. Plugin substrate & sealed blobs — ✅

The claim "the sealed blob *is* the plugin" is operationally true. At boot I watched
`op-dbus` load 65 built-in schemas, detect two plugins whose blobs were missing from the
SHM catalog (`identity_sled`, `routing`), and **auto-seal them back** with logged schema
hashes — the catalog self-heals toward the code's truth. The catalog at
`/dev/shm/opdbus/plugin-blobs/` holds content-addressed files
(`btrfs.563fe64ffab4a088.blob`, `cognitive_mcp.af0afc9538c20e32.blob`, …) and the running
bridge hydrated 68 entries from it.

One sharp observation the system's own law produces: **`gemma_brain` still appears in
`zcall list`.** The plugin's code was deleted weeks ago (384 lines, replaced by the
`routing` plugin), but its sealed blob persists in the catalog, so by the system's own
definition it still *exists*. "The blob is the plugin" cuts both ways: deleting code is
not deleting the plugin, and there is currently no blob-retirement tooling. The catalog
needs a deliberate *unminting* operation, or ghosts accumulate. This is not a bug in the
substrate — it is the substrate telling you a feature is missing.

## 2. Discovery & self-description — ✅

The strongest everyday feature in the system. `zcall list` enumerates the catalog,
`zcall methods <plugin>` returns each method with its read/write classification, its
capability id, and a versioned structured identifier
(`mut.service.identity-sled.container.provision@v1`,
`obs.service.doctor.query-history.read@v1`). Crucially, discovery reads the **blob
catalog on disk** (`--source blob`), so it worked this morning even while every gRPC
endpoint was down — self-description does not depend on the thing being described being
alive. That is exactly the right dependency direction and it made today's recovery
dramatically easier. Reflection over gRPC also works where a real bridge answers (the
deny responses in §4 arrived through reflected `PluginService/CallMethod` calls).

Caveat that belongs here rather than anywhere else: `zcall`'s *default endpoint*
(`127.0.0.1:18789`) now points at a port that no longer speaks gRPC (§7). Discovery is
flawless; the invocation default has rotted.

## 3. Identity provisioning & the sled — 🔧

The architecture session (documented in the companion review) verified the flagship path
end-to-end: one call to `identity_sled provision_container` derived a session id from a
WireGuard public key, created an Incus container *named by that id*, persisted the
record, and survived a control-plane restart. The UUID-named containers on this host
(`9431b9db…` RUNNING, `578a6e31…`, `4991a337…`) are that feature's physical residue —
identities you can `incus list`.

Today's state is honestly worse: post-reboot, the projection at
`/dev/shm/opdbus/projections/identity_sled.json` is `{"sleds":[]}` and boot hydration
logged zero records. Either the durable Cozo relations genuinely hold no sleds (the
verified provision was a test whose record went elsewhere), or the warm-load path did not
run. The method surface is rich and well-shaped — `provision_container`,
`write_identity`, `get_identity`, `touch_session`, `record_session_event`,
`get_session_history` (a per-session append-only "snowball" ledger), plus
`attach_btrfs_device` for the container-persistence model — but I could not exercise any
of it today because of §5. **Open question a future session must answer:** where did the
provisioned identity's durable record go, and why did rehydration produce nothing?

## 4. Zero-trust capability gate — 🟡

The single best *behavior* I observed all day. Calling any method without its capability
returns instantly with a structured, machine-readable denial:

```json
{ "error": { "code": "ERROR_CODE_PERMISSION_DENIED",
             "message": "AccessDenied: method identity_sled.get_identity requires capability identity_sled.read",
             "denyReason": { "capabilityMissing": { "capability": "identity_sled.read" } } } }
```

This is what security that intends to be *debugged* looks like: the deny names the exact
missing capability, so a client can render a precise error or request the right grant.
The zeroclaw ingress additionally stamps sentinel footprints and UUIDv7 trace ids on
requests missing identity headers — traceability survives even unauthenticated traffic.

The caveat is severe though: **passing the gate leads to a hang.** With a capability
attached, `CallMethod` for *any* plugin (I probed `identity_sled` and the innocuous
`doctor.get_query_history`) blocks past a two-minute timeout with no response and no
logged error. The gate is fast and articulate; the room behind the gate doesn't answer.
The system's best and worst behaviors live in the same corridor.

## 5. Mutation chain & notarization — 🔧

Every method call is notarized on an immutable event chain before its domain effect runs
(the identity dispatch module documents this contract explicitly, and the Shuttle binary
constructs the `EventChain` first thing in `main`). I did not observe the chain misbehave
directly — what I observed is the hang in §4, and the notarization/identity-resolution
path is the prime suspect: it is the one stage every capability-bearing call shares, it
performs synchronous Cozo work under `spawn_blocking`, and it plausibly needs identity
state that is currently empty (§3) and WireGuard key material that is currently absent
(§9). I am deliberately not asserting the cause; I am asserting the shape: **an
unstructured failure (indefinite hang) in the exact layer whose sibling produces the
system's most structured failure (the deny).** Whatever the fix is, the hang should
become a typed error — `failed_precondition: no active identity` would match the house
style. Retest is gated on the kernel reboot.

## 6. Durability & projections — 🟡

The "shared memory is a projection, never a copy" law held up under observation: the
projection directories were rebuilt at boot (timestamps match init), missing blobs were
re-sealed from code truth (§1), and the bridge rehydrated its reflection surface from the
catalog. The disk-side stores are real and busy — `/var/lib/op-dbus/cognitive.db` for
cognitive memory, sled-engine Cozo paths for identity relations, the event chain for
mutations. What keeps this at 🟡 rather than ✅ is the identity rehydration gap in §3:
the one projection that mattered most today came up empty, and the arch-session evidence
says it shouldn't have. Either the durable write went to an unexpected path or the warm
loader is selective in a way the docs don't state. The projection *mechanism* is
verified; the identity *contents* are not.

## 7. gRPC topology — 🟡, and the sharpest finding in this review

Every plane answers something, but the **endpoint contract has fractured across three
generations of design evolving in parallel:**

- `op-dbus` binds `10.200.0.1:50051` — the real bridge: reflection, capability gate,
  plugin dispatch. Verified serving (it produced every deny in §4).
- The **Shuttle** (`op-grpc-bridge`) defaults to `127.0.0.1:18789` — the documented
  "Xray redirect target" and `zcall`'s hardcoded default. It is marked *normally down*,
  and today I found it also **panicked at first TLS use** (rustls 0.23 built with both
  `aws-lc-rs` and `ring`, so no default CryptoProvider — fixed this session by installing
  one explicitly in `main`, commit `406c7399`).
- The **zeroclaw gateway** now owns `0.0.0.0:18789` *by explicit config* — it is an
  HTTP/WS dashboard, not a gRPC server, and answers grpcurl with `405 Method Not
  Allowed`.
- The deprecated `op-openvswitch-daemon` still squats `127.0.0.1:50051`, shadowing the
  real bridge's port on loopback.
- `op-cognitive-mcp` and `op-assistant-grpc` both default to `127.0.0.1:50052`; whichever
  starts second crash-loops. Neither port is set in `/etc/op-dbus/environment`.

Each service is individually fine. The *system of defaults* is incoherent: the same port
means three different things depending on which binary won the race, and the standard
client tool dials a port that stopped speaking its protocol. The fix is boring and
important: one authoritative port table in `/etc/op-dbus/environment`, every run script
reading it, `ZCALL_ENDPOINT` set accordingly, the deprecated daemon stopped and its crate
removed, and the Shuttle either retired or re-homed. Until then, every new feature
inherits this ambiguity.

## 8. Public registration via magic link — 🧪, deployed today

The newest feature, completed and deployed this session. The pipeline is genuinely
end-to-end: a public `/register` page (React, rendered outside the admin shell, no
dashboard chrome, no admin SSE stream) → `POST /api/privacy/signup` → 60-second
server-side resend rate limit → token minted → email dispatch → `GET
/privacy/verify?token=` → **verify and provision in one redirect flow**
(`provision_verified_signup`: user → identity sled → memory → container) → human
confirmation page. A Google OAuth path exists in parallel. Verified today: signup returns
200, the user record is created, the link is minted, and with SMTP unconfigured the
system prints the link to the log — a genuinely good dev-mode fallback.

What is deliberately honest about the design: the consumer path derives the session id
from the PSK (`derive_session_id_from_psk`, `session_proof`) — meaning the *public
signup* funnels into the exact same identity primitive as the substrate's
container-provisioning. The collapse principle reaches all the way to the marketing
surface. That is the strongest possible sign the architecture is being *followed*, not
just admired.

Why it stays 🧪, in order of consequence:

1. **All account state is in-memory** (`RwLock<HashMap>` for users and magic links). I
   verified the failure mode concretely: restarting op-web invalidated an outstanding
   link. The project has already decided the fix (Cozo persistence — sessions here are
   account-lifetime identity, not disposable web sessions); it is not yet implemented.
2. **Email failure is invisible to the user.** The send failed (SMTP host unreachable —
   §9) and the API still returned `success: true, "Check your email"`. The error exists
   only in the server log. A user who never receives the mail has been lied to. Return
   the failure, or adopt queued-with-retry semantics and say "queued."
3. The rate limiter is a static in-process map, self-labeled prototype in a code comment
   — fine for now, honest about itself.
4. Found and fixed while deploying: op-web's run script sourced
   `/etc/op-dbus/environment` **without `set -a`**, so `SMTP_*`/`BASE_URL` were shell
   variables, never environment variables — the process ran with zero email config since
   the day it was written. Patched live; magic links now carry
   `https://registration.3tched.com` as their base.

## 9. Privacy network — 🔧 (starved, not broken)

The feature set here is broad: WireGuard client-config generation with QR codes,
per-peer PSK derivation (Argon2 keyed with the peer's WG pubkey as salt — reviewed in the
PR #15 triage and deliberately shipped as the correct formula), the `opdbus` hypervisor
identity tunnel to an Oracle VPS, a decoy ingress tunnel, a netmaker mesh (netclient both
as host service and inside the `netmaker-pro` container), xray with a Reality inbound on
8443 and TLS on 443 wired to NextDNS, and hub-alias plumbing that binds the mesh
addresses. xray is verified up and listening on both ports right now.

Everything WireGuard-shaped is down for one reason that is not this feature's fault: the
running kernel has **no module tree at all** (the boot partition held an orphaned 7.0.12
image while pacman installed 7.1.2 into a shadowed directory — found, verified, and
staged for the next reboot this session). `wg-netmaker` crash-loops with "WireGuard not
detected," the tunnels can't exist, and the SMTP relay for §8 sits on the far side of
those tunnels (`10.149.181.121:587`, unreachable). Two latent defects to retest once the
substrate returns: the netmaker `get_node()` async-in-sync-trait issue from the BigPickle
registry (the workspace compiles, so it is either fixed or dormant — unverified either
way), and the OVS `list_ports` discrepancy whose known-buggy parser lives in the
deprecated daemon and should die with it rather than be fixed.

## 10. Dashboard UI (React SPA) — 🟡

Thirty-one routed pages served by op-web from a Vite build — Overview, Chat, Gallery,
Catalog, Tools, Agents, Models, Services, Security, Config, Inspector, State, Logs,
Workflows, Orchestration, Skills, Containers, PrivacyNetwork, OVS, OpenFlow, Knowledge,
GrpcDiagnostics, GrpcExplorer, Accountability, Btrfs, DataStores, Embedding, Assistant,
and now Register. The codebase opens with an unusually honest banner: *these routes are
design-only prototypes for the native egui port; do not wire real inference, auth, or
mutable state through them.* The registration page added today is the first deliberate
exception — a real production surface, kept outside the shell.

Serving is explicitly dev-mode (`ServeDir` from `OP_WEB_STATIC_DIR`; the rust-embed
production path is scaffolded but "a later step"), with one cosmetic defect verified
today: deep links (`/register`) return the SPA with a **404 status** — browsers render
fine, but the fallback should rewrite to 200. One operational hazard worth stating in a
feature review because it *will* eat someone's afternoon: `update-ui.sh` rsyncs the React
source over `crates/op-web/ui/src` with `--delete` — and that directory currently holds
the *Rust egui GUI's* sources. Deployment today was done by copying `dist/` only. The
script and the tree have diverged; reconcile them before the script fires blind.

## 11. Generated UI: the json-render catalog — 📐, one stream from alive

The most architecturally distinctive feature, and the clearest case of
"designed-not-built" done *well*. What exists and is strict: the DSL types (an `Element`
is an immutable, versioned UI atom; ids are minted once and never reused), the
interpreter (a deliberately minimal primitive set; unknown nodes are a hard
`RenderError`; **no per-element Rust, no WASM, no raw-JSON fallback** — extension is a
deliberate act of adding a match arm and documenting the contract), and the store
(versioned, immutable, stable-core protection, last-known-good cache). The single-writer
model is consistent with the rest of the system: UI-gen writes, everyone else reads.

What does not exist: the connection. The `CatalogService/Subscribe` stream consumer is a
**no-op stub** — the code says so plainly ("wired as a no-op stub so the rest of the
system compiles while UI-gen's proto lands"). Meanwhile the producer side
(`zeroclaw-routing-uigen`, a Grok-backed proposal agent) has been running as a service
all day, presumably proposing into the void. The flagship demo — an LLM minting UI
elements into a governed catalog that a locked-down interpreter renders live — is one
protobuf and one stream handler away from being demonstrable. Nothing else in the
roadmap buys as much visible capability per line of code.

## 12. Native GUI — 🟡

The egui `zeroclaw-gui` runs under two harnesses simultaneously: an Xvfb/X11 service and
a Wayland service p