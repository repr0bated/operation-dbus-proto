# op-mcp-proxy - Specification

## Overview
**Crate**: `op-mcp-proxy`  
**Location**: `crates/op-mcp-proxy`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-mcp-proxy"
version = "0.1.0"
edition = "2021"

[dependencies]
```

### Source Structure
```
op-mcp-proxy/src/session.rs
op-mcp-proxy/src/gcloud_auth.rs
op-mcp-proxy/src/main.rs
op-mcp-proxy/src/cloudaicompanion.rs
op-mcp-proxy/src/direct_llm.rs
```

### Key Dependencies
```toml
op-cache = { path = "../op-cache" }
op-identity = { path = "../op-identity" }
tokio     = { version = "1", features = ["full"] }
tonic     = "0.11"
serde     = { version = "1", features = ["derive"] }
simd-json = { workspace = true }
reqwest   = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tracing   = "0.1"
tracing-subscriber = "0.3"
serde_json = "1"
anyhow    = "1"
dirs      = "5"
hostname  = "0.4"
rusqlite  = { workspace = true, features = ["bundled"] }
chrono    = { version = "0.4", features = ["serde"] }
uuid      = { version = "1.6", features = ["v4", "serde"] }
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
session
gcloud_auth
cloudaicompanion
direct_llm

## Purpose


## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:
- op-cache
- op-identity

---
*Generated from crate analysis*
