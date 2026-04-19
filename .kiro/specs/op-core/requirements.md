# op-core Requirements

## Problem Statement
The Operation D-Bus system needs a foundational layer that defines common types, error handling, and utilities used across all components. This ensures consistency in communication, security models, and system-wide configurations.

## Functional Requirements

### FR-1: Common Type System
- Define core data structures for system-wide use.
- Implement shared traits for message passing and serialization.
- Provide unified identity representation (`SelfIdentity`).

### FR-2: Unified Error Handling
- Centralized error types using `thiserror`.
- Consistent error propagation across crate boundaries.
- Support for `anyhow` in high-level application logic.

### FR-3: Connection Management
- Utilities for establishing and managing D-Bus connections via `zbus`.
- Support for both System and Session bus types.

### FR-4: Security and Execution
- Define security levels and permission models.
- Provide execution context and tracking capabilities (integrating with `op-execution-tracker`).

### FR-5: Configuration
- Standardized configuration loading and validation.
- Support for environment-based and file-based settings.

## Non-Functional Requirements

### NFR-1: Performance
- Zero-cost abstractions where possible.
- Use of `simd-json` for high-performance JSON operations.

### NFR-2: Maintainability
- Minimal dependencies to keep the core light.
- Clear module separation.

### NFR-3: Reliability
- Strong typing to prevent runtime errors.
- Comprehensive error context for debugging.
