# compare-op-blockchain

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 8 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 7 |
| Partial artifacts | 0 |
| Spec-listed source files | 8 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- Streaming blockchain with BTRFS subvolumes for op-dbus-v2
- Internal crate integrations: op-core, op-cache.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/streaming_blockchain.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/streaming_blockchain.rs |
| `src/snapshot.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/snapshot.rs |
| `src/retention.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/retention.rs |
| `src/plugin_footprint.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/plugin_footprint.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/footprint.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/footprint.rs |
| `src/btrfs_numa_integration.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/btrfs_numa_integration.rs |
| `src/blockchain.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/blockchain.rs |
| `root` | ✅ Present | root source group | src/blockchain.rs, src/btrfs_numa_integration.rs, src/footprint.rs, src/lib.rs, src/plugin_footprint.rs, src/retention.rs, src/snapshot.rs, src/streaming_blockchain.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| streaming_blockchain | ✅ Implemented | src/streaming_blockchain.rs | SPEC main module |
| snapshot | ✅ Implemented | src/snapshot.rs | SPEC main module |
| retention | ✅ Implemented | src/retention.rs | SPEC main module |
| plugin_footprint | ✅ Implemented | src/plugin_footprint.rs | SPEC main module |
| footprint | ✅ Implemented | src/footprint.rs | SPEC main module |
| btrfs_numa_integration | ✅ Implemented | src/btrfs_numa_integration.rs | SPEC main module |
| blockchain | ✅ Implemented | src/blockchain.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-cache` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - documented in SPEC
- `sha2` - documented in SPEC
- `gethostname` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: blockchain, btrfs_numa_integration, footprint, plugin_footprint, retention, snapshot, streaming_blockchain.
- Cargo feature flags: default, ml.
