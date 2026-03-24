# op-cognitive-mcp - Specification

## Overview
**Crate**: `op-cognitive-mcp`  
**Location**: `crates/op-cognitive-mcp`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-cognitive-mcp"
version = "0.1.0"
edition = "2021"

[dependencies]
```

### Source Structure
```
op-cognitive-mcp/src/cognitive_tools.rs
op-cognitive-mcp/src/main.rs
op-cognitive-mcp/src/lib.rs
op-cognitive-mcp/src/memory_store.rs
op-cognitive-mcp/src/server.rs
```

### Key Dependencies
```toml
op-core = { path = "../op-core" }
op-mcp = { path = "../op-mcp" }
op-dynamic-loader = { path = "../op-dynamic-loader" }
op-cache = { path = "../op-cache" }
serde = { version = "1.0", features = ["derive"] }
simd-json = { workspace = true }
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
tracing = "0.1"
axum = { version = "0.7", features = ["json", "http2"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors"] }
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
async-trait = "0.1"
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


## Module Structure
       5 Rust source files

### Main Modules
cognitive_tools
memory_store
server

## Purpose


## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:
- op-core
- op-mcp
- op-dynamic-loader
- op-cache

---
*Generated from crate analysis*
