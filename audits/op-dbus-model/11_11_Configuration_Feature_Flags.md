# Configuration and Security Audit: Crate `op-dbus-model`

## 1. Environment Variable Evaluation (`std::env::var`)
A search of the provided source files for the `op-dbus-model` crate reveals that **there are no `std::env::var` reads** present in the code.

* **List of `std::env::var` reads:** None.
* **Flagged unresolved env vars (no defaults/error handling):** None.

---

## 2. Cargo Features Analysis
The crate `op-dbus-model` is configured in `crates/op-dbus-model/Cargo.toml`.

### Crate-Level Features (`crates/op-dbus-model/Cargo.toml`)
* **Features defined:** None.
* **Optional dependencies acting as features:** None.

### Workspace/Sibling Crate Configuration (`Cargo.toml`)
The root workspace package (`op-dbus`) defines the following features:
* `default = ["grpc"]`
* `grpc = []`

### Additive Nature of Features
In accordance with Cargo design principles, all workspace and package features are **additive**. Enabling `grpc` does not destructively remove or mutate existing types, but rather extends the available interface or changes compilation targets downstream. Since `op-dbus-model` exposes no features of its own, its dependency tree remains fully stable across workspace compilation.

---

## 3. Hardcoded Paths, Ports, and Addresses
There are no hardcoded runtime absolute paths, network ports, or IP/socket addresses within the source code.

* **Build-time Paths:** Standard relative path dependencies are defined in `crates/op-dbus-model/Cargo.toml` and the root `Cargo.toml` (e.g., `op-core = { path = "../op-core" }`). These do not affect production runtime execution and are not security flags.
* **SQL Queries:** All SQLite table names and configurations are managed dynamically through the `SqlitePool` handle passed to the constructor or initializer (see `crates/op-dbus-model/src/lib.rs:7` and `crates/op-dbus-model/src/lib.rs:49`). No hardcoded SQLite database file paths exist in this crate.

---

## 4. Schema-as-Code Compliance and Quality Findings

This codebase has a schema-as-code discipline using Protocol Buffers and OSCAL. Ad-hoc structs or SQL string schemas are flagged below.

### [Medium] Ad-Hoc SQL Database Schema Definition
* **Location:** `crates/op-dbus-model/src/lib.rs:8-17` and `crates/op-dbus-model/src/lib.rs:19-32`
* **Finding:** The database schemas for `plugins` and `schemas` tables are declared dynamically as ad-hoc raw SQL strings inside the `create_schema` function rather than being bound to a declarative, version-controlled schema format or migration tool (such as sqlx migrations or protobuf specifications).
* **Impact:** Schema evolution of local databases can easily drift from the architectural specifications of the application.

### [Low] Ad-Hoc Rust Serialization Contracts
* **Location:** `crates/op-dbus-model/src/models.rs:5-10`, `crates/op-dbus-model/src/models.rs:13-21`, and `crates/op-dbus-model/src/models.rs:35-51`
* **Finding:** The data contracts for `Plugin`, `Schema`, and `PluginCatalogDocument` are expressed as ad-hoc Rust structs utilizing serde serialization attributes instead of strict, versioned schemas (such as Protocol Buffers or JSON Schema specifications).
* **Impact:** Interoperability with non-Rust systems (or mismatched versions of the control plane) can lead to parsing failures, particularly when using flexible, unversioned containers like `simd_json::OwnedValue` (e.g., in `Plugin::base_object` and `Schema::definition`). To comply with schema-as-code principles, these structures should ideally be derived from versioned protobuf schema definitions.