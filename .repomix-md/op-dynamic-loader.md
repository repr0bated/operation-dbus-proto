This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-dynamic-loader/
              src/
                dynamic_registry.rs
                error.rs
                execution_aware_loader.rs
                lib.rs
                loading_strategy.rs
              Cargo.toml
              compare-op-dynamic-loader.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/src/dynamic_registry.rs">
use anyhow::Result;
use async_trait::async_trait;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::DynamicLoaderError;
use crate::loading_strategy::LoadingStrategy;
use op_execution_tracker::{ExecutionContext, ExecutionTracker};
use op_tools::{BoxedTool, ToolRegistry};

/// Dynamic tool registry that wraps existing registry with caching
pub struct DynamicToolRegistry {
    /// Underlying tool registry (existing functionality)
    base_registry: Arc<ToolRegistry>,

    /// Execution tracker for load decisions
    execution_tracker: Arc<ExecutionTracker>,

    /// Loading strategy
    loading_strategy: Arc<dyn LoadingStrategy>,

    /// LRU cache for loaded tools
    tool_cache: Arc<RwLock<LruCache<String, BoxedTool>>>,

    /// Cache statistics
    cache_hits: Arc<RwLock<u64>>,
    cache_misses: Arc<RwLock<u64>>,
}

impl DynamicToolRegistry {
    /// Create new dynamic registry
    pub fn new(
        base_registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
        loading_strategy: Arc<dyn LoadingStrategy>,
        max_cache_size: usize,
    ) -> Self {
        Self {
            base_registry,
            execution_tracker,
            loading_strategy,
            tool_cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(max_cache_size).unwrap(),
            ))),
            cache_hits: Arc::new(RwLock::new(0)),
            cache_misses: Arc::new(RwLock::new(0)),
        }
    }

    /// Get tool with dynamic loading
    pub async fn get_tool(&self, name: &str, context: &ExecutionContext) -> Result<BoxedTool> {
        // Check cache first
        {
            // LruCache::get requires &mut self to update LRU order
            let mut cache = self.tool_cache.write().await;
            if let Some(tool) = cache.get(name) {
                *self.cache_hits.write().await += 1;
                return Ok(Arc::clone(tool));
            }
        }

        // Tool not in cache - check if we should load it
        if self.loading_strategy.should_load(name, context).await {
            // Load from base registry
            if let Some(tool) = self.base_registry.get(name).await {
                // Cache the tool
                let mut cache = self.tool_cache.write().await;
                cache.put(name.to_string(), tool.clone());

                *self.cache_misses.write().await += 1;
                return Ok(tool);
            }
        }

        Err(DynamicLoaderError::ToolNotFound(name.to_string()).into())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (u64, u64) {
        let hits = *self.cache_hits.read().await;
        let misses = *self.cache_misses.read().await;
        (hits, misses)
    }

    /// Get current cache size
    pub async fn get_cache_size(&self) -> usize {
        let cache = self.tool_cache.read().await;
        cache.len()
    }

    /// Clear cache (for testing or memory management)
    pub async fn clear_cache(&self) {
        let mut cache = self.tool_cache.write().await;
        cache.clear();
    }

    /// Get base registry (for compatibility)
    pub fn base_registry(&self) -> Arc<ToolRegistry> {
        Arc::clone(&self.base_registry)
    }

    /// Get execution tracker (for compatibility)
    pub fn execution_tracker(&self) -> Arc<ExecutionTracker> {
        Arc::clone(&self.execution_tracker)
    }
}

/// Enhanced tool registry trait
#[async_trait]
pub trait EnhancedToolRegistry: Send + Sync {
    /// Get tool with dynamic loading
    async fn get_tool(&self, name: &str, context: &ExecutionContext) -> Result<BoxedTool>;

    /// Get cache statistics
    async fn get_cache_stats(&self) -> (u64, u64);

    /// Get current cache size
    async fn get_cache_size(&self) -> usize;
}

#[async_trait]
impl EnhancedToolRegistry for DynamicToolRegistry {
    async fn get_tool(&self, name: &str, context: &ExecutionContext) -> Result<BoxedTool> {
        self.get_tool(name, context).await
    }

    async fn get_cache_stats(&self) -> (u64, u64) {
        self.get_cache_stats().await
    }

    async fn get_cache_size(&self) -> usize {
        self.get_cache_size().await
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/src/error.rs">
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DynamicLoaderError {
    #[error("Tool loading error: {0}")]
    LoadingError(String),

    #[error("Cache eviction error: {0}")]
    CacheError(String),

    #[error("Execution tracking integration error: {0}")]
    TrackingError(String),

    #[error("Tool not found in registry or cache: {0}")]
    ToolNotFound(String),

    #[error("Strategy selection error: {0}")]
    StrategyError(String),
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/src/execution_aware_loader.rs">
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::{DynamicToolRegistry, SmartLoadingStrategy};
use op_execution_tracker::{ExecutionContext, ExecutionTracker};
use op_tools::ToolRegistry;

/// Execution-aware tool loader
pub struct ExecutionAwareLoader {
    /// Dynamic registry
    dynamic_registry: Arc<DynamicToolRegistry>,

    /// Execution tracker
    execution_tracker: Arc<ExecutionTracker>,
}

impl ExecutionAwareLoader {
    /// Create new execution-aware loader
    pub fn new(
        base_registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
        max_cache_size: usize,
    ) -> Self {
        let loading_strategy = Arc::new(SmartLoadingStrategy::new(
            Arc::clone(&execution_tracker),
            300, // 5 minute base TTL
        ));

        let dynamic_registry = Arc::new(DynamicToolRegistry::new(
            base_registry,
            Arc::clone(&execution_tracker),
            loading_strategy,
            max_cache_size,
        ));

        Self {
            dynamic_registry,
            execution_tracker,
        }
    }

    /// Get tool with execution-aware loading
    pub async fn get_tool_with_context(
        &self,
        tool_name: &str,
        context: &ExecutionContext,
    ) -> Result<op_tools::BoxedTool> {
        self.dynamic_registry.get_tool(tool_name, context).await
    }

    /// Get tool with automatic context creation
    pub async fn get_tool(&self, tool_name: &str) -> Result<op_tools::BoxedTool> {
        let context = ExecutionContext::new(tool_name);
        self.get_tool_with_context(tool_name, &context).await
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (u64, u64) {
        self.dynamic_registry.get_cache_stats().await
    }

    /// Get current cache size
    pub async fn get_cache_size(&self) -> usize {
        self.dynamic_registry.get_cache_size().await
    }

    /// Get base registry (for compatibility)
    pub fn base_registry(&self) -> Arc<ToolRegistry> {
        self.dynamic_registry.base_registry()
    }

    /// Get execution tracker (for compatibility)
    pub fn execution_tracker(&self) -> Arc<ExecutionTracker> {
        Arc::clone(&self.execution_tracker)
    }
}

/// Execution-aware tool registry trait
#[async_trait]
pub trait ExecutionAwareToolRegistry: Send + Sync {
    /// Get tool with execution context
    async fn get_tool_with_context(
        &self,
        tool_name: &str,
        context: &ExecutionContext,
    ) -> Result<op_tools::BoxedTool>;

    /// Get tool with automatic context
    async fn get_tool(&self, tool_name: &str) -> Result<op_tools::BoxedTool>;

    /// Get cache statistics
    async fn get_cache_stats(&self) -> (u64, u64);
}

#[async_trait]
impl ExecutionAwareToolRegistry for ExecutionAwareLoader {
    async fn get_tool_with_context(
        &self,
        tool_name: &str,
        context: &ExecutionContext,
    ) -> Result<op_tools::BoxedTool> {
        self.get_tool_with_context(tool_name, context).await
    }

    async fn get_tool(&self, tool_name: &str) -> Result<op_tools::BoxedTool> {
        self.get_tool(tool_name).await
    }

    async fn get_cache_stats(&self) -> (u64, u64) {
        self.get_cache_stats().await
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/src/lib.rs">
//! OP Dynamic Loader - Intelligent Tool Loading Enhancement
//!
//! Complements existing MCP tool loading by adding:
//! - LRU caching for frequently used tools
//! - Execution-aware loading decisions
//! - Integration with execution tracking
//! - Memory-efficient tool management

pub mod dynamic_registry;
pub mod error;
pub mod execution_aware_loader;
pub mod loading_strategy;

pub use dynamic_registry::DynamicToolRegistry;
pub use error::DynamicLoaderError;
pub use execution_aware_loader::ExecutionAwareLoader;
pub use loading_strategy::{LoadingStrategy, SmartLoadingStrategy};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/src/loading_strategy.rs">
use async_trait::async_trait;
use std::sync::Arc;

use op_execution_tracker::{ExecutionContext, ExecutionTracker};

/// Loading strategy interface
#[async_trait]
pub trait LoadingStrategy: Send + Sync {
    /// Determine if a tool should be loaded
    async fn should_load(&self, tool_name: &str, context: &ExecutionContext) -> bool;

    /// Get load priority (0-100)
    async fn get_priority(&self, tool_name: &str) -> u8;

    /// Get cache TTL in seconds
    fn cache_ttl(&self, tool_name: &str) -> u64;
}

/// Smart loading strategy that considers execution patterns
pub struct SmartLoadingStrategy {
    execution_tracker: Arc<ExecutionTracker>,
    base_cache_ttl: u64,
}

impl SmartLoadingStrategy {
    pub fn new(execution_tracker: Arc<ExecutionTracker>, base_cache_ttl: u64) -> Self {
        Self {
            execution_tracker,
            base_cache_ttl,
        }
    }
}

#[async_trait]
impl LoadingStrategy for SmartLoadingStrategy {
    async fn should_load(&self, tool_name: &str, _context: &ExecutionContext) -> bool {
        // Always load if it's a critical tool
        if self.is_critical_tool(tool_name) {
            return true;
        }

        // Check recent execution history
        let recent_executions = self.execution_tracker.list_recent_completed(10).await;

        let recent_tool_executions = recent_executions
            .iter()
            .filter(|exec| exec.tool_name == tool_name)
            .count();

        // Load if recently used (last 10 executions)
        if recent_tool_executions > 0 {
            return true;
        }

        // Default: load on-demand
        true
    }

    async fn get_priority(&self, tool_name: &str) -> u8 {
        if self.is_critical_tool(tool_name) {
            return 100;
        }

        // Check execution frequency
        let recent_executions = self.execution_tracker.list_recent_completed(50).await;

        let tool_executions = recent_executions
            .iter()
            .filter(|exec| exec.tool_name == tool_name)
            .count();

        // Priority based on usage frequency
        match tool_executions {
            0..=2 => 20, // Low priority
            3..=5 => 50, // Medium priority
            _ => 80,     // High priority
        }
    }

    fn cache_ttl(&self, tool_name: &str) -> u64 {
        if self.is_critical_tool(tool_name) {
            // Critical tools stay loaded longer
            self.base_cache_ttl * 2
        } else {
            self.base_cache_ttl
        }
    }
}

impl SmartLoadingStrategy {
    fn is_critical_tool(&self, tool_name: &str) -> bool {
        // Define critical tools that should always be available
        let critical_tools = [
            "respond_to_user",
            "cannot_perform",
            "systemd_status",
            "file_read",
            "agent_status",
        ];

        critical_tools.contains(&tool_name)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/Cargo.toml">
[package]
name = "op-dynamic-loader"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Dynamic Tool Loading Enhancement - Complements existing MCP tool loading with intelligent caching and execution tracking"

[dependencies]
tokio = { workspace = true, features = ["full"] }
serde = { workspace = true, features = ["derive"] }
simd-json = { workspace = true }
chrono = { workspace = true, features = ["serde"] }
uuid = { workspace = true, features = ["v4", "serde"] }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
lru = { workspace = true }
anyhow = { workspace = true }

# Internal dependencies
op-core = { path = "../op-core" }
op-tools = { path = "../op-tools" }
op-execution-tracker = { path = "../op-execution-tracker" }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/compare-op-dynamic-loader.md">
# compare-op-dynamic-loader

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 5 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 0 |
| Spec-listed source files | 5 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- Dynamic Tool Loading Enhancement - Complements existing MCP tool loading with intelligent caching and execution tracking
- Internal crate integrations: op-core, op-tools, op-execution-tracker.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/loading_strategy.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/loading_strategy.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/execution_aware_loader.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/execution_aware_loader.rs |
| `src/error.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/error.rs |
| `src/dynamic_registry.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dynamic_registry.rs |
| `root` | ✅ Present | root source group | src/dynamic_registry.rs, src/error.rs, src/execution_aware_loader.rs, src/lib.rs, src/loading_strategy.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| loading_strategy | ✅ Implemented | src/loading_strategy.rs | SPEC main module |
| execution_aware_loader | ✅ Implemented | src/execution_aware_loader.rs | SPEC main module |
| error | ✅ Implemented | src/error.rs | SPEC main module |
| dynamic_registry | ✅ Implemented | src/dynamic_registry.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-tools` - documented in SPEC
- `op-execution-tracker` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `lru` - documented in SPEC
- `anyhow` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: dynamic_registry, error, execution_aware_loader, loading_strategy.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dynamic-loader/SPEC.md">
# op-dynamic-loader - Specification

## Overview
**Crate**: `op-dynamic-loader`  
**Location**: `crates/op-dynamic-loader`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-dynamic-loader"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Dynamic Tool Loading Enhancement - Complements existing MCP tool loading with intelligent caching and execution tracking"
```

### Source Structure
```
op-dynamic-loader/src/loading_strategy.rs
op-dynamic-loader/src/lib.rs
op-dynamic-loader/src/execution_aware_loader.rs
op-dynamic-loader/src/error.rs
op-dynamic-loader/src/dynamic_registry.rs
```

### Key Dependencies
```toml
tokio = { workspace = true, features = ["full"] }
serde = { workspace = true, features = ["derive"] }
simd-json = { workspace = true }
chrono = { workspace = true, features = ["serde"] }
uuid = { workspace = true, features = ["v4", "serde"] }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
lru = { workspace = true }
anyhow = { workspace = true }

# Internal dependencies
op-core = { path = "../op-core" }
op-tools = { path = "../op-tools" }
op-execution-tracker = { path = "../op-execution-tracker" }
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
loading_strategy
execution_aware_loader
error
dynamic_registry

## Purpose
Dynamic Tool Loading Enhancement - Complements existing MCP tool loading with intelligent caching and execution tracking

## Build Information
- **Edition**: edition.workspace = true
- **Version**: 0.1.0
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-core
- op-tools
- op-execution-tracker

---
*Generated from crate analysis*
</file>

</files>
