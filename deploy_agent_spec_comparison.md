# Comprehensive Assessment: `.kiro/specs/op-services` vs Current Architecture

This document provides a thorough assessment of the original `op-services` specifications (`requirements.md`, `design.md`, and `tasks.md`) compared to the current, unified `op-dbus` architecture (as documented in `docs/architecture-flow.md`).

## 1. Overview of the Disconnect

The `.kiro/specs/op-services` documents describe a **standalone, stateful service manager daemon** (`op-services`) that manages its own SQLite database, acts as its own gRPC server, and projects its own D-Bus interfaces. 

The current codebase evolved into a **unified Deterministic Control Plane (`op-dbus`)** where:
- State is strictly maintained in authoritative RCP stores (OVSDB and NonNet).
- The `op-dbus` server is the singular gRPC ingress (`10.200.0.2:50051`).
- The D-Bus tree (`org.opdbus.v1`) is a pure 1:1 projection of the RCP stores.
- Local SQLite hoarding by individual plugins or daemons is an explicit anti-pattern.

## 2. Assessment of `requirements.md`

### ✅ What Aligns
- **FR-1: Service Definition Schema:** Services are still treated as declarative schema-as-code, migrating away from imperative systemd units.
- **FR-3: dinit Integration:** The system successfully uses `dinit` as PID 1, and `op-services` communicates with it via the `zbus` D-Bus proxy.
- **NFR-1 / NFR-2:** The lightweight footprint and high-reliability/speed goals of the daemon remain intact.

### ❌ What Violates Current Architecture
- **FR-4: gRPC Interface (Internal):** The spec called for `op-services` to host its own gRPC `ServiceManager`. In the current architecture, **all gRPC ingress goes through `op-dbus`** on port 50051. `op-services` should not be spinning up its own gRPC servers.
- **FR-6: Persistence:** The spec mandates "SQLite for service definitions and state" and an "Audit log of all operations". **This is the biggest violation.** The current architecture demands that `op-services` be completely stateless. Service definitions must live in the authoritative NonNet RCP database, and auditing is handled system-wide by the BTRFS `timing_subvol` blockchain footprints.

## 3. Assessment of `design.md`

### ❌ Architectural Diagram Conflicts
The diagram in `design.md` depicts `op-services` as a monolith containing:
- A gRPC Server (tonic)
- A SQLite Database (sqlx)
- A ServiceRegistry

**Current Reality:** `op-services` should be a thin, stateless reconciliation engine.
- It should receive its target state (ServiceRegistry) by querying `op-dbus` via JSON-RPC or gRPC.
- It should execute those states against `dinit` (via the DinitProxy).
- It must drop the `SQLite (sqlx)` block entirely.

### ❌ Data Schema Conflicts
`design.md` outlines a rigid set of Protobuf definitions (`opdbus.services.v1.ServiceManager`). While the schema shape (`ServiceDef`) is still largely relevant, the transport mechanism is outdated. Instead of a dedicated `ServiceManager` gRPC service running inside `op-services`, mutations and reads should route through the central `op-dbus` `StateSyncServer`.

## 4. Assessment of `tasks.md`

### ✅ Completed & Relevant Tasks
- **Phase 0 & 1:** Crate setup, structure, and schema definitions (`ServiceDef`, `ServiceName`, etc.) are generally well-implemented and remain useful as the core data model.
- **Phase 4 (dinit Integration):** The `DinitProxy` and D-Bus communications are implemented and actively working (verified by logs).

### ❌ Obsolete Tasks (Must Be Reverted/Ignored)
- **Phase 2 (Storage Layer):** "SQLite schema for services table", "Audit log insertion", and CRUD operations for `ServiceDef` inside SQLite. This was fully implemented in `src/store/mod.rs` but **must be ripped out**.
- **Phase 5 (gRPC Interface):** Building a standalone gRPC server for `op-services`. Instead, `op-services` should be a *client* of the central `op-dbus` gRPC/RCP server.

## 5. Actionable Refactoring Plan

To bring `op-services` into compliance with the unified architecture, the following steps are required:

1. **Purge SQLite:** Remove `sqlx` from `Cargo.toml`. Delete `src/store/mod.rs` and any local `.db` file creation logic.
2. **Stateless RCP Client:** Rewrite `op-services` to fetch its desired `ServiceDef` state directly from the `op-dbus` NonNet store via JSON-RPC or gRPC.
3. **Remove Local Auditing:** Strip out the custom `audit_log` logic. If an action occurs, the footprint should be logged centrally by `op-dbus` to the BTRFS blockchain.
4. **Update Specs:** Deprecate or rewrite `.kiro/specs/op-services/*` so that future agents do not attempt to re-implement local SQLite caching.