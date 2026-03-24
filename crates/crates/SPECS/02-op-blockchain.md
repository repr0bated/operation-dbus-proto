# op-blockchain - Specification

## Overview
**Crate**: `op-blockchain`  
**Location**: `crates/op-blockchain`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-blockchain"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-blockchain/src/streaming_blockchain.rs
op-blockchain/src/snapshot.rs
op-blockchain/src/retention.rs
op-blockchain/src/plugin_footprint.rs
op-blockchain/src/lib.rs
op-blockchain/src/footprint.rs
op-blockchain/src/btrfs_numa_integration.rs
op-blockchain/src/blockchain.rs
```

### Key Dependencies
```toml
op-core = { workspace = true }
op-cache = { path = "../op-cache" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
sha2 = { workspace = true }
gethostname = { workspace = true }

default = []
ml = []
```

### Binaries
```toml
# No binaries
```

### Features
```toml
[features]
default = []
ml = []
```

## Documentation Files
SPEC.md

## Module Structure
       8 Rust source files

### Main Modules
streaming_blockchain
snapshot
retention
plugin_footprint
footprint
btrfs_numa_integration
blockchain

## Purpose
Streaming blockchain with BTRFS subvolumes for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-cache

---
*Generated from crate analysis*
