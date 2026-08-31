# Kiro spec prompt — Session Genesis Identity

**Run the full Kiro spec workflow for this feature: requirements.md → design.md → tasks.md, in that order, with approval between each. Do not write implementation code from this prompt.**

Feature name: `session-genesis-identity`

---

## 0 · Read before writing requirements

1. `/srv/git/odbus/CLAUDE.md` — architecture invariants. **Note one stale claim, corrected below.**
2. `.kiro/specs/netmaker-xray-identity-handoff/` — the locked identity spec. This feature **extends** it and must not contradict it. It establishes: WG terminates at the Oracle decoy only, the decoy issues a short-lived Ed25519-signed assertion carried as gRPC metadata, and op-grpc-bridge is the sole validator.
3. `.kiro/specs/3tched-ghostbridge-control-plane/` — topology lock (no host wg-lan, no SNI front on public :443, gRPC at 10.0.0.2:8090 mesh-private).
4. `.kiro/specs/README.md` — spec index and topology lock summary.

---

## 1 · The problem, in one paragraph

The identity value carried in the per-request header is **recomputed on every mutation**, so it is stale before the client can present it. A.N.N.A. Scribe therefore rejects live callers with "Temporal Hash Mismatch". Two unrelated things are both called "footprint" — the identity anchor and the blockchain record — and the collision is why neither works: the identity anchor chases a moving global counter, and the chain record carries no session identity at all, so the chain cannot be sliced or verified per session.

## 2 · Verified live evidence (verified 2026-08-15/16 against the running host, not inferred from docs)

| Fact | Evidence |
|---|---|
| The SHM sled is ONE file, 152 bytes, last-write-wins, shared by all sessions | /dev/shm/plugin_schema.dat; SHM_SLED_PATH at op-identity/src/schema_bridge.rs:22 |
| Its mutation_index is a global counter near 1.9M and moves constantly | live sled dump; blockchain counter 1,822,084 |
| The identity anchor is derived from that counter | etch_footprint, op-identity/src/schema_bridge.rs:1121 — blake3(pubkey ‖ catalog_hash ‖ mutation_index ‖ source_port) |
| source_port is structurally zero on this host | no wg0; interfaces are wgcf-egress, wgcf-uiStream (WARP), netmaker |
| The per-session store disagrees with SHM | /dev/shm/opdbus/state/identity_sled.json — record for live container bea37ecb-92be-197c-660f-09e806f1a34f has hashed_footprint "" and mutation_index 0, while SHM holds a real footprint for the same pubkey |
| The gate reads the per-session store, not SHM | verify_per_identity → dispatch get_identity, op-grpc-bridge/src/identity_sled_dispatch.rs:299-328 |
| The chain record carries no session identity | event_to_footprint, op-grpc-bridge/src/mutation_engine.rs:2358 — actor_id, capability_id, method_name, event_id, event_hash, decision, replay copy. No pubkey, no session_id, no genesis, no catalog hash. vector_features hardcoded vec![] |
| The chain head is available O(1) | EventChain.last_hash(), op-state-store/src/event_chain.rs:513; chain self-verifies by prev_hash linkage at :679 |
| Both mutation paths already funnel through one engine | mutation_engine.rs:1343 (property-set) and :1471 (method call), alongside persist_audit_event at :488 |
| A second, non-redirectable sled reader exists | anna_scribe.rs:50 hardcodes File::open("/dev/shm/plugin_schema.dat") instead of read_sled(); it is also the one failing test (test_notarize_arrival_rejects_missing_schema) |
| An absent identity becomes a zeros identity | tracing.rs:19,38 — SENTINEL_FOOTPRINT of 64 zeros stamped with a warning; present in the live log at 20:35 and 21:03 on 2026-08-15 |
| The UDS injector stamps the global record for every caller | shared_socket.rs:137-176, CanonicalPeerIdentity::from_sled — overwrite-not-trust is correct, but the value is whichever session mutated last |
| No container has a NIC — including xray | `incus config show xray --expanded` lists five disk devices and no nic; ovsbr0 has only eth0, ovsbr0, 3tched, svc0, pub0 — no container ports |
| **CLAUDE.md is stale here** | It says xray is the sole container with a real NIC bridged onto ovsbr0. Not true as of 2026-08-16. Public traffic reaches xray via host-side forwarders into a UDS |
| The edge is unversioned Python byte relays | sni-demux.py on 188.68.58.237:443 and 10.0.0.2:443, socket-relay processes, nm-api-tls.py, nm-warp-egress-proxy.py — all from /usr/local/libexec/3tched, no source in the repo; deploy/runit/libexec-3tched/tls-relay.py is untracked in the working tree |
| **Seven writer call sites across five processes write the same 152-byte file, last-write-wins** | op-cognitive-mcp/src/main.rs:66; op-mcp/src/main.rs:113; op-mcp/src/compact.rs:585; op-grpc-bridge/src/identity_sled_dispatch.rs:465; op-grpc-bridge/src/mutation_engine.rs:514; op-identity/src/bin/op-identity-sled.rs:52; op-identity/src/schema_bridge.rs:1381 (ip-monitor handshake thread). Five are write_sled_from_wg |
| Two uncommitted patches are compensating for the churn | schema_bridge.rs (pin the footprint once) and interceptor.rs (mismatch → None instead of permission_denied). Built at 21:23, never installed. The running /usr/local/bin/op-grpc-bridge is the 19:52 binary |

## 3 · The design that was converged on (turn this into requirements, do not re-litigate it)

**Split the two meanings.**

- **Genesis** — the session anchor. Minted **once**, at login, and never recomputed:

      genesis = blake3( wg_pubkey ‖ chain_head_hash ‖ head_timestamp ‖ catalog_hash ‖ arrival_timestamp )

  Identity, state-at-arrival, contract-at-arrival, moment. The chain head is a commitment to all prior mutations by hash linkage, so it *is* the current mutation state without walking anything. The arrival timestamp is the uniqueness term that separates two logins landing at the same head; unlike the head timestamp it cannot be re-derived, so it must be **stored** to stay checkable.

- **Footprint** — goes back to meaning the blockchain record (PluginFootprint, op-blockchain/src/footprint.rs:54). One per mutation, delivered by the session, carrying the genesis as its session stamp, free to grow (including vector_features, currently always empty).

**The header is on every packet.** Constant for the life of the session, verified with a single equality against the session record. This is the point of the whole design, not an implementation detail. When it is absent the request must **fail closed** — the zeros sentinel is the same as having no gate.

**The mutation engine owns all of it.** Arrival is not a special case; it is mutation one of the session. The engine reads the chain head, mints the genesis, seals the identity blob, writes the session record, and thereafter stamps the genesis onto every footprint that session delivers. No circularity: genesis binds the head *before* the arrival, the arrival records the genesis after.

**Identity reaches xray out-of-band as a sealed blob, not in-band.** The xray in the container is stock Xray-core with Reality and is a passthrough — it cannot inject HTTP headers, and arbitrary preamble bytes on its inbound socket would land where it expects a ClientHello. So the component that first sees the key seals an identity blob into the catalog and splices the stream untouched; the bridge's UDS injector joins on it and stamps the header after termination.

**Both sled writers, in parallel, with field ownership.** The mutation path owns the exact chain position. The stream feeds per-session accumulation — the span and the tokio/metadata context. They supplement rather than overwrite because they no longer write the same field of the same shared record.

## 4 · The user's own framing (verbatim — do not paraphrase these into something else)

> "we had a discussion and we figured out that if the sled isnt fred by steram it is only oupdating the last mugtatation, that the sled with the mutation stream and the rest witing the metadate a and stokio that produces full"

The sled fed only by the mutation-write path holds the last mutation, not an account. The mutation stream plus the metadata/tokio context is what produces the complete one.

> "thats the sleds role to deliver a compolet account of the session"

> "i dont think we should staert out with the footpring we should start out with hash from a hash of the current mutation state upon login"

> "with the mutationstat timestamp anhd wg key should be verifiable"

> "now ther eis the footpeint tha t was neant to be the blcokchan freed to be that"

> "the pint of the whole thing was every packet had the header"

> "it can write a blob packet payload"

> "so the mutaiton engine"

## 5 · Invariants the spec must respect

- D-Bus is the only control plane. New capability = a plugin in op-plugins/src/default_registry.rs, **never** a new gRPC proto service package.
- PluginSchema is the single source of truth; any derived value is computed in exactly one function, one place.
- The sealed blob IS the record. Sole writer is the blob sealer in op-blob. Consumers read SHM directly and never re-hash.
- Reactive, not polled. No watchers, no polling loops, no `Command::new` subprocesses. The rejected TransportBindingIndex design (a source-IP table populated by a handshake watcher) is explicitly **not** an acceptable binding — see netmaker-xray-identity-handoff §1.3.
- Durability stays inline on the dispatch path (NFR-4). If genesis delivery moves behind the stream, that guarantee is lost — the recommended shape is: the session record assembles and stamps on the same thread the mutation is on, and the stream feeds only what tolerates a gap.
- The Ed25519 oracle assertion (x-oracle-identity-assertion-bin, interceptor.rs:23) remains the cryptographic binding of the pubkey to an authenticated peer. **Genesis does not replace it** — genesis binds that identity to state and moment. The spec must say exactly how the two compose.
- Rust-first; no new Python.
- Every new object/mutation/event/tool carries a uuid and a subid from the seven-category taxonomy, registered in op-plugins/src/state_plugins/oscal_subid_registry.rs.
- Host runs runit (`sv`), not s6.

## 6 · Open questions the spec must resolve (each changes the design)

1. **Join key** — what ties the sealed identity blob to the connection xray hands over. Candidates: source address+port (needs PROXY v2 to survive termination, otherwise lost at the UDS hop); SNI (visible to sni-demux before any decryption, survives to xray, but **the topology lock says no SNI front on public :443** — reconcile or reject); one UDS connection per session (needs nothing from xray, costs a socket per session).
2. **Where the mint happens** — Oracle decoy (it is the only place the human WG peer is authenticated), xray, or the bridge. The locked spec makes the bridge the sole validator; that constrains but does not by itself answer where the *mint* lives.
3. **State term** — chain head hash only, or additionally a root over the sealed blob catalog contents at login. Head proves "these mutations happened" and is free; a blob root proves "the world looked like this" and costs a hash per login. Recommendation: start with the head, add the blob root as a second term if the stronger claim is wanted.
4. **Header naming and migration** — x-ghostbridge-footprint now carries a genesis, which re-creates the two-meanings collision on the wire. Adding x-ghostbridge-genesis and accepting both during transition avoids a flag day across xray config, op-web handlers, the grpc_web allow-list (grpc_web.rs:31-33) and the UI.
5. **schema_version** — IdentitySled is at SLED_SCHEMA_VERSION 2 with the genesis change pending. Decide whether the new record shape is version 3 and how old records are distinguished.
5a. **Is the session record schema-defined or hand-maintained?** IdentitySled is a hand-written 152-byte `#[repr(C)]` struct with a manually bumped schema_version, while every other record in the system derives its shape from PluginSchema. The point of this feature is that identity becomes schema/code like everything else — bound to the catalog hash *and* defined by a schema, with a subid, a D-Bus surface, and schema-driven rendering for free. If the struct stays hand-maintained, identity is schema-bound but not schema-defined, and the struct and the schema will drift exactly the way the two "footprints" did. **Direction is decided: schema-derived.** The schema stream carries the definition and the fixed SHM layout is *generated* from it — not hand-written, and not a schema mirroring a hand-written struct. Both properties are required simultaneously: the gate needs a fixed-layout zero-copy read to do one equality per packet (dynamic JSON at the gate is not acceptable), and the definition must live in exactly one place. Generation satisfies both. Because every stream frame carries the catalog hash, a shape change is announced rather than discovered when two components disagree — which is exactly how the two "footprints" drifted. The spec must name the single function that owns the generation, and say what happens to a live session whose record shape changes mid-session.
6. **Fail-closed rollout** — removing the zeros sentinel will reject anything not yet stamping the header. Sequence it so the edge emits the header before the gate starts requiring it.
7. **The Python edge** — whether replacing sni-demux/socket-relay with a Rust terminator is in scope here or a separate spec. It is the reason nothing at the edge can stamp today.

## 7 · Non-goals

- Do not redesign the MCP gateway surface (tonic-web on :8090 is settled).
- Do not add per-service TCP ports or new proto service packages.
- Do not introduce a WireGuard interface on the main host.
- Do not build OpenFlow/OVS packet tagging as the identity carrier: no container has a NIC and no container has a port on ovsbr0, so there are no packets to tag on the host→container hop. OVS classification at the public edge (eth0/pub0/svc0) may be referenced, but it is not this feature.

## 8 · Definition of done for the spec

requirements.md in EARS form covering, at minimum:

- genesis is minted exactly once per session and never recomputed
- the header is present on every request and the request fails closed when it is absent
- the chain record carries enough session identity to be sliced by session and re-verified offline from durable data alone
- verification requires no live shared-memory read
- exactly one function computes the genesis, and exactly one component reads the sled
- the two sled writers have disjoint field ownership: the stream owns the span and the tokio/metadata context, the mutation path owns the exact chain position. Neither can overwrite the other, and both derive from the same event, so agreement is structural rather than maintained
- **all seven existing writer call sites are removed or converted.** Seven writers cannot be made to agree; one writer per field cannot disagree. If the stream is added without retiring the others it is simply an eighth writer and nothing is fixed

design.md must show the arrival sequence end to end (key lands → blob sealed → genesis minted → session record written → header stamped → mutation delivered → offline re-verification), name every file and line it changes, and state what gets **deleted** — etch_footprint's index and port terms, anna_scribe's duplicate reader, the zeros sentinel, and the two uncommitted compensating patches in schema_bridge.rs and interceptor.rs.

tasks.md must be incremental and independently testable, and must sequence the fail-closed change after the stamping change.
