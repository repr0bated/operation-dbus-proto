### 1. Dependencies & Feature Inventory

The following table lists the direct dependencies declared in `crates/op-dbus-model/Cargo.toml` alongside their versions, explicitly enabled features, and their workspace default statuses.

| Crate | Declared Version | Explicitly Enabled Features | Default Features | Category / Risks |
| :--- | :--- | :--- | :--- | :--- |
| `serde` | `1.0` | `["derive"]` | Enabled | Standard serialization |
| `simd-json` | `workspace = true` | Inherited from workspace: `["serde", "serde_impl"]` | Enabled | High-performance JSON parser |
| `serde_json` | `workspace = true` | Inherited from workspace: None | Enabled | Standard JSON parser |
| `sqlx` | `0.8` | `["runtime-tokio", "sqlite", "json"]` | Enabled | Database driver; pulls in `tokio` under the hood |
| `chrono` | `0.4` | `["serde"]` | Enabled | Time representation |
| `uuid` | `1.6` | `["v4", "serde"]` | Enabled | ID generation |
| `thiserror` | `1.0` | None | Enabled | Error derivation |
| `anyhow` | `1.0` | None | Enabled | Ad-hoc error handling |
| `op-core` | `path = "../op-core"` | N/A | N/A | Internal workspace crate |
| `op-state-store`| `path = "../op-state-store"` | N/A | N/A | Internal workspace crate |

#### Features Defined by Crate
* **No local features are defined** within `crates/op-dbus-model/Cargo.toml`. 

#### Key Monitored Libraries
* **`anyhow`**: Version `1.0` is used for high-level error context propagation.
* **`thiserror`**: Version `1.0` is leveraged for structured, strongly-typed errors.
* **`tokio`**: Not declared directly, but pulled in transitively with the `full` feature-set through `sqlx`'s `runtime-tokio` feature.

#### Schema-as-Code Dependencies
* **Protocol Buffers / gRPC**: Workspace declares `prost` (`0.13`), `prost-types` (`0.13`), `tonic` (`0.12`), `tonic-build` (`0.12`), and `tonic-reflection` (`0.12`).
* **Validation**: `jsonschema` (`0.29` / `0.18`) is pulled in at the workspace level, but **not utilized** within `op-dbus-model` for enforcing database record validity or document contracts.
* **OSCAL Compliance**: No OSCAL component model crates (e.g., `oscal-rs`) are declared, signaling a strict gap in machine-readable compliance automation. Structured compliance documents must currently be maintained as ad-hoc configurations.

---

### 2. Storage Backend Inventory

| Backend | Found at File:Line | Role | Architecture Alignment Check |
| :--- | :--- | :--- | :--- |
| **SQLite (via `sqlx`)** | `crates/op-dbus-model/src/lib.rs:5` | Local Document & Catalog Persistence | **Aligned.** The crate explicitly states that SQLite serves as a local persistence cache rather than the source of truth (`crates/op-dbus-model/src/lib.rs:34-39`). |
| **CozoDB (with Sled)**| Workspace `Cargo.toml` | Graph / Relational-Vector Store | **N/A in Crate.** While CozoDB is declared in the workspace for graph relationships, it is not integrated into `op-dbus-model` directly. |

#### Database Alignment Findings:
The SQLite database stores serialized dynamic JSON documents in standard relational columns. While a `schemas` table is defined during database initialization in `crates/op-dbus-model/src/lib.rs:18-31` with a foreign-key relationship pointing back to `plugins`, the database logic never actually uses this table. 

---

### 3. Schema-as-Code Audit & Compliance Analysis

The workspace aims to implement a schema-as-code discipline using Protocol Buffers and machine-readable data contracts. However, the `op-dbus-model` crate displays several architectural drift gaps where dynamic data contracts are handled using ad-hoc structures and loose JSON typing:

1. **Ad-hoc Serialization Maps**:
   * In `crates/op-dbus-model/src/models.rs:5-10`, the `Plugin` struct defines the contract field `base_object` as `simd_json::OwnedValue`. This is an unstructured JSON AST, rather than a versioned schema.
   * Similarly, in `crates/op-dbus-model/src/models.rs:12-20`, `Schema::definition` uses `simd_json::OwnedValue`. Storing schema definitions as raw dynamic JSON values directly compromises deterministic compilation and static verification.
2. **String-ly Typed Connection Handlers**:
   * In `crates/op-dbus-model/src/models.rs:43` and `crates/op-dbus-model/src/models.rs:47`, file system and IPC routing boundaries (`dbus_path` and `storage_path`) are represented as unstructured primitive `String`s. These lack type-enforced validation, bypassing serialization schemas.
3. **Ignored Schema Catalogs**:
   * The relational tables for `schemas` are constructed but entirely unused, shifting cataloging away from structured relational models toward monolithic, JSON-encoded binary objects (`base_object` columns in SQLite).

---

### 4. Security & Quality Findings

#### Finding 1: Structured Log Bypass / Direct Stderr Writes (Low)
* **File Citation**: `crates/op-dbus-model/src/lib.rs:91`
* **Line Code**:
  ```rust
  eprintln!("Skipping stale plugin catalog document '{}': {}", name, error);
  ```
* **Description**:
  The `list_documents` function prints directly to the standard error stream (`eprintln!`) when failing to deserialize a database record. In a daemonized control-plane system, writing directly to `stderr` bypasses the structured tracing subscriber configured in `tracing-subscriber`. This prevents central log aggregators from capturing, alerting on, or indexing catalog corruption events.
* **Remediation**:
  Replace `eprintln!` with structured logging macros:
  ```rust
  tracing::error!(
      target: "catalog",
      plugin_name = %name,
      error = %error,
      "Skipping stale plugin catalog document due to deserialization failure"
  );
  ```

---

#### Finding 2: Unvalidated Path and IPC Address Primitives (Low)
* **File Citation**: `crates/op-dbus-model/src/models.rs:43-47`
* **Line Code**:
  ```rust
  pub dbus_path: String,
  pub service_name: String,
  pub storage_path: String,
  ```
* **Description**:
  The system persists absolute paths to disk (`storage_path`) and D-Bus interfaces (`dbus_path`) as primitive `String`s without sanitization. If an attacker gains write access to the underlying SQLite database file, or if a dynamic plugin registers with directory traversal indicators (e.g., `../../etc/shadow`), subsequent reading layers in other modules that consume these paths may run with administrative control plane privileges, potentially leading to unauthorized path traversal or IPC boundary crossing.
* **Remediation**:
  Introduce strong type boundaries during serialization and deserialization. Leverage specific validated path types, such as canonicalized `std::path::PathBuf` checks, or parse the `dbus_path` into a validated D-Bus object path descriptor (e.g., `zbus::zvariant::ObjectPath`).

---

#### Finding 3: Orphaned Schema Persistence Layer (Low)
* **File Citation**: `crates/op-dbus-model/src/lib.rs:18-31`
* **Line Code**:
  ```rust
  sqlx::query(
      r#"
      CREATE TABLE IF NOT EXISTS schemas ( ... )
      "#
  )
  ```
* **Description**:
  The `create_schema` query provisions a dedicated relational table to house schemas, including foreign key tracking, discovery origins (`discovered_from`), and timestamps. However, `SqlitePluginCatalog` provides no implementation, methods, or queries to insert, update, or retrieve data from the `schemas` table. The `Schema` struct defined in `crates/op-dbus-model/src/models.rs:12-20` is similarly dead code. This represents structural schema drift where schema persistence is bypassed in favor of raw JSON-encoded structures.
* **Remediation**:
  Either fully implement the persistence methods for `schemas` and the `Schema` model inside `SqlitePluginCatalog` to normalize schema storage, or remove the unused table migration and structural model to reduce code maintenance overhead.

---

#### Finding 4: Inconsistent JSON Parser Usage (Low)
* **File Citation**: `crates/op-dbus-model/src/lib.rs:50`, `crates/op-dbus-model/src/lib.rs:69`, and `crates/op-dbus-model/src/models.rs:8`
* **Line Code**:
  ```rust
  // lib.rs:50
  let encoded = serde_json::to_string(document)?;
  // lib.rs:69
  let document = serde_json::from_str(&encoded)?;
  // models.rs:8
  pub base_object: simd_json::OwnedValue,
  ```
* **Description**:
  The crate mixes standard `serde_json` and `simd-json` within overlapping models. `simd-json::OwnedValue` is embedded in the models, but `serde_json` is used for serializing and deserializing `PluginCatalogDocument` when storing catalog entries in the database. Mixing parsing engines can introduce subtle parsing discrepancies, limit nested depth representation unexpectedly, or alter numerical precision handling depending on the backend, creating consistency issues.
* **Remediation**:
  Standardize on a single JSON processing engine across all persistence models. If `simd-json` is selected for its high performance, replace standard `serde_json::to_string` and `from_str` calls with their unified `simd-json` equivalents.