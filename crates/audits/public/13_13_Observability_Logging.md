# Observability, Quality, and Security Audit Report

This audit is bounded strictly to the configuration and dependency manifest files provided: `Cargo.toml` and `Cargo.lock`. No Rust source files (`.rs`) were provided in the input; therefore, runtime execution, macro invocation counts, and dynamic behaviors cannot be audited directly. The findings below represent static analysis of dependency topologies, configuration vulnerabilities, and schema compliance.

---

## 1. Macro Counts (Tracing vs println!)

As no `.rs` files are present in the provided file list, dynamic or static occurrences of macros in application code cannot be measured. Based strictly on the provided codebase files:

*   **`tracing::` macros (`info!`, `warn!`, `error!`, `debug!`)**: 0 occurrences (No source code provided)
*   **`println!` macros**: 0 occurrences (No source code provided)

### Dependency Manifest Observations
*   **Tracing Framework Ecosystem**: The workspace establishes `tracing = "0.1"` (`Cargo.toml:111`) and `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }` (`Cargo.toml:112`) as the standard logging facility.
*   **Legacy Log Bridge**: `log = "0.4"` is pulled in as a workspace dependency (`Cargo.toml:144`). This indicates a mixed-ecosystem runtime where library-level `log` macros are bridged into the `tracing` subscriber framework.

---

## 2. Error Handling & Swallow Detection

Because no Rust source code is available in the `FILES` section, we cannot identify specific code locations where errors are caught and swallowed without logging (e.g., `if let Err(_) = ... {}`).

However, we identify a architectural risk in the dependency profile:
*   **`anyhow = "1"` (`Cargo.toml:107`) vs `thiserror = "1"` (`Cargo.toml:108`)**: The workspace permits both ad-hoc error erasing (`anyhow`) and structured error definition (`thiserror`). In production observability patterns, overuse of `anyhow` in internal library crates (such as `op-core`, `op-state-store`, `op-network`) results in swallowed context and un-parseable errors at the service boundaries.

---

## 3. PII and Secret Exposure in Logs

Since actual logging statements cannot be inspected, we analyze the serialization and payload processing dependencies in `Cargo.toml` to identify crates handling sensitive parameters where accidental log formatting presents a leak risk.

### Risk Vectors: Cryptography and Authentication Payloads
*   **Argon2 and Password Hashing**: `argon2 = "0.5"` (`Cargo.toml:141`) is integrated into `op-gateway` and `op-state`. If log statements print configuration contexts, database connection strings, or user records using `{:?}` (Debug formatting) rather than clean, redacting structs, raw password hashes or salt parameters could be printed to standard output.
*   **AES-GCM Secret Key Material**: `aes-gcm = "0.10"` (`Cargo.toml:140`) is a dependency of `op-state`. Any debugging context around cryptographic state storage must be carefully gated to prevent logging initialization vectors (IVs) or plaintext structures.
*   **JSON Web Tokens**: `jsonwebtoken` (`Cargo.lock` dependency of `op-llm`) processes private/public keys and claims payloads. Printing un-sanitized client tokens or session keys during token validation failures is a high-risk vector.

---

## 4. Metrics Instrumentation

The manifest files configure standard metrics instrumentation engines to track runtime telemetry.

### Configured Telemetry Engines
*   **Prometheus**: `prometheus = { version = "0.13", features = ["process"] }` (`Cargo.toml:148`) is declared as a workspace dependency. 
    *   This is pulled into `op-execution-tracker` and `op-state-store` to instrument and export low-level metrics.
    *   The `process` feature flags collection of system-level descriptors (CPU, memory, file descriptors), which is highly suited for control-plane systems.
*   **OpenTelemetry**: `opentelemetry = { version = "0.22", features = ["metrics", "trace"] }` (`Cargo.toml:147`) is declared.
    *   This provides standard APIs for semantic-convention metrics collection and distributed tracing.
    *   It is consumed by `op-state-store`, ensuring that database operations can be monitored across process boundaries.

---

## 5. Schema-as-Code Compliance

This codebase utilizes a combination of system interface specifications and schema tools. We audit the alignment of schema boundaries versus ad-hoc data structures.

### Protocol Buffers (Prost / gRPC)
The workspace extensively enforces structured schemas for inter-service communication:
*   `prost = "0.13"` (`Cargo.toml:128`) and `prost-types = "0.13"` (`Cargo.toml:129`) are established.
*   These dependencies are explicitly active in:
    *   `op-cache`
    *   `op-chat`
    *   `op-cognitive-mcp`
    *   `op-grpc-bridge`
    *   `op-mcp`
    *   `op-mcp-proxy`
    *   `op-projection`
    *   `op-services`
*   **Status**: **Compliant**. Data contracts between internal modules are expressed using compiled Proto schemas rather than ad-hoc JSON or YAML parsing.

### JSON Schema Validation
For configuration and policy-level validation, the workspace utilizes schema validation instead of loose typings:
*   `jsonschema = { version = "0.29", default-features = false }` (`Cargo.toml:86`) is established.
*   It is consumed in `op-dbus`, `op-compliance`, `op-state-store`, and `op-tools`.
*   **Status**: **Compliant**. System configurations and JSON payloads crossing validation boundaries are validated against defined schemas.

### DBus Interface Definitions
The project targets Linux control-plane systems and integrates with system DBus interfaces:
*   `zbus = { version = "5.12", features = ["tokio"] }` (`Cargo.toml:89`) and `zbus_xml = "4.0"` (`Cargo.toml:90`) are defined.
*   **Ad-Hoc Risk**: Any DBus methods or structural exchanges that do not auto-generate contracts from XML specifications run the risk of introducing ad-hoc serialization failures. The presence of `zbus_xml` indicates standard compliance, but verification is required in code to ensure ad-hoc strings are not used in place of validated interface definitions.

---

## 6. Detailed Quality and Configuration Findings

### [Warning] Outdated Dependency Configurations (Reqwest Client Mismatch)
*   **File**: `Cargo.toml:98`
*   **Description**: The workspace defines `reqwest = { version = "0.11", features = ["json", "stream"] }` (`Cargo.toml:98`). However, individual packages within the workspace override or import alternate major versions of `reqwest`. For instance, `op-mcp-proxy` imports `reqwest = "0.12.28"` (`Cargo.lock`), while other crates remain on version `0.11`.
*   **Impact**: This forces the compilation of two concurrent versions of `reqwest` (v0.11 and v0.12), pulling in different versions of the underlying HTTP client (`hyper 0.14` and `hyper 1.0` / `h2 0.3` and `h2 0.4`), which significantly increases compile times, binary footprint, and complicates TLS/connection pool configuration in memory.

### [Warning] Coexistence of Standardized and Custom DBus Packages
*   **File**: `Cargo.toml:89`
*   **Description**: The workspace references `zbus = { version = "5.12", ... }` (`Cargo.toml:89`), but internal crates pull in distinct minor versions of `zbus` (e.g., `zbus 4.4.0` in `op-agents` and `zbus 5.13.2` in `op-identity`, as shown in `Cargo.lock`).
*   **Impact**: Telemetry registration, context propagation, and error handlers will not interoperate smoothly if traits or core types mismatch across minor versions. Telemetry spans cannot easily cross the DBus boundary if library traits are incompatible.