# op-jsonrpc - Specification

## Overview
**Crate**: `op-jsonrpc`  
**Location**: `crates/op-jsonrpc`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-jsonrpc"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-jsonrpc/src/server.rs
op-jsonrpc/src/protocol.rs
op-jsonrpc/src/ovsdb_jsonrpc.rs
op-jsonrpc/src/ovsdb.rs
op-jsonrpc/src/nonnet_staging.rs
op-jsonrpc/src/nonnet.rs
op-jsonrpc/src/lib.rs
```

### Key Dependencies
```toml
op-core = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files
SPEC.md

## Module Structure
       7 Rust source files

### Main Modules
server
protocol
ovsdb_jsonrpc
ovsdb
nonnet_staging
nonnet

## Purpose
JSON-RPC server with OVSDB and NonNet database support for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
