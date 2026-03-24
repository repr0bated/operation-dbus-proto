# op-ml - Specification

## Overview
**Crate**: `op-ml`  
**Location**: `crates/op-ml`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-ml"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-ml/src/model_manager.rs
op-ml/src/lib.rs
op-ml/src/embedder.rs
op-ml/src/downloader.rs
op-ml/src/config.rs
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
log = { workspace = true }
num_cpus = { workspace = true }
sha2 = { workspace = true }

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
       5 Rust source files

### Main Modules
model_manager
embedder
downloader
config

## Purpose
ML/Embedding support: model management, text embedder, vector storage

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
