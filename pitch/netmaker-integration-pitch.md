# GhostBridge × Netmaker
### A zero-trust, OSCAL-backed control plane built on the Netmaker mesh

> Pitch deck — partner meeting with Netmaker
> Prepared from the live GhostBridge architecture (Operation D-Bus)

---

## Slide 1 — The one-liner

**We turned Netmaker into the identity-bearing backbone of a zero-trust, compliance-grade control plane — and we want to do it *with* you, not around you.**

Netmaker gives us the mesh. We add: cryptographic identity that rides the WireGuard layer, OSCAL-mapped policy enforcement at every hop, AI-driven orchestration, and a hardening pattern that makes the Netmaker server itself unreachable to attackers.

Two products fall out of one codebase: a **Privacy** edition and a **Compliance** edition.

---

## Slide 2 — The problem we both keep hitting

Distributed/edge networks need three things that normally fight each other:

1. **Connectivity** — flat, fast, NAT-traversing mesh (Netmaker solves this today).
2. **Zero-trust identity** — *who* is on the wire, provable, not just *what IP*.
3. **Compliance evidence** — an auditor-grade record that policy was enforced, not just configured.

Most stacks bolt #2 and #3 on top with IAM sprawl and SIEM glue. We pushed identity and policy **down into the transport** — the WireGuard handshake *is* the credential, and every gRPC hop is an enforcement point.

Netmaker is the natural substrate for that. This deck shows how we wired it.

---

## Slide 3 — Where Netmaker sits in the stack

```
            Internet
               │
        ┌──────▼───────┐
        │  Decoy ingress│  Oracle VPS (decoy-wg2) — public face is a honeypot.
        │  (WireGuard)  │  Identity injection only; nothing of value lives here.
        └──────┬───────┘
               │  WG handshake = the front door
        ┌──────▼────────────────────────────────┐
        │  Hypervisor (Incus host)               │
        │                                        │
        │   netclient ─ Netmaker mesh member     │
        │                                        │
        │   ┌───────── wg-xray container ───────┐│
        │   │ Xray-core: VLESS+TLS, SNI routing ││
        │   │ OpenFlow steering, identity header ││
        │   └───────┬──────────┬────────────────┘│
        │           │          │                 │
        │   ┌───────▼──┐ ┌─────▼─────┐ ┌────────┐│
        │   │ netmaker │ │netmaker-mq│ │netmkr- ││  ← Netmaker server,
        │   │  (API)   │ │(mosquitto)│ │  ui    ││    hardened (next slide)
        │   └──────────┘ └───────────┘ └────────┘│
        └────────────────────────────────────────┘
```

The Netmaker control server runs **as containers on the hypervisor**, fronted by Xray, reached only through the mesh + identity layer.

---

## Slide 4 — The hardening innovation: a Netmaker server with **no network interface**

This is the part we most want to show you.

The `netmaker`, `netmaker-mq` (Mosquitto), and `netmaker-ui` containers run with **NO NIC** — loopback only. They cannot be reached over IP at all.

- Each service binds `127.0.0.1` inside its own container.
- An **Incus proxy device** exposes each as a **unix socket** on the host (`/run/netmaker/api.sock`, `mq.sock`, `mqtts.sock`, `ui.sock`).
- The `wg-xray` container mounts `/run/netmaker` and bridges those sockets back to TCP **only inside its own namespace**, where Xray terminates TLS and routes by SNI:
  - `api.*` → `api.sock` (Netmaker API)
  - `broker.*` → `mq.sock` / `mqtts.sock` (MQTT)
  - `dashboard.*` → `ui.sock` (Netmaker UI)

**Result:** there is no open port on the Netmaker server. A scanner sees nothing. The attack surface is a unix socket file reachable only after the WireGuard + identity gate. This is a deployment pattern we think your security-conscious users would want as a reference architecture.

---

## Slide 5 — Identity rides the WireGuard layer

We don't run a separate IAM. The credential **is** the WireGuard relationship Netmaker already manages:

```
session_id = Argon2( secret = PSK, salt = WG_public_key )
```

- The **PSK** rides WireGuard's built-in `PresharedKey` slot — already part of the Netmaker peer config, invisible on the wire.
- `session_id` is **stable and deterministic** (same peer → same identity), so sessions persist and are correlatable for audit, yet unguessable.
- At provisioning: peer pubkey → stored in graph DB → MCP token = `UUIDv5(pubkey)`.

So **every Netmaker peer is automatically a first-class identity** in our control plane. No extra enrollment step — joining the mesh *is* enrollment.

---

## Slide 6 — A.N.N.A.: enforcement at every hop

**A.N.N.A. = Axon Network Notary Arbitrator** — a gRPC interceptor that checks + approves every request, in real time, at every internal door (not a one-time gateway check).

- WG handshake → identity sled (`/dev/shm`) → Xray injects a signed **GhostBridge identity header** → `GhostbridgeInterceptor` validates it → request reaches the service.
- The header carries an **HMAC stamp** under a vaulted issuer key — the user can't forge it, the server verifies it without holding the raw PSK.
- Roles come from **OSCAL** (`authorizing-official`, `content-approver`, `assessor`, `system-owner`, …). Policy isn't a config file; it's the cast of an OSCAL system security plan, enforced at runtime.

Every mutation is an enforcement point and produces audit evidence. That's the compliance story auditors actually accept.

---

## Slide 7 — One codebase, two editions

| | **Privacy edition (GhostBridge)** | **Compliance edition (Corporate)** |
|---|---|---|
| Identity | Key **is** the identity | Key **maps to** a verified user |
| User linkage | None — no email, no PII | `pubkey ↔ user` edge + sealed vault escrow |
| De-anonymization | Impossible by design | A.N.N.A.-gated, OSCAL-authorized only |
| Data store | Public + hashed only (pubkey, `blake3(psk)`, token) | Adds escrow vault + ACL graph |
| Target | Privacy networks, journalists, sensitive ops | Enterprise / EU regulatory |
| Netmaker role | Mesh + anonymous peer identity | Mesh + attributable peer identity |

> Deployment mode is chosen at install time — one instance is one mode. "If they want both, they run two." The Netmaker mesh underpins **both**; only the identity-linkage layer differs.

---

## Slide 8 — The control plane on top (why this is more than a VPN)

- **D-Bus-style state plugins**: every domain (incus, netmaker, xray, dns, mail, qdrant…) is a plugin implementing a common `StatePlugin` trait (query → diff → apply → verify → checkpoint → rollback). Netmaker is a first-class plugin.
- **Schema-driven control panel**: the Netmaker unix sockets and containers are declared in plugin schemas, so they **render automatically in a GUI control panel** — peers, sockets, broker, UI, all visible and manageable.
- **AI orchestration (cognitive-mcp)**: an MCP gateway lets AI agents query and drive the mesh + compliance state through governed tools — provisioning peers, reading posture, generating OSCAL evidence.
- **Knowledge graph**: every action is classified and sunk into a semantic store (Voyage→Qdrant) + a learning graph (Cozo) — institutional memory of the network's posture over time.

---

## Slide 9 — Integration touchpoints with Netmaker today

What we already use from Netmaker:

- **netclient** — mesh membership on the hypervisor and edge nodes.
- **Netmaker server (CE v1.5.1)** — API (`:8081`), Mosquitto broker, UI — deployed in the hardened no-NIC pattern above.
- **Peer provisioning** — Netmaker peer lifecycle is the enrollment trigger for GhostBridge identity.
- **WireGuard PSK slot** — reused as the root secret for our identity derivation.
- **CoreDNS / mesh addressing** — internal service reachability over the mesh.

Everything we add is **complementary** — we don't fork netclient or the server; we wrap and harden them.

---

## Slide 10 — Why bring this to Netmaker

1. **A reference hardening architecture** (no-NIC, socket-fronted server) your enterprise + gov users will want.
2. **A compliance/zero-trust layer** that makes Netmaker answer RFPs it can't today (OSCAL, attributable identity, audit evidence).
3. **An AI-native control surface** for the mesh — agents that provision and audit peers through governed tools.
4. **Two go-to-market surfaces** (privacy + compliance) that both pull Netmaker in as the substrate.

We're not competing with Netmaker — we're a force-multiplier *on* Netmaker.

---

## Slide 11 — The ask

- **Technical**: validate the no-NIC / socket-fronted deployment pattern; guidance on Netmaker API surface for programmatic peer + identity provisioning at scale.
- **Roadmap alignment**: where our identity-on-WG model meets your enterprise auth roadmap (SSO, OAuth, ACLs).
- **Partnership**: co-marketing the hardened "zero-trust Netmaker" reference architecture; design-partner status for the compliance edition.

---

## Slide 12 — Close

> Netmaker connects the machines.
> GhostBridge proves *who* they are and *that policy was enforced* — every packet, every hop, auditor-grade.
>
> The mesh is the backbone. Let's make it the **trusted** backbone, together.

---

### Appendix — talking points / FAQ

- **"Isn't no-NIC fragile?"** It's resilient: services are loopback-bound, sockets are recreated on container start, and the pattern survives reboot via `boot.autostart`. Failure modes are contained to a single container, not the mesh.
- **"How does identity survive a key rotation?"** `session_id` is derived from `(PSK, pubkey)`; rotating the WG keypair issues a new identity by design — old footprints remain in the audit graph, timestamped.
- **"Where's the compliance evidence stored?"** Hashed/public facts in the user store; forging secrets (issuer key, escrow) in a separate vault referenced by `vault://`. The ACL is modeled as graph data; enforcement happens at the interceptor, not the DB.
- **"Performance overhead?"** Identity check is an HMAC verify + sled lookup in `/dev/shm` — microseconds per hop. Xray TLS termination is the only crypto cost on the data path.
