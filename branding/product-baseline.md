# Product Baseline: Branding & Marketing Brief

This document is the source-of-truth feed for LLM-assisted branding and marketing work.
It covers three distinct product angles drawn from the same underlying platform.

---

## Platform Overview

**3tched** (pronounced "etched") is a native Linux system control plane built in pure Rust.
It replaces legacy infrastructure stacks — systemd, NetworkManager, Active Directory, Docker —
with a unified, database-driven, privacy-first orchestration layer. The name signals
permanence: every action is etched into an immutable audit trail.

**Underlying tech in one line**: D-Bus introspection + WireGuard networking + gRPC control plane
+ SQLite/BTRFS state store + LLM-powered agents — all in a single, zero-dependency Rust binary.

---

## Angle 1 — Ghostbridge: Privacy Network

### What It Is
Ghostbridge is the privacy networking layer of 3tched. It gives each user their own encrypted
network container — a private tunnel that isolates traffic, masks identity, and routes
communications through cryptographically derived paths.

### How It Works (non-technical summary)
- Every user gets a unique WireGuard keypair generated on-device.
- Traffic is routed through a private IP assigned exclusively to that user.
- Routes are derived via HKDF (a cryptographic key derivation function) so they cannot be
  reverse-engineered or predicted.
- Network containers are ephemeral and namespaced — no shared surfaces with other users.
- No logs of user traffic are kept; only the immutable audit trail of authentication events.

### Core Features
| Feature | What It Means for Users |
|---|---|
| Per-user WireGuard containers | Your traffic never shares a path with anyone else |
| Cryptographically derived route IDs | Routes cannot be guessed or mapped by outsiders |
| Private IP assignment | You appear as a known identity only within your authorized namespace |
| Network namespace isolation | System-level separation — not just a VPN app |
| Zero shell dependencies | No OpenVPN, no NetworkManager, no iptables scripts |

### Target Personas
- **Privacy-conscious professionals**: journalists, lawyers, activists, remote workers in high-risk environments
- **Enterprise security teams**: zero-trust network access without third-party VPN vendors
- **Developers building privacy products**: embed Ghostbridge as infrastructure

### Key Differentiators vs. Competitors
- **vs. Tailscale/WireGuard apps**: Ghostbridge is OS-level, not application-level. Isolation is enforced by the kernel namespace, not a userspace daemon.
- **vs. Tor/Mullvad**: Deterministic routing under your control, not anonymizing relays. Speed + privacy, not anonymity theater.
- **vs. Corporate VPNs**: Per-user containers mean a compromised account can't pivot to other users' traffic.

### Tagline Candidates
- "Your network. Your namespace. Nobody else's."
- "Cryptographically isolated. Operationally invisible."
- "WireGuard, without the setup. Privacy, without the tradeoffs."
- "Every user gets their own internet."

### Marketing Angles
1. **Fear/Risk**: "One shared VPN tunnel is one shared blast radius."
2. **Simplicity**: "No config files. No scripts. One command to provision a private network."
3. **Compliance**: "Prove isolation — don't just claim it. Every route change is on the audit trail."
4. **Developer pitch**: "Ship network privacy as a feature, not a project."

---

## Angle 2 — 3tched: Compliance Platform

### What It Is
3tched is the compliance and governance layer of the platform. It gives enterprises,
regulated industries, and security teams a verifiable, immutable record of every system
action — mapped to regulatory frameworks like GDPR, FedRAMP, SOC 2, and NIST OSCAL.

### How It Works (non-technical summary)
- Every state change (user created, service started, permission granted) is written to a
  blockchain-backed audit log stored on BTRFS — append-only, cryptographically chained.
- A policy engine enforces rules before actions execute — not after the fact.
- Compliance mappings are built in: OSCAL schemas connect system events to regulatory controls automatically.
- Role-based access control with approval workflows means no action happens outside policy.

### Core Features
| Feature | What It Means for Compliance Teams |
|---|---|
| Immutable blockchain audit trail | Tamper-evident log acceptable for regulatory review |
| OSCAL-native compliance mapping | FedRAMP, NIST 800-53 controls tracked automatically |
| Pre-execution policy engine | Block non-compliant actions before they happen |
| Role-based access + approval workflows | Segregation of duties enforced at the OS level |
| Change tracking across 40+ state domains | Full visibility: network, identity, storage, containers, services |
| gRPC event chain | Real-time compliance event streaming to SIEM or dashboard |

### Target Personas
- **CISOs and compliance officers**: need audit-ready evidence without manual log wrangling
- **FedRAMP/FISMA teams**: building systems that must meet federal control requirements
- **Regulated industries**: healthcare (HIPAA), finance (SOX/PCI), defense contractors
- **DevSecOps engineers**: want compliance guardrails baked into infrastructure, not bolted on

### Key Differentiators vs. Competitors
- **vs. Splunk/SIEM tools**: 3tched generates structured compliance events at the source — not scraped log noise. Less false positives, less tuning.
- **vs. HashiCorp Vault + Terraform**: 3tched replaces the whole stack (identity, network, state, audit) — not a collection of tools requiring integration.
- **vs. Chef InSpec / OpenSCAP**: Passive scanners check what happened. 3tched's policy engine prevents non-compliant states from occurring.
- **vs. Manual audit processes**: The audit trail is machine-readable OSCAL — ready for automated assessor review.

### Tagline Candidates
- "Compliance isn't a report. It's a system property."
- "Every action. Every change. Every approval. Etched."
- "Built for auditors. Run by engineers."
- "The first infrastructure that proves itself."

### Marketing Angles
1. **Pain relief**: "Stop reconstructing what happened from logs. Know what happened as it happened."
2. **Proof over promises**: "Your auditors want evidence. 3tched generates it continuously."
3. **Prevention over detection**: "Don't detect policy violations. Prevent them."
4. **Cost**: "Reduce audit prep from months to a query."

### Use Case Scenarios
- A FedRAMP contractor needs to show continuous monitoring — 3tched streams OSCAL-mapped events to their assessor portal automatically.
- A healthcare org gets breached; forensics needs an exact timeline — the blockchain log provides cryptographic proof of every state transition.
- A financial services firm needs segregation of duties for privileged access — 3tched's approval workflows enforce it at the kernel level, not the application layer.

---

## Angle 3 — Personal Workspace Platform

### What It Is
The personal workspace is 3tched's end-user face: a unified, encrypted, AI-assisted
environment where a person's identity, files, network, and tools travel with them —
isolated from other users and governed by their own policy.

### How It Works (non-technical summary)
- A "workspace" is a namespaced container combining: a private network route (Ghostbridge),
  a personal identity record, a BTRFS subvolume for files, and an agent runtime.
- 70+ AI agents are available within the workspace — coding, networking, memory, search —
  running in sandboxed execution environments with no cross-user bleed.
- Workflows are DAG-based: the user defines a goal and the platform routes it through
  the right agents in parallel.
- A semantic memory layer (CozoDB knowledge graph + Qdrant vector DB) means the workspace
  learns — it remembers past context and surfaces relevant knowledge automatically.

### Core Features
| Feature | What It Means for Users |
|---|---|
| Isolated network container | Your browsing, tools, and communications in a private namespace |
| 70+ specialized AI agents | Domain experts (Rust, Python, Kubernetes, network, memory) on demand |
| Semantic memory (CozoDB + Qdrant) | The workspace remembers your context across sessions |
| BTRFS subvolume per user | Your files are isolated, snapshotted, and rollback-ready |
| DAG workflow orchestration | Multi-step tasks run in parallel automatically |
| Multi-protocol access | Use via browser, gRPC client, MCP tool, or CLI |

### Target Personas
- **Power users and developers**: want a persistent, intelligent environment that keeps up with their work
- **Privacy-first individuals**: want a workspace that doesn't leak data to cloud providers
- **Remote teams**: need isolated, reproducible environments per team member
- **AI-native workers**: want agents that have real context about their work, not generic chatbots

### Key Differentiators vs. Competitors
- **vs. GitHub Codespaces / Gitpod**: 3tched workspaces include identity, network, and AI agents — not just a dev container.
- **vs. Notion / Linear**: The workspace is a live system environment, not a document layer on top of other tools.
- **vs. ChatGPT / Copilot**: Agents run locally with full system access and persistent memory — not stateless API calls.
- **vs. 1Password / Bitwarden**: Identity and secrets are managed at the OS level, not a browser extension.

### Tagline Candidates
- "Your tools. Your memory. Your network. One workspace."
- "The workspace that knows you — and only you."
- "Work that moves with you, not against you."
- "A personal OS layer. Finally."

### Marketing Angles
1. **Productivity**: "Stop context-switching. Your agents remember where you left off."
2. **Privacy**: "A workspace only you can see — enforced by the kernel, not a checkbox."
3. **Power**: "70 specialized agents. One place. No setup."
4. **Ownership**: "Your workspace doesn't live on someone else's server. It lives on yours."

### Use Case Scenarios
- A freelance developer switches clients daily — each client gets a separate workspace with isolated network, credentials, and tools. No cross-contamination.
- A researcher runs a memory agent that indexes all their notes, papers, and code. Next session it surfaces exactly what's relevant to the current task.
- A privacy advocate wants to run AI tools locally without sending data to OpenAI — 3tched's agent runtime runs models in-process with no external calls.

---

## Cross-Platform Positioning

| Dimension | Ghostbridge | 3tched Compliance | Personal Workspace |
|---|---|---|---|
| **Primary buyer** | Security architect | CISO / compliance officer | Individual / dev team |
| **Primary pain** | Network exposure | Audit burden | Tool fragmentation |
| **Core promise** | Isolation | Evidence | Intelligence |
| **Key proof point** | Kernel-level namespacing | OSCAL-mapped immutable log | 70 agents + semantic memory |
| **Competitive moat** | No userspace VPN compromise surface | Prevention vs. detection | Local-first AI with real context |

## Brand Voice Notes

- **Tone**: Precise. Confident. No hype. Engineers respect specificity — use it.
- **Avoid**: "revolutionary", "game-changing", "seamless", "next-gen"
- **Use**: concrete numbers (16,000+ tools, 70+ agents), mechanism descriptions ("kernel namespace, not app layer"), and outcome framing ("prove isolation, don't claim it")
- **3tched name**: lean into the double meaning — etched as in permanent/immutable (audit trail), etched as in deeply integrated (OS-level, not bolted on)
- **Ghostbridge name**: the bridge no one can see — private paths between endpoints that leave no visible trace

## LLM Prompting Notes

When using this document to generate marketing copy:
- Ground all claims in the concrete features listed above — no invented capabilities
- Maintain the three angles as distinct products with distinct buyers, even though they share infrastructure
- Ghostbridge copy should feel like a security product, not a consumer privacy app
- 3tched compliance copy should speak to auditors and regulators, not just engineers
- Workspace copy can be warmer and more personal — it's the human-facing layer
- All three can share the theme: **OS-level guarantees, not application-layer promises**
