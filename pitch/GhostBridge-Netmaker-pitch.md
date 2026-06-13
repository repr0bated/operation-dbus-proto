# GhostBridge × Netmaker
### Netmaker isn't a dependency in this project — it's core code.

*A short read before we meet. Honestly: this is the first time anyone outside the
project will have seen it. We booked the trial/sales call because it was the door —
but what we actually want is for a sharp engineer on your side to look closely and
tell us what you really think.*

---

## TL;DR (for the busy reader)

- We've built a **zero-trust, compliance-grade AI control plane** — in Rust — that runs **on a Netmaker mesh** and treats it as the spine: the transport **and** the root of identity.
- We deploy your control server in a pattern most of your customers don't: **no network interface at all** — loopback + unix sockets, fronted by Xray. A port scan returns nothing.
- We already run Netmaker in production. We're not here for a standard enterprise eval — we're here because **you're already load-bearing in something we think is genuinely novel**, and you should see it.
- **What we want from this meeting:** a real first reaction from someone technical. Pull on any thread below — we'd rather be challenged than flattered. (The one practical thing we're feeling out along the way is **multiple egress**, but that's not the point of the conversation.)

---

## What GhostBridge is, in one paragraph

GhostBridge turns a Netmaker mesh into a **trusted** backbone. Traffic flows through a hardened pipeline — decoy ingress, identity sled, Xray header injection, a per-hop interceptor — where every request is classified, routed, gated against machine-readable policy (OSCAL), and recorded into a **tamper-evident hash-linked chain**. An AI layer reasons over that verified history but **cannot execute anything itself**. Same codebase ships as a **privacy** edition or a **compliance** edition — one config flag decides which world you're in.

---

## How it works — one governance pattern, every layer

The whole system runs on a single discipline: **make the wrong thing impossible first; gate what's left; record everything.** Here it is top to bottom.

```
  RUST          the compiler is the first enforcer — invalid code won't build
    ▼
  SCHEMA        invalid STATE is unrepresentable — the silent gate, before any rule runs
    ▼
  TRAFFIC COP   Gemma classifies (what is this?) + Xray routes (where does it go?)
  (Gemma+Xray)  — directs traffic, reports every decision up. Directs, never authorizes.
    ▼
  ZEROCLAW      routes to the right MODEL by cost / reasoning / specialty —
                schema-native from the ground up, so it can't route off-policy
    ▼
  A.N.N.A.      OSCAL-gated approval at EVERY hop (HMAC-stamped identity header)
                — the higher-up that actually says yes/no
    ▼
  CHAIN         hash-linked, tamper-evident evidence. Every action becomes a block.
    ▼
  THE LOOP      AI reassembles the chain into a semantic DB (Qdrant) + realtime graph
                (Cozo), reasons over it, recommends/acts-by-delegation → new blocks
```

**The same idea at three scales:** Rust says *code can't be wrong*, schema says *state can't be invalid*, A.N.N.A. says *action can't be unauthorized* — and the chain says *and it's all on the record.*

---

## Four things that make this different

**1. The AI is a brain with no hands.**
The chatbot is a **delegator, recommender, and fixer** — and that's the ceiling. **No internet access, no execution rights, no execution control.** It proposes into a schema-shaped mold; a separate, OSCAL-gated, chained layer disposes. A confabulating or compromised model can't do damage — the worst case is a rejected recommendation. Autonomous *reasoning* without autonomous *risk*.

**2. You can cross-examine it.**
Because every action is a chained, content-addressed, searchable block, you can confront the agent in real time — *"why did you do this?"* — and it answers with **cryptographic receipts** pulled live by semantic search (chatbot on top, evidence on the bottom). If the model confabulates, the evidence pane contradicts it. **An AI you can put on the stand.**

**3. The chain is the truth; the DB and graph are rebuilt views.**
The hash-linked chain is canonical and tamper-evident. Qdrant (semantic) and Cozo (realtime graph) are **AI-reassembled projections** of it — if they're lost or corrupted, you re-derive them from the chain. Trust lives in the chain; queryability lives in the projections. (It's a blockchain *by structure* — hashed, chained blocks — kept honest by **anchoring**, not consensus, which is the right call for a single-operator compliance system.)

**4. One dial: Snowden ↔ Big Brother.**
Same mesh, same chain, same confrontable AI — **only the identity-linkage layer changes**, at deploy time:
- **Privacy edition (Snowden):** key *is* identity, no PII, de-anonymization **cryptographically impossible**.
- **Compliance edition (Big Brother):** every actor attributable, de-anon **only on a signed OSCAL authority — which is itself a chained, confrontable block.** You can cross-examine the watchers.

---

## The Netmaker hardening pattern (the part we want to show you)

Your control server runs with **no NIC**:
- `netmaker`, `netmaker-mq` (Mosquitto), `netmaker-ui` → loopback only.
- Exposed to the host as **unix sockets** (`/run/netmaker/api.sock`, `mq.sock`, `mqtts.sock`, `ui.sock`) via Incus proxy devices.
- `wg-xray` + Xray terminate TLS and route by SNI to those sockets.

**There is no open port on the Netmaker server.** The attack surface is a socket file behind the WireGuard + identity gate. We'd love to publish this jointly as a reference architecture for your security-conscious users.

---

## Why Netmaker is core, not a dependency

- It's a **first-class state plugin** — managed by the same engine that runs the whole platform.
- Its peers **are the identity model**: `session_id = Argon2(PSK, salt = WG_pubkey)`, rooted in the WireGuard pre-shared key you already manage. Remove Netmaker and the identity layer has no root. It is **link #0 of the evidence chain.**
- **Every GhostBridge deployment is a hardened Netmaker deployment**, sold into regulated rooms Netmaker alone doesn't reach yet. We expand your surface; you anchor our spine.

---

## What we're actually after

Plainly: **we want a knowledgeable human to look closely and react.** This is the first outside set of eyes on the project, and your team is the right first audience — because you built the layer it all stands on.

So in the meeting:
1. **Tell us what you really think** — poke at the hardening pattern, the identity model, the chain, the brain-with-no-hands. We want the hard questions, not a thumbs-up.
2. **Tell us where we're wrong about Netmaker** — if we're using something in a way you'd discourage, or missing a capability that'd make this cleaner, that's gold.
3. **The one practical itch:** we're working through **multiple egress** — if that's a natural part of the conversation, great; if not, no pressure.

No commercial ask. Just: be the first humans to see this, and tell us if it's as interesting as we think it is.

---

## Close

> Netmaker connects the machines.
> GhostBridge proves *who* they are and *that policy was enforced* — every hop, auditor-grade, in an AI loop you can cross-examine.
>
> You're already in the core of what we built. We'd just like you to be the first to *see* it — and tell us, straight, what you think.

---

### Appendix — quick FAQ

- **"Is it really a blockchain if it's not distributed?"** It's a blockchain *by structure* (hashed, chained blocks). Distribution buys trustless multi-party consensus; we don't need that — we get operator-tamper-resistance by **anchoring** the chain head externally. Correct tool for a single-operator compliance ledger.
- **"What stops the AI going rogue?"** It has no hands — no internet, no execution rights. It recommends; the governed layer executes. Hallucination has no reach.
- **"Why Rust / why these components?"** Selection principle: **schema-native from the ground up** (e.g. zeroclaw) and **safe by construction** (Rust — memory-safe, race-free, single static binaries). The governance holds at every layer because nothing was bolted on.
- **"What do you actually want from us?"** An honest technical reaction — you're the first outside eyes on this. Multiple egress is the one practical thing we're working through, but the reason we're excited is that you built the layer this all stands on.
