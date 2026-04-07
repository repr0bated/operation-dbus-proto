# compare-op-dbus-mirror

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 6 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 1 |
| Spec-listed source files | 5 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- 1:1 D-Bus projection of internal databases (OVSDB, NonNet)
- Internal crate integrations: op-core, op-jsonrpc.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/lib.rs` | ⚠️ Partial | Declared in source inventory from spec/design docs | src/lib.rs; partial artifacts: src/lib.rs.orig |
| `src/object.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/object.rs |
| `src/tree.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tree.rs |
| `src/dbus_interface.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dbus_interface.rs |
| `src/bin/verify_performance.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/bin/verify_performance.rs |
| `bin` | ✅ Present | bin group | src/bin/verify_performance.rs |
| `root` | ✅ Present | root source group | src/dbus_interface.rs, src/jsonrpc_interface.rs, src/lib.rs, src/object.rs, src/tree.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| object | ✅ Implemented | src/object.rs | SPEC main module |
| tree | ✅ Implemented | src/tree.rs | SPEC main module |
| dbus_interface | ✅ Implemented | src/dbus_interface.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-jsonrpc` - documented in SPEC

### External Runtime Dependencies
- `anyhow` - documented in SPEC
- `tokio` - documented in SPEC
- `zbus` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `sqlx` - documented in SPEC
- `tracing` - documented in SPEC
- `tracing-subscriber` - documented in SPEC
- `futures` - documented in SPEC
- `async-trait` - documented in SPEC
- `dashmap` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Transitional or partial artifacts detected: src/lib.rs.orig.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: dbus_interface, jsonrpc_interface, object, tree.
