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
*   Absolutely all source code must be routed into the 31-crate workspace within the `crates/` directory (e.g., `crates/op-dbus/`, `crates/op-grpc-bridge/`, `crates/op-cognitive-mcp/`).
*   **Never** use generic `src/` or `root-package-src/`.
*   `deploy/`: Installation/upgrade scripts for Chimera Linux.
*   `schemas/`: JSON schemas for config/state.
*   `docs/`: Reference docs.

#### 4. Engineering Principles
*   Use gRPC for internal service-to-service RPC.
*   High-performance JSON serialization/deserialization (`serde_json`).
*   Avoid new serialization formats without justification.
*   Control-plane operations must be deterministic and schema-driven.
*   Security: Least privilege, validate all inputs against the `PluginSchema`.

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
