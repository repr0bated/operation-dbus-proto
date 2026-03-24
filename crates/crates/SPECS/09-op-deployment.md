# op-deployment - Specification

## Overview
**Crate**: `op-deployment`  
**Location**: `crates/op-deployment`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-deployment"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-deployment/src/lib.rs
op-deployment/src/image_manager.rs
```

### Key Dependencies
```toml
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
reqwest = { workspace = true }
sha2 = { workspace = true }
chrono = { workspace = true }
log = { workspace = true }
uuid = { workspace = true }
tar = { workspace = true }
flate2 = { workspace = true }

tempfile = { workspace = true }
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
image_manager

## Purpose
Container and image deployment management

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
