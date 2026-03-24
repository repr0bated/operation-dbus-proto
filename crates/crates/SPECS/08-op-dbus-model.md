# op-dbus-model - Specification

## Overview
**Crate**: `op-dbus-model`  
**Location**: `crates/op-dbus-model`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-dbus-model"
version = "0.1.0"
edition = "2021"

[dependencies]
```

### Source Structure
```
op-dbus-model/src/models.rs
op-dbus-model/src/lib.rs
```

### Key Dependencies
```toml
serde = { version = "1.0", features = ["derive"] }
simd-json = { workspace = true }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "json"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
thiserror = "1.0"
anyhow = "1.0"
op-core = { path = "../op-core" }
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
       2 Rust source files

### Main Modules
models

## Purpose


## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:
- op-core

---
*Generated from crate analysis*
