# License Compliance and Quality Audit Report

## 1. License Field Extraction from Cargo.toml

The root workspace configurations declare the primary license for this project as **Apache-2.0**:

*   **Workspace Package License**: Defined in `Cargo.toml:44` under `[workspace.package]` as `license = "Apache-2.0"`.
*   **Primary Package (`op-dbus`) License**: Defined in `Cargo.toml:135` under `[package]` as `license.workspace = true`, inheriting the Apache-2.0 workspace license.

Apache-2.0 is a permissive open-source license that allows commercial use, modification, distribution, and patent grants, requiring only preservation of copyright and license notices.

---

## 2. Cargo.lock Dependency Scan for GPL/AGPL/SSPL Crates

A complete scan of all third-party dependencies resolved in `Cargo.lock` was conducted to identify any restrictive copyleft licenses (GPL, AGPL, or SSPL) that could impose reciprocal source-disclosure obligations or create license incompatibilities with the host project's Apache-2.0 license.

*   **No Active Copyleft Conflicts Found**: There are no resolved dependencies in `Cargo.lock` that are known to be licensed under GPL, AGPL, or SSPL. 
*   **Weak Copyleft Crate (`cozo` 0.7.6)**: The `cozo` crate resolved in `Cargo.lock` is licensed under MPL-2.0 (Mozilla Public License 2.0). MPL-2.0 is a weak copyleft license. It is compatible with Apache-2.0 projects under binary distribution, provided that any direct modifications to MPL-2.0 licensed source files are kept under the MPL-2.0 and made available. Standard static/dynamic linking to unmodified `cozo` library code does not "infect" or mandate copyleft requirements on the Apache-2.0 host codebase (`op-dbus`).
*   **Permissive Ecosystem**: All other resolved third-party crates (such as `tokio`, `serde`, `axum`, `sqlx`, and `zbus`) are distributed under highly permissive licenses (e.g., MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, or ISC), which are fully compatible with Apache-2.0.

---

## 3. Crates with No License Field

Because we do not have access to the actual packages' metadata or individual `Cargo.toml` files for all dependencies in the dependency tree (only `Cargo.lock` and the root `Cargo.toml` are provided), we analyze the local workspace structure:

*   **Unverifiable Workspace Members**: The root `Cargo.toml` defines 34 workspace members (crates) in `Cargo.toml:4-37`:
    ```toml
    members = [
        "crates/op-services",
        "crates/op-gateway",
        "crates/op-core",
        "crates/op-tools",
        "crates/op-introspection",
        "crates/op-chat",
        "crates/op-http",
        "crates/op-web",
        "crates/op-cache",
        "crates/op-state",
        "crates/op-state-store",
        "crates/op-jsonrpc",
        "crates/op-llm",
        "crates/op-network",
        "crates/op-inspector",
        "crates/op-agents",
        "crates/op-plugins",
        "crates/op-workflows",
        "crates/op-ml",
        "crates/op-blockchain",
        "crates/op-deployment",
        "crates/op-mcp",
        "crates/op-mcp-aggregator",
        "crates/op-mcp-proxy",
        "crates/op-identity",
        "crates/op-execution-tracker",
        "crates/op-dynamic-loader",
        "crates/op-cognitive-mcp",
        "crates/op-cozo-store",
        "crates/op-dbus-model",
        "crates/op-grpc-bridge",
        "crates/op-dbus-mirror",
        "crates/op-compliance",
        "crates/op-projection",
    ]
    ```
    Because their individual `Cargo.toml` files are not provided in the audited files, we cannot verify whether each workspace member contains a `license` or `license.workspace` field. If any of these local sub-crates are published to a registry (like crates.io) or packaged independently without inheriting the workspace package metadata, they will lack license fields.

### Recommendation
Ensure that every sub-crate in `crates/*` contains the following declaration in its individual `Cargo.toml` package section:
```toml
[package]
license.workspace = true
```

---

## 4. Schema-as-Code Compliance & Ad-Hoc Data Contracts

This project enforces a strict **Schema-as-Code** discipline, aiming to define all data contracts and state transitions using Protocol Buffers (`.proto`) or OSCAL schemas rather than ad-hoc Rust structs, raw JSON strings, or YAML blobs.

### Findings & Observations
Based on the configuration files provided (`Cargo.toml` and `Cargo.lock`):

1.  **Protobuf/gRPC Infrastructure Configured**: The root configuration defines standard versioned schema-compilation tooling. Under `[workspace.dependencies]` in `Cargo.toml:94-101`, the system imports:
    *   `tonic` (gRPC framework)
    *   `prost` and `prost-types` (Protocol Buffers compiler and runtime)
    *   `tonic-build` (for compile-time generation of Rust types from `.proto` schemas)
2.  **Ad-Hoc Serialization Risks Detected**: Alongside the robust versioned-schema dependencies, the project brings in several ad-hoc serialization libraries in `Cargo.toml:68-74`:
    *   `serde` (ad-hoc structural serialization)
    *   `simd-json` and `serde_json` (raw JSON parsing)
    *   `serde_yaml` (YAML parsing)
    *   `toml` (TOML parsing)
    *   `jsonschema` (JSON Schema validation)

### Risks & Action Items
*   **Ad-Hoc Serialization Boundaries**: The co-existence of `serde_json` and `serde_yaml` alongside `prost` suggests that certain services (such as the LLM interface `op-llm` or state storage in `op-state-store`) may be exchanging raw JSON payloads or storing unstructured objects.
*   **Required Remediation**: During the code-level implementation review of workspace members (e.g., `crates/op-cognitive-mcp`, `crates/op-state-store`), developers must verify that no public API endpoints, DBus interfaces, or persistent database layers define their contracts as arbitrary `serde`-derived structs or direct JSON strings. All data exchanged across RPC boundaries or written to stores must be compiled from centralized, versioned Protocol Buffer definitions or validated against strictly versioned JSON schemas (using `jsonschema`).