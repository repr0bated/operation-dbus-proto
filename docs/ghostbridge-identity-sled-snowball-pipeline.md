# Ghostbridge Identity Sled + Snowball Accountability Pipeline (Recovered + As-Built)

**Status**: Long-standing core architecture. Design artifacts partially erased in 783dced7; implementation and intent preserved in code and recoverable commits.

**Date of recovery**: 2026-07-03 (from pre-783dced7 .kiro specs + live sources).

## Core Principle (Absolute Base)

`PluginSchema` is the single source of truth.
No valid schema => entity does not exist on the system.

Identity, mutations, traces, and accountability are always bound to the current schema state via a cryptographic footprint.

## The Model (Settled)

- **Identity is the WireGuard public key** (Curve25519). It is persistent for the account lifetime.
- **The Sled** (`/dev/shm/plugin_schema.dat`) is a `#[repr(C)]` 152-byte structure (IdentitySled):
  - `wireguard_pubkey: [u8; 32]`
  - `mutation_index: u64`
  - `hashed_footprint: [u8; 32]` (Blake3)
  - `trace_id: [u8; 16]`
  - `schema_version`, `vector_id` (Qdrant episode), reserved.
- **A.N.N.A. Scribe** (`op-identity/src/anna_scribe.rs`): Notarizes arrival. On WireGuard identity presentation:
  - Performs 1:1 direct read of the sled.
  - Validates sled (non-zero footprint + trace_id).
  - Does the **Strike/Etch**: blake3(wg_pubkey + schema_catalog_blob + mutation_index).
  - Produces genesis **Snowball** ledger entry (`SessionLedger`).
- **The Shuttle / Schema Bridge** (`op-identity/src/schema_bridge.rs`):
  - SchemaEngine writes the sled on relevant schema mutations (zero-copy / tmpfs only).
  - Readers (Shuttle) mmap once, cast, extract `GB_FOOTPRINT` / `GB_TRACE_ID` (optionally WG pubkey) into env.
  - Writes stateless Xray config (`/dev/shm/xray-ghostbridge.json`) and hands off.
  - Strictly avoids any disk I/O that would hit Btrfs.
- **Xray** (on the host, attached to the OVS datapath):
  - Receives GB_* env vars.
  - Injects into gRPC metadata on outbound:
    - `X-Ghostbridge-Footprint`
    - `X-Ghostbridge-Trace-ID`
    - `X-WireGuard-Pubkey` (when applicable).
- **GhostbridgeInterceptor** (`op-grpc-bridge/src/interceptor.rs` + similar in other crates):
  - Enforces the Accountability Loop on every gRPC ingress (port 8090).
  - Requires the two Ghostbridge headers.
  - Re-reads the live sled (zero-copy).
  - Rejects on:
    - Missing headers.
    - Invalid sled state (per Absolute Base).
    - Footprint mismatch (client out of sync with current mutation_index / Btrfs state).
  - Passes validated `trace_id` downstream via request extensions for GUI / Qdrant linkage.
- **Snowball Session Ledger**: Appended audit chain starting with the genesis record from A.N.N.A. Scribe. Lives in shared memory; can be streamed / projected.

Mutations that change schema state update `mutation_index` + footprint. Every subsequent call carries a fresh proof that the client is synchronized with the authoritative mutation history.

## Relation to PluginSchema and Blobs (2026 evolution)

- The sled/footprint **binds identity to the schema state** (hash of canonicalized PluginSchema / catalog).
- Modern bridge uses **per-plugin `PluginObjectBlob`** (see `docs/schema-coupled-plugin-blob-reflection-whitepaper.md`).
- Blobs copy `D-Bus identity + gRPC identity + method metadata from PluginSchema.methods`.
- Restoration of typed `MethodDecl`, `SideEffect`, `SignalDecl`, etc. (from earlier recovery of op-state-store/plugin_schema) directly feeds richer blob synthesis and reflection.

## Key Invariants (AGENTS + recovered specs)

- D-Bus is the only control plane.
- 1:1 direct read (zero-copy mmap in /dev/shm) — no SQL polling, no generic D-Bus watchers for state.
- Zero-Btrfs overhead for the identity/accountability path (NVMe preserved for blockchain vectorized transport).
- Schema drives everything: if no valid PluginSchema, no identity, no service, nothing exists.
- Persistent WG-key identity (account lifetime), not ephemeral SQL sessions.
- Full accountability loop: WG handshake → AnnaScribe Snowball → Xray header injection → Interceptor enforcement → trace propagation.

## Recovered Artifacts (from 783dced7^)

- `.kiro/specs/3tched-schema-shuttle-xray-pipeline/{design,requirements,tasks}.md`
  - Detailed phased implementation (Sled struct/layout, Shuttle zero-copy + env injection + disk-I/O abort, GhostbridgeInterceptor, JSON-RPC Mutation Pipeline stages, AI Accountability).
- Consistent terminology: The Sled, The Shuttle, Strike/Etch, The Snowball, A.N.N.A. Scribe, Accountability Loop, Absolute Base.

Current code in `op-identity`, `op-grpc-bridge`, `op-projection`, `op-cognitive-mcp`, etc. implements the above with only minor natural extensions (Qdrant vector_id, subid taxonomy, per-object blobs).

## Current Primary Files (as-built)

- `crates/op-identity/src/schema_bridge.rs` — IdentitySled definition, read_sled / write_*, Shuttle bridge, env + xray config emission.
- `crates/op-identity/src/anna_scribe.rs` — notarize_arrival, SessionLedger, etch_footprint, validation.
- `crates/op-grpc-bridge/src/interceptor.rs` — GhostbridgeInterceptor enforcement.
- Plugin blob path: `crates/op-grpc-bridge/src/{plugin_object_blob.rs, dynamic_reflection.rs, zeroclaw_object_blob.rs, grpc_server.rs}` — uses schema-derived methods.

## Open Alignment Work (current session)

- Finish propagating restored `MethodDecl` / typed methods into `PluginSchema` everywhere + into per-plugin blob synthesis.
- Produce or refresh living design docs from the material recovered here so future erasures don't lose the intent again.
- Ensure all call sites (cognitive-mcp, projection, web, etc.) consistently carry / respect the ghostbridge headers and sled validation.

This architecture has been stable in intent since well before the 783dced7 consolidation. The erased .kiro material + handoffs are now partially restored.
