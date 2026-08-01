# GhostBridge — System Overview (Netmaker spin)
### Pipelines · OSCAL · Vectors · Evidence chain — how it all fits, with Netmaker as the spine

> A plain-language tour of the whole machine. Each section ends with **"The Netmaker
> spin"** — why that piece depends on, or strengthens, the mesh.

---

## 0. The 30-second picture

GhostBridge is a **zero-trust control plane** that sits on a **Netmaker mesh** and does three things at once:

1. **Moves data** through hardened pipelines (mesh → identity → routing → services).
2. **Proves compliance** by mapping every action to **OSCAL** (machine-readable policy).
3. **Remembers and reasons** using **vectors** (semantic memory) + a **hash-linked evidence chain** (tamper-evident audit trail).

Netmaker is the layer all three stand on: it's the transport *and* the root of identity.

---

## 1. Pipelines — how things actually flow

There are **two kinds of pipeline**: the **data-plane** (live traffic) and the **knowledge-plane** (memory + learning).

### 1a. The data-plane pipeline (a request's journey)
```
Netmaker peer (WireGuard)
   │   handshake = the credential
   ▼
Decoy ingress (decoy-wg2)  ── public face is a honeypot; nothing of value here
   │
   ▼
Identity sled (/dev/shm)    ── per-session footprint written at the door
   │
   ▼
Xray (wg-xray container)    ── terminates TLS, injects signed identity header,
   │                           routes by SNI / OpenFlow tags
   ▼
A.N.N.A. interceptor (gRPC) ── validates HMAC-stamped header at EVERY hop
   │
   ▼
Service container           ── netmaker-api / mail / qdrant / assistant …
                               (no NIC — reached only via unix sockets)
```
Every arrow is an enforcement point. The pipeline is "deny by default, prove to proceed."

### 1b. The knowledge-plane pipeline (memory + learning)
```
Action / document / transcript
   │
   ├──► Embed (Voyage voyage-code-3)  ──►  Qdrant   (semantic VECTORS)
   │
   └──► Classify (subid taxonomy)     ──►  Cozo     (learning GRAPH: nodes + edges)
```
- **Semantic sink** = "find me things *like* this" (similarity search).
- **Graph sink** = "show me how these things *relate*" (roles, plugins, peers, tasks).
- A daily export feeds both sinks, so the system's memory of the network grows over time.

### The Netmaker spin
The data-plane pipeline **starts at a Netmaker handshake** — no mesh, no front door. And the knowledge-plane pipeline ingests **mesh events as graph nodes** (peers, joins, posture changes), so Netmaker activity becomes queryable institutional memory, not just logs.

---

## 2. OSCAL — compliance as code (the "vectors" of policy)

**OSCAL** (Open Security Controls Assessment Language) is NIST's machine-readable format for security/compliance. Instead of a PDF policy, you have structured data: controls, roles, assessments, evidence.

In GhostBridge, OSCAL isn't paperwork bolted on — it's the **runtime policy model**:

- **Roles are the cast**: `authorizing-official`, `content-approver`, `assessor`, `system-owner`, `provider`, `operator`. A user's OSCAL role decides what A.N.N.A. lets them do.
- **Controls are routing tags**: a request carries a compliance class; the router (Gemma + OpenFlow) steers it accordingly. We call these the **subid taxonomy** (src / prj / sch / mut / obs / evt / exp) — think of them as the **policy vectors** that classify *what kind* of action this is.
- **Every mutation is an assessment event**: doing a thing automatically produces OSCAL evidence that the thing was authorized.

So "compliance vectors" = each action is tagged with a small structured vector of OSCAL/subid attributes that determines routing, permission, and the evidence it emits.

### The Netmaker spin
A Netmaker **peer's identity carries its OSCAL role**. Because identity is derived from the WireGuard relationship (next section), joining the mesh in the **compliance edition** means the peer is *already* an OSCAL principal — its role gates every hop, and its actions emit assessment evidence automatically. The mesh membership and the compliance posture are the same fact.

---

## 3. Vectors — two different meanings, both matter

People say "vectors" for two things here; keep them separate:

| | **Semantic vectors** | **Policy/compliance vectors** |
|---|---|---|
| What | High-dimensional embeddings of text/code | Small structured tags (OSCAL role + subid class) |
| Where | Qdrant | Carried on each request; logged to the graph |
| Used for | Similarity / RAG / "find related" | Permission + routing + evidence |
| Example | "this config looks like that incident" | "this is a `mut` by an `authorizing-official`" |

Both feed decisions: semantic vectors inform the **AI layer** (cognitive-mcp), policy vectors inform the **enforcement layer** (A.N.N.A.).

### The Netmaker spin
Mesh state gets embedded too: peer configs, topology changes, and posture snapshots become **semantic vectors**, so an operator (or an AI agent) can ask *"show me peers that look like the one that misbehaved last month"* — turning Netmaker telemetry into searchable knowledge.

---

## 4. The evidence chain ("blockchain" spin) — tamper-evident audit

We don't run a coin or a public ledger. What we have is a **hash-linked chain of custody** — the property people actually want when they say "blockchain": *you can't quietly alter the record.*

How the chain is built:

1. **Deterministic identity** — `session_id = Argon2(PSK, salt = WG_pubkey)`. Same peer → same identity, provable, unforgeable.
2. **Signed footprints** — each request gets an **HMAC stamp** under a vaulted issuer key (server can verify, user can't forge).
3. **Datestamped sleds** — the identity sled carries a timestamp; the interceptor rejects a footprint that doesn't match the current sled ("temporal hash mismatch").
4. **Per-event trace IDs** — every action gets a UUID, hashed (`blake3`) and written to the graph + Qdrant.
5. **Hash-linked records** — each evidence record references the prior state hash, so the audit trail is **append-only and self-verifying** — alter one link and the chain breaks.

That's the compliance promise: not "we logged it," but "here is a cryptographic chain proving it happened, in order, authorized, and unaltered."

> If/when a regulator wants an external anchor, the chain's head hash can be notarized to a public ledger — but the security comes from the hash-linking, not from any token.

### The Netmaker spin
The **root of the entire chain is the WireGuard PSK** Netmaker manages. The mesh credential is link #0. Every piece of evidence ultimately traces back to "this Netmaker peer, with this key, at this time." Netmaker isn't just transporting the audited traffic — it's the **anchor the whole evidence chain hangs from.**

---

## 5. Putting it together — one diagram

```
            ┌───────────────────────── NETMAKER MESH (WireGuard) ─────────────────────────┐
            │  PSK + pubkey  =  root of identity  AND  link #0 of the evidence chain        │
            └───────────────┬───────────────────────────────────────────────┬─────────────┘
                            │                                               │
                   DATA-PLANE PIPELINE                              KNOWLEDGE-PLANE PIPELINE
                            │                                               │
   decoy → sled → Xray(header) → A.N.N.A. → service          embed→Qdrant (semantic vectors)
                            │                                  classify→Cozo (learning graph)
                            ▼                                               │
                 OSCAL role + subid vector                                  ▼
                 gates each hop, emits evidence  ─────────────►  hash-linked evidence chain
                                                                  (tamper-evident audit)
```

**Read it as:** Netmaker roots identity → pipelines move and gate traffic → OSCAL vectors decide and document → semantic vectors remember → the evidence chain makes it all provable.

---

## 6. One-paragraph version (for the meeting)

> GhostBridge runs on a Netmaker mesh and turns it into a zero-trust, compliance-grade backbone. Traffic flows through a hardened pipeline — decoy ingress, identity sled, Xray header injection, and a per-hop interceptor — where every request is tagged with OSCAL/compliance *vectors* that decide permission and emit audit evidence. In parallel, a knowledge pipeline embeds everything into semantic *vectors* (Qdrant) and a learning graph (Cozo) so the system remembers and reasons. All of it is bound into a hash-linked *evidence chain* whose root is the WireGuard pre-shared key Netmaker already manages. Netmaker isn't a dependency we use — it's the spine: the transport, the root of identity, and link zero of the audit chain.
