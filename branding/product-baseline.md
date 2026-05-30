# Product Baseline: Branding & Marketing Brief

This document is the source-of-truth feed for LLM-assisted branding and marketing work.
It covers the full platform, all major components, and three primary market angles.

---

## Platform Overview

**3tched** (pronounced "etched") is a native Linux system control plane built in pure Rust.
It replaces entire legacy infrastructure stacks — systemd, NetworkManager, Active Directory,
Docker, LVM — with a unified, database-driven, privacy-first orchestration layer.

Every action is etched: recorded on Chronicle, the platform's distributed blockchain.
Every capability is discoverable: 16,000+ system tools indexed natively from D-Bus.
Every component is replaceable: 40+ state plugins cover network, identity, storage, containers,
services, and compliance — each independently swappable.

**Underlying tech in one line**: D-Bus introspection + WireGuard networking + gRPC control
plane + Sled/BTRFS state store + 70+ AI agents + distributed blockchain — pure Rust,
no external tool registries, no framework lock-in.

---

## Chronicle (Crates: op-blockchain + op-state-store)

**Chronicle** is 3tched's audit system. It has two components with distinct roles:

**op-blockchain** — the event recording layer:
Every event on the system — tool execution, state change, policy decision, login — is
appended to an on-disk log as a time-stamped, self-hashing block stored on BTRFS.
Each `BlockEvent` carries a SHA256 hash of its own content (`timestamp:category:action:data`).
Events are written sequentially as `block-N.json` files. This is the append-only, durable
record — the raw material of the audit trail.

**op-state-store::ChainEvent** — the hash-linked compliance chain:
The compliance and reproducibility layer. Each `ChainEvent` carries a `prev_hash` field and
computes `event_hash = H(prev_hash || canonical_payload)`. You cannot alter any event without
breaking every hash that follows. Additional fields: `actor_id`, `capability_id`,
`plugin_id`, `decision` (Allow/Deny), `deny_reason`, `input_patch_hash`,
`result_effective_hash`. Merkle tree batching for scale; tag-scoped proofs for compliance.
This is the tamper-proof chain auditors rely on.

**Current crate names**: `op-blockchain` (event log), `op-state-store` (chain + compliance).
The product-facing name for the combined system is Chronicle.

### The two layers
- **Layer 1 — the chain**: `op-state-store::ChainEvent` — hash-linked (`prev_hash`),
  append-only, with decision tracking and actor attribution. Tamper-proof by math.
- **Layer 2 — the recall**: An AI layer (Qdrant) designed to vectorize all Chronicle events
  for plain-language query — "what changed on this host between 2am and 4am?" The embedding
  pipeline is in active development.

### Tagline candidates
- "Every event. Chained. Proven."
- "The blockchain your system runs on."
- "Immutable by math, not policy."
- "3tched's blockchain. Your proof."

---

## Platform Components

### 1. The Bus — D-Bus Introspection Engine (op-introspection)

**What it is**: A native D-Bus introspection engine that auto-discovers and catalogs every
capability on the system — no shell wrappers, no hand-coded tool lists.

**How it works**:
- Calls `org.freedesktop.DBus.ListNames()` natively via `zbus` to enumerate all services
- Calls the D-Bus Introspectable interface on each service to get XML schemas
- Parses XML to typed JSON structs (methods, properties, signals, interfaces)
- Indexes everything into an FTS5 full-text search index for semantic tool discovery
- Each D-Bus method becomes a callable tool; aggregated across all active services = 16,000+

**Why 16,000+ tools**: systemd alone exposes 100+ methods; NetworkManager 150+; CUPS 200+.
Multiplied across all active services on system + session bus, plus 70+ agent tool wrappers
registered as D-Bus services, plus MCP tool endpoints = 16,000+ discoverable, callable tools.
None are hand-coded. All are auto-discovered at startup.

**Who benefits**:
- AI agents: access the entire system's capabilities without manual tool definitions
- Platform engineers: introspect any service without `dbus-send` CLI parsing
- Security teams: full inventory of every callable interface on the system

**Tagline candidates**:
- "Every capability on your system. Discovered. Callable."
- "16,000 tools. Zero shell scripts."
- "The bus knows everything. So do your agents."

---

### 2. gRPC Bridge (op-grpc-bridge)

**What it is**: Bidirectional D-Bus ↔ gRPC synchronization layer. D-Bus state changes
project in real time to gRPC streaming subscribers; gRPC mutations flow back through the
same event chain.

**Architecture**:
```
D-Bus property change → SchemaEngine → Chronicle block → broadcast to gRPC subscribers
gRPC client mutate()  → SchemaEngine → Chronicle block → broadcast to D-Bus watchers
```

**Services exposed**:
- `StateSync` — subscribe to live state changes (server-streaming), mutate, batch mutate
- `PluginService` — list plugins, get schema, call method, subscribe signals
- `EventChainService` — get Chronicle blocks, subscribe events, verify chain, get cryptographic proof
- `OvsdbMirror` — full OVSDB network state as gRPC messages
- `RuntimeMirror` — system info, service list, metrics streaming, NUMA topology
- `ComponentRegistry` — service discovery with Watch streaming

**Why it matters**: Remote clients (web dashboard, mobile, cloud services) get real-time
system state over gRPC-Web. The audit trail is unified — Chronicle doesn't care whether
a mutation came from D-Bus or gRPC; both go through the same SchemaEngine.

**Technical**: port 50051, TLS-ready, gRPC-Web enabled, built-in reflection for `grpcurl`,
tonic-health liveness probes on all services.

---

### 3. Agent Platform (op-agents)

**What it is**: 70+ specialized AI agents, each a domain expert, running in sandboxed
execution environments and exposed as D-Bus services.

**Structure**: Trait-based (`AgentTrait`), lifecycle-managed registry (Idle/Running/Paused/
Error/Stopped), path validation and resource limits per agent, registered to D-Bus as
`org.dbusmcp.Agent.<Name>` so any tool or workflow can invoke them.

**Agent categories** (70+ total):
| Category | Agents |
|---|---|
| Languages (15) | rust-pro, python-pro, javascript-pro, typescript-pro, golang-pro, java-pro, c#-pro, c++-pro, ruby-pro, php-pro, scala-pro, elixir-pro, julia-pro, bash-pro |
| Infrastructure (5) | network-engineer, deployment, kubernetes, terraform, cloud-architect |
| Database (3) | database-architect, database-optimizer, sql-pro |
| Architecture (3) | backend-architect, frontend-developer, graphql-architect |
| Analysis (4) | code-reviewer, debugger, performance-engineer, security-auditor |
| AI/ML (6) | ai-engineer, data-engineer, data-scientist, ml-engineer, ml-ops-engineer, prompt-engineer |
| Orchestration (5) | context-manager, memory-agent, sequential-thinking-agent, tdd-orchestrator, dx-optimizer |
| Operations (3) | devops-troubleshooter, incident-responder, test-automator |
| Mobile (3) | flutter-expert, ios-developer, mobile-developer |
| Content/Docs (4) | api-documenter, docs-architect, mermaid-expert, tutorial-engineer |
| Security/Business (18+) | security-auditor, prompt-engineer, and more |

**Memory Agent**: Persistent cross-session memory with fuzzy match + access frequency +
tag matching. Memory types: Ephemeral (session), Persistent (CozoDB), Shared (cross-agent).
Optional ONNX vector embeddings for semantic similarity.

**Tagline candidates**:
- "70 domain experts. One platform. No context switching."
- "Agents that know your system — because they run on it."

---

### 4. Workstacks & DAG Workflows (op-workflows)

**Workstacks**: Immutable execution containers with causality tracking.
- **Vector clocks**: Distributed causality guarantees (happens-before ordering)
- **Content-addressable**: SHA256 hash of the execution sequence; identical patterns hit cache
- **Promotion candidates**: After 3 executions of a pattern, the orchestrator flags it as a
  promotion candidate (suggestion only — callers explicitly promote). Promoted workstacks
  cache to BTRFS; cache hits return pre-computed results in ~10ms vs multi-second execution
- **Immutable records**: Workstacks can be extended, never modified
- **Topological sort**: Execution order respects declared dependencies

**DAG Workflows**: Define a goal; the engine routes it through agents in the optimal order.
- Sequential, parallel, and conditional execution
- Configurable concurrency limits
- Intermediate LRU caching within a workflow
- Pattern tracking: frequently used patterns become workstack candidates

**Who benefits**:
- DevOps teams: reproducible multi-step pipelines with immutable causality records
- Researchers: notebook-style workflows with full execution history on Chronicle
- Compliance: every step of every workflow is on the blockchain

**Tagline candidates**:
- "Workflows that remember. Results that scale."
- "Immutable execution. Every step. Proven."

---

### 5. Ghostbridge — Per-User Privacy Network (op-network)

**What it is**: A privacy networking layer that gives each user their own encrypted network
container — WireGuard tunnel, private IP, kernel namespace. Not a VPN app. OS-level isolation.

**How it works**:
- Unique WireGuard keypair generated per user, on-device
- Route IDs derived via HKDF — cannot be guessed or reverse-engineered
- Per-user network namespace: kernel-enforced, not application-enforced
- OVS bridges with OpenFlow rules handle traffic isolation between namespaces
- No logs of user traffic — only Chronicle records authentication events

**Key differentiators**:
- vs. Tailscale/WireGuard apps: kernel namespace, not userspace daemon
- vs. Tor/Mullvad: deterministic routing under your control, not anonymizing relays
- vs. Corporate VPNs: per-user containers mean a compromised account can't pivot to others

**Tagline candidates**:
- "Your network. Your namespace. Nobody else's."
- "Cryptographically isolated. Operationally invisible."
- "Every user gets their own internet."
- "WireGuard, without the setup. Privacy, without the tradeoffs."

---

### 6. op-identity — Zero-Password Identity

**What it is**: Native identity management that replaces LDAP and Active Directory.
Authentication is a WireGuard public key — no passwords, no LDAP queries, no AD trust.

**How it works**:
- WireGuard pubkey IS the identity — possession of the private key proves who you are
- Magic link registration: email token → WireGuard keypair provisioned automatically
- OAuth token cache via `org.freedesktop.secrets` (native secrets service)
- Session management with expiry, built into the platform

**Who benefits**:
- Enterprise teams replacing AD/LDAP: no more LDAP servers, no more AD domain join
- Zero-trust architectures: identity is cryptographic, not credential-based
- Privacy-first deployments: no central directory server to breach

**Tagline candidates**:
- "No passwords. No LDAP. No Active Directory. Just keys."
- "Your WireGuard key is your identity."

---

### 7. System Replacement Stack

3tched replaces the following Linux infrastructure components natively:

| Replaced | 3tched Component | How |
|---|---|---|
| **systemd** | op-services + dinit-dbus | Service definitions in SQLite; dinit (2-5MB) as PID 1 vs systemd (20-40MB); lifecycle managed via D-Bus |
| **NetworkManager** | op-network | Native netlink ops (rtnetlink); OVSDB JSON-RPC; OpenFlow (all versions, pure Rust); no NetworkManager daemon |
| **Active Directory / LDAP** | op-identity | WireGuard pubkey identity; in-memory session store (DashMap); magic link provisioning |
| **Docker / Podman** | op-plugins (incus/lxc state plugins) + op-network (container networking) | 5-10% overhead vs Docker's 20-30%; per-user WireGuard tunnels built in |
| **LVM / mdadm** | op-cache (BTRFS subvolumes) | Subvolume management, snapshots, incremental replication, retention policy with auto-pruning |
| **5 separate audit logs** | op-blockchain (Chronicle) | One blockchain, one query, every component |

**The pitch**: One platform, one binary, one audit trail — instead of five different tools
with five different log formats and five different permission models.

---

### 8. Compliance Engine (op-compliance + CozoDB)

**Schema Validation Engine** (`LawFirm::review_schema`): Every plugin schema is reviewed
before registration against four compliance frameworks in sequence:
- **Olivia Scal** (OSCAL): flags root-capable plugins for security control assessment
- **E.U.gene Risk** (EU AI Act): requires training data source declaration on AI/ML plugins
- **Penny Privacy** (GDPR): detects PII fields (email, user_id, phone) without retention policy
- **Reggie O.P.A.** (OPA): enforces versioning and structural policy requirements

Each validator returns structured errors — schema registration is blocked until all pass.
Every failed validation is an event on Chronicle.

**OSCAL integration**:
- NIST 800-53, FedRAMP, EU AI Act, GDPR controls mapped to platform events
- CozoDB subid_registry: canonical OSCAL subid taxonomy as queryable graph

**CozoDB knowledge graph**:
- Datalog-based graph database, embedded in Sled (pure Rust, no native lib)
- Stores: compliance rules, OSCAL subids, audit events, memory namespaces,
  user/session records, identity graph
- All relations queryable: "show me all NIST controls violated in the last 7 days"

**Who benefits**:
- FedRAMP teams: continuous monitoring with OSCAL-mapped Chronicle events
- Healthcare/finance: HIPAA/PCI controls enforced at execution time, not audit time
- DevSecOps: compliance guardrails baked into infrastructure — can't be bypassed

---

### 9. Plugin Architecture (op-plugins)

**40+ state plugins**, each managing one domain of system state:

| Category | Plugins |
|---|---|
| Core | adc, agent_config, config, dinit, endpoint, full_system, hardware, keypair, keyring, mcp |
| Network | net, netmaker, openflow, ovsdb_bridge, rtnetlink, privacy_router, privacy_routes |
| Identity/Auth | login1, users, wireguard, gcloud_adc |
| Container | incus, lxc |
| Services | service, packagekit, systemd, dinit |
| Storage/Security | pcidecl, privacy, schemas, software, web_ui |

**Plugin model** (`get_state`, `get_desired_state`, `set_desired_state`, `apply_state`,
`diff`, `validate`, `capabilities`): Declarative like `kubectl apply` — declare what you
want, the plugin reconciles the system to match.

**Immutable paths**: Plugins declare creation-only fields; the engine refuses to mutate them.
Every plugin change goes through Chronicle.

---

### 10. MCP / Cognitive Layer (op-mcp + op-cognitive-mcp)

**op-mcp (HTTP+SSE port 3001, WebSocket port 3002, gRPC port 50051)** — Standard Model Context Protocol server:
- Compact mode: 4 meta-tools expose 148+ underlying tools (token-efficient for LLMs)
- Full mode: all tools directly, no filtering
- Transports: stdio (Claude Desktop), HTTP+SSE, WebSocket, gRPC
- Real-time streaming: long-running tool results streamed as SSE

**op-cognitive-mcp (HTTP/SSE port 3003, gRPC port 50052)** — Knowledge and reasoning:
- CozoDB knowledge graph (see Compliance Engine above)
- Qdrant vector database for semantic search over code corpus and events
- RAG pipeline: ingest codebase via repomix → chunk → embed → store → query
- Voyage AI embeddings; ONNX models for multi-modal
- PII filtering: PII events go to Chronicle but are stripped before Qdrant

**LLM provider support** (configurable per deployment):
- Local: MCP proxy, in-process models
- External APIs: Gemini, Anthropic, Gemini CLI
- Operators choose their data residency model

---

### 11. Web Dashboard (op-web)

**Unified server on :8080** serving:
- REST API (health, status, tools, agents, chat, events)
- gRPC-Web proxy to op-grpc-bridge
- WebSocket chat with streaming
- MCP endpoint for Claude Desktop
- React/TypeScript/WASM frontend (axon-trace-ui)

**Frontend routes** (30+):
Overview, Chat, Tools, Agents, Models, LLM, Services, Security, Config, Inspector,
State, Logs, Workflows, Orchestration, Skills, Containers, Privacy Network, OVS,
OpenFlow, Knowledge, gRPC Diagnostics, Accountability (Chronicle), BTRFS, Data Stores,
Embedding

**Real-time**: Live metrics via gRPC Server-Streaming; event log (1000-entry ring buffer);
agent status; service health indicators.

---

### 12. NUMA-Aware Performance Caching (op-cache)

- Detect NUMA topology via CPU affinity; allocate BTRFS subvolumes on specific NUMA nodes
- Bind execution threads to the node's CPUs for cache locality
- BTRFS subvolume layout: `timing/` (Chronicle), `vectors/` (ML embeddings),
  `state/` (DR snapshots), `snapshots/` (incremental replication)
- Workstack promotion: 3 executions → flagged as candidate; callers explicitly promote to BTRFS; cache hit ~10ms vs multi-second

---

## Three Market Angles

### Angle 1 — Ghostbridge: Privacy Network

**Primary buyer**: Security architects, enterprise zero-trust teams, privacy-focused developers
**Primary pain**: Network exposure; shared VPN blast radius; application-layer "privacy" that doesn't hold
**Core promise**: Kernel-enforced isolation — not an app, not a policy, the kernel

#### Target Personas
- **Privacy-conscious professionals**: journalists, lawyers, activists, high-risk remote workers
- **Enterprise security teams**: zero-trust network access without third-party VPN vendors
- **Developers building privacy products**: embed Ghostbridge as infrastructure

#### Key Differentiators
- **vs. Tailscale/WireGuard apps**: OS-level, kernel namespace — not a userspace daemon
- **vs. Tor/Mullvad**: Deterministic routing you control, not anonymizing relays
- **vs. Corporate VPNs**: Per-user containers; a compromised account can't pivot to others

#### Taglines
- "Your network. Your namespace. Nobody else's."
- "Cryptographically isolated. Operationally invisible."
- "One shared VPN tunnel is one shared blast radius."
- "Every user gets their own internet."

#### Marketing Angles
1. **Fear/Risk**: "One shared VPN tunnel is one shared blast radius."
2. **Simplicity**: "No config files. No scripts. One command to provision a private network."
3. **Compliance**: "Prove isolation — don't just claim it. Every route change is in Chronicle."
4. **Developer pitch**: "Ship network privacy as a feature, not a project."

---

### Angle 2 — 3tched: Compliance Platform

**Primary buyer**: CISOs, compliance officers, FedRAMP teams, regulated industries
**Primary pain**: Audit burden; reconstructing timelines from logs; policy that can't be enforced pre-execution
**Core promise**: Chronicle — your distributed blockchain, your continuous proof

#### How Chronicle Works (compliance context)
- D-Bus nodes generate events; op-blockchain records them as append-only BTRFS blocks
- op-state-store::ChainEvent links each event to the previous via prev_hash — chain cannot be altered without breaking subsequent hashes
- OSCAL compliance mappings connect Chronicle events to regulatory controls automatically
- Schema validation engine (op-compliance) gates plugin registration: OSCAL, GDPR, EU AI Act, OPA validators run in sequence; registration blocked on any failure
- AI recall layer (Qdrant) in active development — embedding pipeline shipping

#### Core Features
| Feature | What It Means |
|---|---|
| Chronicle — distributed audit system | op-blockchain: append-only BTRFS event log; op-state-store: hash-linked compliance chain (prev_hash) |
| Schema validation engine (op-compliance) | OSCAL, GDPR, EU AI Act, OPA validators gate plugin registration |
| OSCAL-native compliance mapping | FedRAMP, NIST 800-53 controls tracked automatically |
| Change tracking across 40+ state domains | Network, identity, storage, containers, services |
| gRPC event streaming | Real-time Chronicle events to SIEM or assessor portal |
| Chronicle recall via Qdrant *(roadmap)* | Plain-language query of blockchain history |

#### Target Personas
- **CISOs and compliance officers**: audit-ready evidence without manual log wrangling
- **FedRAMP/FISMA teams**: continuous monitoring with OSCAL-mapped events
- **Regulated industries**: healthcare (HIPAA), finance (SOX/PCI), defense contractors
- **DevSecOps engineers**: compliance guardrails in infrastructure, not bolted on

#### Key Differentiators
- **vs. Splunk/SIEM**: Chronicle generates structured blockchain events at the source — not scraped log noise
- **vs. HashiCorp Vault + Terraform**: replaces the whole stack, not a collection of tools
- **vs. Chef InSpec / OpenSCAP**: prevents non-compliant states; doesn't just scan for them
- **vs. Manual audits**: Chronicle is machine-readable — ready for automated assessor review

#### Taglines
- "Compliance isn't a report. It's a system property."
- "Every action. Every change. Every violation. In Chronicle."
- "Built for auditors. Run by engineers."
- "3tched's blockchain. Distributed. Proven. Yours."
- "Don't detect policy violations. Prevent them."

#### Use Case Scenarios
- FedRAMP contractor: 3tched streams OSCAL-mapped Chronicle events to assessor portal automatically
- Healthcare breach: Chronicle's hash-linked compliance chain (op-state-store::ChainEvent) provides cryptographic proof of every state transition, exact order
- Financial firm: op-compliance validators gate every plugin registration against OSCAL/GDPR/OPA rules; every violation is on the blockchain

---

### Angle 3 — Personal Workspace Platform

**Primary buyer**: Power developers, privacy-first individuals, AI-native teams
**Primary pain**: Tool fragmentation; stateless AI with no memory; cloud services that own your data
**Core promise**: A workspace that knows your system, remembers your context, and runs on your hardware

#### How It Works
- Workspace = Ghostbridge container + identity record + BTRFS subvolume + agent runtime
- 70+ specialized agents run sandboxed, with full system access via D-Bus
- DAG workflow orchestration: define a goal, agents execute in parallel automatically
- Semantic memory (CozoDB + Qdrant) architected for persistent cross-session recall
- Every action recorded on Chronicle; every workspace is its own auditable namespace

#### Core Features
| Feature | What It Means |
|---|---|
| Ghostbridge container | Private network namespace per workspace |
| 70+ specialized AI agents | Domain experts on demand, sandboxed, full system access |
| Semantic memory architecture | CozoDB + Qdrant designed for cross-session context |
| Chronicle blockchain trail | Every workspace action on the distributed blockchain |
| BTRFS subvolume per workspace | Isolated, snapshotted, rollback-ready (via op-cache) |
| DAG workflow orchestration | Multi-step tasks in parallel, automatically |
| Multi-protocol access | Browser, gRPC, MCP, CLI |

#### Key Differentiators
- **vs. GitHub Codespaces**: includes identity, network, AI agents — not just a dev container
- **vs. Notion/Linear**: live system environment, not a document layer
- **vs. ChatGPT/Copilot**: full system access + persistent memory architecture, not stateless API calls
- **vs. 1Password/Bitwarden**: identity managed at OS level, not a browser extension

#### Taglines
- "Your tools. Your memory. Your network. One workspace."
- "The workspace that knows you — and only you."
- "70 specialized agents. One place. No setup."
- "Work that moves with you, not against you."

#### Use Case Scenarios
- Freelance developer: each client gets a separate workspace — isolated network, credentials, tools. No cross-contamination.
- Researcher: memory agent indexes notes, papers, code. Next session surfaces exactly what's relevant.
- Privacy-conscious team: configurable LLM providers including local models — operators choose what leaves the machine.

---

## Cross-Platform Positioning

| Dimension | Ghostbridge | 3tched Compliance | Personal Workspace |
|---|---|---|---|
| **Primary buyer** | Security architect | CISO / compliance officer | Developer / power user |
| **Primary pain** | Network exposure | Audit burden | Tool fragmentation |
| **Core promise** | Kernel isolation | Continuous proof | Persistent intelligence |
| **Key proof point** | Per-user WireGuard namespaces | Chronicle distributed blockchain | 70 agents + D-Bus tool surface |
| **Moat** | OS-level, no userspace attack surface | Prevention + blockchain proof | 16,000 tools, local-first AI |

## The Replacement Stack Pitch

For enterprise and infrastructure buyers who respond to "replace" messaging:

| You're running | 3tched replaces it with | Why it's better |
|---|---|---|
| systemd (20-40MB) | dinit + op-services (2-5MB, SQL-driven) | Lighter, auditable, no unit file sprawl |
| NetworkManager | op-network (native netlink + OpenFlow) | No daemon, native performance, built-in WireGuard |
| Active Directory / LDAP | op-identity (WireGuard pubkey) | Zero-password, no central server to breach |
| Docker / Podman | op-plugins (incus/lxc) + op-network | 5-10% overhead vs 20-30%; privacy networking built in |
| LVM / mdadm | op-cache (BTRFS subvolumes) | Snapshots, incremental replication, retention policy |
| 5 separate audit logs | op-blockchain (Chronicle) | One blockchain, one query, every component |

## Brand Voice Notes

- **Tone**: Precise. Confident. No hype. Engineers respect specificity — use it.
- **Avoid**: "revolutionary", "game-changing", "seamless", "next-gen"
- **Use**: Product names (Chronicle, Ghostbridge), concrete numbers (16,000+ tools, 70+ agents,
  40+ plugins), mechanism descriptions ("kernel namespace", "distributed blockchain",
  "native netlink"), outcome framing ("prove isolation, don't claim it")
- **Chronicle**: It IS a blockchain — hash-linked, append-only, distributed across D-Bus nodes
  via gRPC. Own the term. Current crate name: `op-blockchain`.
- **Ghostbridge**: The bridge no one can see — private paths that leave no visible trace.
- **3tched name**: Etched as in permanent (Chronicle), etched as in integrated (OS-level).
- **Replacement framing**: "replaces" is a strong word — use it deliberately for the stack
  replacement pitch. Only use it when you mean the full component, not just a feature.

## LLM Prompting Notes

When using this document to generate marketing copy:

**Grounding rules**:
- All claims must come from this document — no invented capabilities
- Concrete numbers: 16,000+ tools (D-Bus auto-discovery), 70+ agents (op-agents), 40+ plugins (op-plugins)
- Chronicle IS a blockchain — use the term. Two crates: `op-blockchain` (append-only event log, per-event SHA256) and `op-state-store` (hash-linked chain via `ChainEvent`, `prev_hash` field). The hash-linked tamper-proof property comes from `op-state-store::ChainEvent`, not `op-blockchain`.
- **Do not claim Qdrant plain-language recall as shipped** — it is in active development; use
  "designed to", "in development", or "roadmap" framing
- **Do not claim "no external calls" for LLM** — providers are configurable (Gemini, Anthropic,
  local); use "configurable data residency" framing

**Angle guidance**:
- Ghostbridge: security product voice — kernel-level, precise, no consumer privacy app tone
- 3tched compliance: speak to auditors and regulators, not just engineers — "proof", "evidence", "blockchain"
- Workspace: warmer and more personal — it's the human-facing layer
- Replacement stack: enterprise infrastructure voice — "replaces", "native", "zero dependencies"
- All angles share one theme: **OS-level guarantees, not application-layer promises**

**Component reference** (actual crate names):
- D-Bus engine → op-introspection / op-inspector
- Blockchain / Chronicle → op-blockchain
- Privacy network / Ghostbridge → op-network
- Identity → op-identity
- Agents → op-agents (70+)
- Workflows → op-workflows + workstacks
- Compliance → op-compliance + CozoDB + op-blockchain
- Plugins → op-plugins (40+)
- Container support → op-plugins (incus/lxc plugins) + op-network
- Storage/BTRFS → op-cache
- Web dashboard → op-web (axon-trace-ui frontend)
- MCP layer → op-mcp (port 3000) + op-cognitive-mcp (port 3001)
- Performance caching → op-cache (NUMA-aware BTRFS)
