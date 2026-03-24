# op-dbus-mirror - Specification

## Overview
**Crate**: `op-dbus-mirror`  
**Location**: `crates/op-dbus-mirror`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-dbus-mirror"
version = "1.0.0"
edition = "2021"
description = "1:1 D-Bus projection of internal databases (OVSDB, NonNet)"
```

### Source Structure
```
op-dbus-mirror/src/lib.rs
op-dbus-mirror/src/object.rs
op-dbus-mirror/src/tree.rs
op-dbus-mirror/src/dbus_interface.rs
op-dbus-mirror/src/bin/verify_performance.rs
```

### Key Dependencies
```toml
op-core = { path = "../op-core" }
op-jsonrpc = { path = "../op-jsonrpc" }
anyhow = "1"
tokio = { version = "1", features = ["full"] }
zbus = { version = "4.0", features = ["tokio"] }
serde = { version = "1", features = ["derive"] }
simd-json = { version = "0.13", features = ["serde"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "json"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
futures = "0.3"
async-trait = "0.1"
dashmap = "5.0"
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
       5 Rust source files

### Main Modules
object
tree
dbus_interface

## Purpose
1:1 D-Bus projection of internal databases (OVSDB, NonNet)

## Build Information
- **Edition**: 2021
- **Version**: 1.0.0
- **License**: 

## Related Crates
Internal dependencies:
- op-core
- op-jsonrpc

---
*Generated from crate analysis*
