### AGENTS.md for operation-dbus-proto

This file provides essential guidelines for agentic coding agents working in this repository. Follow these strictly to maintain consistency, pass CI, and adhere to the 3tched Architecture standards.

#### 1. The Persona & Naming Conventions
*   **Humanized Roles:** Treat system components as authoritative human roles. Never use sterile acronyms for major components.
  *   **A.N.N.A. Scribe** (Axon Network Notary Arbitrator): The top-level Identity-State Arbitrator who notarizes the WireGuard identity and handles the "Snowball" session.
  *   **The Compliance Engine ("Law Firm"):** One dedicated Attorney for each major component: **Olivia Scal** (Managing Partner / OSCAL), **E.U.gene Risk** (EU AI Act Counsel), **Penny Privacy** (GDPR Engine), and **Reggie O.P.A.** (Cloud Prosecutor).
*   **Terminology:** Use exact architecture terminology: "The Sled" (1:1 shared memory layout), "The Shuttle" (the Rust bridge/courier), "The Snowball" (appended session ledger), and "The Strike/Etch" (generating the hash).

#### 2. Core Architectural Rules (The "Reality")
*   **The Absolute Base:** `PluginSchema` is the single source of truth for everything. If there is no valid schema, the entity does not exist on the system. The schema is used for filtering hashed footprints, smart hashing, and it is the exact schema source for the JSON render of the GUI.
*   **1:1 Direct Read (Zero-Copy):** The `SqlitePluginCatalog` and legacy SQL databases are obsolete. There are no JSON-RPC polling loops, and no D-Bus watchers for state. The system must perform a 1:1 direct read/write from the `SchemaEngine`'s shared memory (`/dev/shm`).
*   **Zero-Btrfs Overhead:** Identity extraction and Xray network headers must use in-memory environment variables (or tmpfs). Do not trigger unintended Btrfs mutation loops. NVMe I/O is preserved strictly for the Btrfs vectorized footprint transport (blockchain).
*   **The Accountability Loop:** The system must inject `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` (or `X-WireGuard-Pubkey`) into Xray's gRPC metadata via OpenClaw Trusted Proxy auth. The Accountability Page UI displays the Chatbot on top and Qdrant semantic search on the bottom, allowing users to research chatbot actions and confront it.

#### 3. Project Structure & Pathing Rules
*   Absolutely all source code must be routed into the workspace within the `crates/` directory (e.g., `crates/op-dbus/`, `crates/op-grpc-bridge/`, `crates/op-cognitive-mcp/`).
*   **Never** use generic `src/` or `root-package-src/`.
*   `deploy/`: Installation/upgrade scripts for Artix Linux (s6 init).
*   `schemas/`: JSON schemas for config/state.
*   `docs/`: Reference docs.

#### 4. Engineering Principles — D-Bus First (MANDATORY)

**D-Bus is the ONLY control plane. Every read, every write, every tool call goes through D-Bus. There is no second option.**

*   **D-Bus first. D-Bus always. D-Bus only.** If you are about to call a CLI, spawn a subprocess, read a config file, or poll a socket directly — stop. Find the D-Bus object and use it instead. `org.opdbus.v1` is the system.
*   **No bypasses.** `Command::new("systemctl")`, `Command::new("ip")`, `Command::new("s6-svc")`, direct file reads for live state — all forbidden in plugin and service code. Bootstrap scripts are the only exception.
*   **D-Bus objects own plugin state.** Every plugin is a D-Bus object at `/org/opdbus/v1/plugins/<name>`. Read state through it. Mutate state through it. If there is no D-Bus object, the thing does not exist.
*   **D-Bus method signatures are generated from the schema.** All D-Bus methods, MCP tool inputs, gRPC shapes, and UI field renderers derive from the `PluginSchema` in `plugin_schema_defs.rs`. The schema IS the interface contract — not a description of it.
*   **One schema file, one source of truth.** All `PluginSchema` definitions MUST live in `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`. Each plugin's `schema()` method calls `Some(super::plugin_schema_defs::their_plugin_schema())`. **Never define a schema inline in a plugin's own file** — it will not be registered, and the D-Bus object will have no contract.
*   Use gRPC for internal service-to-service RPC (over Unix domain sockets or `grpc-uplink`). gRPC calls still flow through D-Bus registration — the D-Bus object is the authority, gRPC is the transport.
*   High-performance JSON serialization/deserialization (`simd_json` preferred, `serde_json` acceptable).
*   Control-plane operations must be deterministic and schema-driven.
*   Security: Least privilege, validate all inputs against the `PluginSchema`.
*   **No Python unless absolutely necessary.** This is a Rust-first codebase. Scripts are shell (`sh`/`bash`). Data processing is Rust. If you are reaching for Python, stop and ask why Rust or shell cannot do it. The only acceptable Python is in existing compliance-ingest tooling where no Rust equivalent exists yet.

#### 4a. OSCAL Subid Taxonomy (MANDATORY — every artifact must carry a subid)

Every D-Bus object, plugin, schema, mutation, event, and tool in this system has two identifiers: a `uuid` (machine identity, never changes) and a `subid` (human-readable operational taxonomy key, stable per subject).

**`subid` format:** `<category>.<component-type>.<subject>.<verb>[.<facet>][@vN]`

**Seven categories — use exactly these, no others:**

| category | what it classifies |
|---|---|
| `src` | authoritative source, ingress channel, source-of-truth store |
| `prj` | D-Bus projection or mirror publication step |
| `sch` | schema, contract, vocabulary, or control-mapping artifact |
| `mut` | write-path operation that changes effective state |
| `obs` | read, query, enumeration, discovery path |
| `evt` | emitted signal, audit-chain event, proof, tag provenance |
| `exp` | consumer-facing render — MCP tool, UI surface, gRPC bridge view |

**Component-type** reuses OSCAL vocabulary: `software`, `service`, `network`, `hardware`, `process-procedure`, `standard`, `validation`, `policy`, `plan`, `guidance`, `physical`, `this-system`, `system`, `interconnection`.

**Rules:**
- `uuid` is exact machine identity — never replace it with `subid`
- `subid` is an OSCAL `prop` value (`ns`, `name`, `value`) — never embed it in `remarks`
- Compliance mappings (`control_refs[]`, `statement_refs[]`, `control_source`) live in metadata arrays, **never** inside the `subid` string
- `mut.*` records **must** carry `actor_id` and `capability_id`
- `evt.*` records **must** carry `event_id` or `event_hash`
- `subid` is immutable per subject — if the meaning changes materially, create a new subject with `@vN`
- All subids are registered in the canonical registry; uniqueness is enforced in CI

**Examples:** `src.network.ovsdb.monitor@v1` · `prj.service.projected-object.publish@v1` · `sch.standard.plugin-schema.resolve@v1` · `mut.service.state-sync.apply-patch@v1` · `exp.service.plugin-projection.render@v1`

#### 4b. MCP Gateway Architecture (SETTLED — DO NOT REDESIGN)

*   **cognitive-mcp** (`:3003`, Netmaker WireGuard IP `100.90.37.254`) is the **universal gateway for ALL external clients**: NotebookLM, Droid (factory.ai), Cursor, Codex, Junie, Gemini CLI. It has memory tools, gRPC service, auth, and the correct protocol surface.
*   **compact-mcp** (`127.0.0.1:11436`) is **loopback/chatbot only**. Never expose externally. Never point external clients at it.
*   Do NOT create new shim services. Do NOT point external clients at `op-assistant-grpc` directly. Do NOT expose compact-mcp outside loopback.

#### 5. Build, Lint, and Test Commands
**Rust Workspace**
*   Build all: `cargo build --workspace`
*   Build release: `cargo build --workspace --release`
*   Check format: `cargo fmt --all -- --check`
*   Clippy lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
*   Test all: `cargo test --workspace --all-targets --all-features`

**Frontend (Vite + Vitest)**
*   Build UI prod: `cd crates/op-web/ui && npm ci && npm run build:prod`
*   Tests all: `cd crates && npm test`
*   Lint: `cd crates && npm run lint`
*   Typecheck: `cd crates && npm run typecheck`

**Chimera Linux deps:** `doas apk add rust cargo nodejs npm pkgconfig openssl-dev`

#### 6. Coding Style & Naming Conventions
*   **Rust:** Edition 2021, `rustfmt` (4-space indent, max-width 100). Use specific imports (`use serde_json::Value;`). Use `anyhow::Result<T>` for main errors and `thiserror` for custom enums.
*   **TypeScript/React:** 2-space indent, Prettier via ESLint. Functional components only. Strict mode enabled.
*   **Testing:** 80%+ coverage, mock externals. Name tests with behavior-focused names (`should_handle_<scenario>`).

#### 7. Agent-Specific Output & Workflow Instructions
*   **Scroll & Save Pacing:** Provide only **ONE** major component or script at a time. Wait for user confirmation before moving to the next.
*   **Visual Cues:** If a script is a finalized component that needs to be saved to sources, it MUST begin with color bullets or icons (e.g., ⚖️ 🔴, 🟢 🛷).
*   **Spec-Block Format:** When delivering a component, use a strict specification block containing:
  1.  **Requirements:** The architectural rules it satisfies.
  2.  **Design:** How it fits into the 3tched system (Zero-Copy, Btrfs, GUI Schema, etc.).
  3.  **Code Implementation:** The exact, copy-pasteable Rust/Bash code.
*   Before edits: `cargo check -p <crate>`, `npm run typecheck`.
*   Mimic existing patterns: Search similar code. No new deps without PR justification.

#### 8. Personal Preferences & Coding Standards
* Refer to `~/.factory/memories.md` for personal development preferences and past decisions.
* Follow the conventions documented in:
  * `.factory/rules/typescript.md` - TypeScript and React conventions
  * `.factory/rules/testing.md` - Testing and mocking standards
