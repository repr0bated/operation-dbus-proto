# op-chat - Specification

## Overview
**Crate**: `op-chat`  
**Location**: `crates/op-chat`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-chat"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Chat functionality and LLM integration for op-dbus-v2"
```

### Source Structure
```
op-chat/src/orchestration/proto/op_chat.orchestration.rs
op-chat/src/orchestration/proto/mod.rs
op-chat/src/orchestration/workstacks.rs
op-chat/src/orchestration/workstack_executor.rs
op-chat/src/orchestration/workflows.rs
op-chat/src/orchestration/executor.rs
op-chat/src/orchestration/error.rs
op-chat/src/orchestration/dbus_orchestrator.rs
op-chat/src/orchestration/coordinator.rs
op-chat/src/orchestration/mod.rs
op-chat/src/orchestration/grpc_pool.rs
op-chat/src/orchestration/skills.rs
op-chat/src/actor.rs
op-chat/src/tool_orchestrator.rs
op-chat/src/tool_executor.rs
op-chat/src/system_prompt.rs
op-chat/src/session.rs
op-chat/src/router.rs
op-chat/src/nl_admin.rs
op-chat/src/llm.rs
```

### Key Dependencies
```toml
# Workspace dependencies
tokio = { workspace = true, features = ["full"] }
serde = { workspace = true, features = ["derive"] }
simd-json = { workspace = true }
chrono = { workspace = true, features = ["serde"] }
uuid = { workspace = true, features = ["v4", "serde"] }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
anyhow = { workspace = true }
futures = { workspace = true }

# D-Bus support
zbus = { workspace = true }

# Internal dependencies
op-core = { path = "../op-core" }
op-tools = { path = "../op-tools" }
op-introspection = { path = "../op-introspection" }
op-llm = { path = "../op-llm" }
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
      30 Rust source files

### Main Modules
actor
tool_orchestrator
tool_executor
system_prompt
session
router
nl_admin
llm
intent_executor
hybrid_executor

## Purpose
Chat functionality and LLM integration for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-core
- op-tools
- op-introspection
- op-llm
- op-execution-tracker
- op-agents

---
*Generated from crate analysis*
