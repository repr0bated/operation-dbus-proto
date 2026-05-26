# Production Security and Quality Audit: OP-DBUS Control Plane

## 1. Dependencies & Feature Inventory

### Direct Dependency Analysis (Workspace & Package)
The following table documents the direct dependencies declared in the workspace configuration (`Cargo.toml`) and the main package (`Cargo.toml`), listing their versions, explicit features, and default feature status:

| Dependency | Version | Explicit Features | Default Features | Pull Method / Notes |
| :--- | :--- | :--- | :--- | :--- |
| `tokio` | `1` | `["full"]` | Enabled | Workspace. Pulls in all tokio async runtime features. |
| `tokio-stream` | `0.1` | None | Enabled | Workspace. |
| `futures` | `0.3` | None | Enabled | Workspace. |
| `async-trait` | `0.1` | None | Enabled | Workspace. |
| `serde` | `1` | `["derive"]` | Enabled | Workspace. |
| `simd-json` | `0.13` | `["serde", "serde_impl"]` | Enabled | Workspace. High-performance JSON parser. |
| `serde_json` | `1` | None | Enabled | Workspace. |
| `serde_yaml` | `0.9` | None | Enabled | Workspace. Deprecated. |
| `toml` | `0.8` | None | Enabled | Workspace. |
| `jsonschema` | `0.29` | None | Disabled (`default-features = false`) | Workspace. Handled with explicit schema constraints. |
| `zbus` | `5.12` | `["tokio"]` | Enabled | Workspace. Desktop bus connection framework. |
| `zbus_xml` | `4.0` | None | Enabled | Workspace. |
| `axum` | `0.7` | `["ws", "macros", "tokio"]` | Enabled | Workspace. |
| `tower` | `0.4` | None | Enabled | Workspace. |
| `tower-http` | `0.5` | `["cors", "fs", "trace", "compression-gzip"]` | Enabled | Workspace. |
| `reqwest` | `0.11` | `["json", "stream"]` | Enabled | Workspace. HTTP Client. |
| `qdrant-client` | `1.7` | None | Enabled | Workspace. Vector search client. |
| `cozo` | `0.7.6` | `["rayon", "storage-sled"]` | Disabled (`default-features = false`) | Workspace. Graph relational store backend. |
| `anyhow` | `1` | None | Enabled | Workspace. |
| `thiserror` | `1` | None | Enabled | Workspace. |
| `tracing` | `0.1` | None | Enabled | Workspace. |
| `tracing-subscriber`| `0.3` | `["env-filter", "json"]` | Enabled | Workspace. |
| `uuid` | `1.6` | `["v4", "serde"]` | Enabled | Workspace. |
| `chrono` | `0.4` | `["serde"]` | Enabled | Workspace. |
| `quick-xml` | `0.36` | `["serialize"]` | Enabled | Workspace. |
| `regex` | `1` | None | Enabled | Workspace. |
| `sha2` | `0.10` | None | Enabled | Workspace. |
| `base64` | `0.21` | None | Enabled | Workspace. |
| `libc` | `0.2` | None | Enabled | Workspace. |
| `bytes` | `1.0` | None | Enabled | Workspace. |
| `hex` | `0.4` | None | Enabled | Workspace. |
| `memmap2` | `0.9` | None | Enabled | Workspace. |
| `parking_lot` | `0.12` | None | Enabled | Workspace. |
| `dashmap` | `5.0` | None | Enabled | Workspace. |
| `pin-project-lite` | `0.2` | None | Enabled | Workspace. |
| `glob` | `0.3` | None | Enabled | Workspace. |
| `mime_guess` | `2.0` | None | Enabled | Workspace. |
| `tonic` | `0.12` | `["tls", "tls-roots", "tls-webpki-roots"]` | Enabled | Workspace. gRPC core library. |
| `prost` | `0.13` | None | Enabled | Workspace. Protobuf core library. |
| `prost-types` | `0.13` | None | Enabled | Workspace. |
| `tonic-build` | `0.12` | None | Enabled | Workspace. |
| `tonic-reflection` | `0.12` | None | Enabled | Workspace. |
| `tonic-health` | `0.12` | None | Enabled | Workspace. |
| `tonic-web` | `0.12` | None | Enabled | Workspace. |
| `sqlx` | `0.8` | `["sqlite", "runtime-tokio", "json"]` | Enabled | Workspace. Relational database driver. |
| `rusqlite` | `0.32` | `["bundled"]` | Enabled | Workspace. Bundled SQLite binding. |
| `redis` | `0.25` | `["tokio-comp"]` | Enabled | Workspace. |
| `lru` | `0.12` | None | Enabled | Workspace. |
| `clap` | `4` | `["derive"]` | Enabled | Workspace. Command line parser. |
| `lazy_static` | `1.4` | None | Enabled | Workspace. |
| `hyper` | `1.0` | `["full"]` | Enabled | Workspace. HTTP core engine. |
| `hyper-util` | `0.1` | `["full"]` | Enabled | Workspace. |
| `rtnetlink` | `0.14` | None | Enabled | Workspace. netlink networking interface. |
| `gethostname` | `0.5` | None | Enabled | Workspace. |
| `num_cpus` | `1.16` | None | Enabled | Workspace. |
| `tempfile` | `3` | None | Enabled | Workspace. |
| `tar` | `0.4` | None | Enabled | Workspace. |
| `flate2` | `1` | None | Enabled | Workspace. |
| `bincode` | `1.3` | None | Enabled | Workspace. |
| `log` | `0.4` | None | Enabled | Workspace. |
| `aes-gcm` | `0.10` | None | Enabled | Workspace. |
| `argon2` | `0.5` | None | Enabled | Workspace. |
| `rand` | `0.8` | None | Enabled | Workspace. |
| `md5` | `0.7` | None | Enabled | Workspace. Cryptographically broken hashing. |
| `opentelemetry` | `0.22` | `["metrics", "trace"]` | Enabled | Workspace. |
| `prometheus` | `0.13` | `["process"]` | Enabled | Workspace. |
| `rustls` | `0.23` | None | Enabled | Workspace. |
| `rustls-pemfile` | `2` | None | Enabled | Workspace. |
| `tokio-rustls` | `0.26` | None | Enabled | Workspace. |
| `rust-embed` | `8.0` | None | Enabled | Direct package dependency. Unpinned major. |

### Crate Feature Flags
The main `op-dbus` crate defines the following feature gate structure:

```toml
[features]
default = ["grpc"]
grpc = []
```

#### Feature Gate Impact Analysis
*   **`grpc` Feature Gate**: The feature is declared on `Cargo.toml:110-111`. However, audit of `Cargo.toml:147-151` shows that gRPC dependencies (`tonic`, `tonic-reflection`, `tonic-web`) are declared as **mandatory dependencies** without the `optional = true` property. Consequently, compiling `op-dbus` without the `default` features (i.e. `--no-default-features`) will *not* prevent compiling and linking the entire gRPC stack. This results in useless feature configuration and unnecessary build bloat.

### Critical, Yanked, and Deprecated Supply-Chain Risks
*   **`serde_yaml` (Deprecated)**: Declared on `Cargo.toml:32`. The `serde_yaml` crate is officially deprecated and unmaintained. It is highly susceptible to unpatched deserialization issues or security gaps.
*   **`md5` (Cryptographically Broken)**: Declared on `Cargo.toml:105`. MD5 is a cryptographically broken hashing algorithm with practical collision attacks. Its use inside identity, state verification, or compliance modules introduces structural risks.
*   **`rust-embed` (Unpinned)**: Declared on `Cargo.toml:153` as `version = "8.0"`. Specifying `8.0` instead of a locked patch version or using workspace-level dependency inheritance bypasses supply chain normalization.

---

## 2. Storage Backend Mapping

The OP-DBUS platform utilizes multiple conflicting persistence layers across its modular crates. The mapping below identifies every persistence layer used across the workspace:

| Backend | Found at file:line | Role (KV/Graph/Cache/Queue) | Architectural & Linkage Context |
| :--- | :--- | :--- | :--- |
| **CozoDB (Sled)** | `Cargo.toml:44` | Graph / Vector Knowledge Base | Embedded Datalog engine. Configured with pure-Rust `storage-sled` to bypass link conflicts. Used by `op-cognitive-mcp` and `op-cozo-store`. |
| **SQLx (SQLite)** | `Cargo.toml:88` | Relational Store | Workspace dependency mapping SQLite metadata storage. Used directly by `op-dbus`, `op-dbus-model`, `op-gateway`, `op-services`, and `op-state-store`. |
| **rusqlite** | `Cargo.toml:89` | Local Caching / Buffered State | Raw SQLite library compiled with `["bundled"]`. Used by `op-cache`, `op-introspection`, and `op-mcp-proxy`. |
| **Redis** | `Cargo.toml:90` | Shared Cache & State Store | Distributed key-value cache used for remote state synchronizations in `op-state-store`. |
| **Qdrant Client** | `Cargo.toml:39` | Vector Database | Remote gRPC/HTTP interface to a Qdrant instance. Used in `op-cognitive-mcp` and `op-grpc-bridge`. |
| **LRU** | `Cargo.toml:91` | In-Memory Eviction KV Cache | Thread-local eviction container used in `op-dynamic-loader` and `op-mcp-aggregator`. |

### Linker Hazard & Architectural Violations
1.  **SQLite Direct Linker Collision**: The workspace attempts to bypass C-linker conflicts on SQLite by forcing Cozo to use Sled (`Cargo.toml:44`). However, the system imports *both* `sqlx` with the `"sqlite"` feature (`Cargo.toml:88`) and `rusqlite` with the `"bundled"` feature (`Cargo.toml:89`). When compiling the final unified `op-dbus` executable, which imports both dependencies (see `Cargo.toml:122` and transitive sub-crate dependencies), the linker will encounter symbol collisions between `sqlx`'s SQLite bindings and the bundled `rusqlite` C-source library. This results in unstable build behavior or binary compilation failure.
2.  **Sled Store Instability**: `cozo` uses Sled (`Cargo.toml:44`) for embedded storage. Sled is currently in a pre-1.0 unmaintained state, posing a risk of database corruption on unexpected power loss. This undermines the goal of creating a "deterministic control plane."

---

## 3. Schema-as-Code & Compliance Audit

### Schema-as-Code Evaluation
The project has implemented a mixed architecture where some boundaries leverage Schema-as-Code principles, while others fall back on ad-hoc structures:

*   **Protocol Buffer Integration**: System communication boundaries are formalized using `prost` (`Cargo.toml:81`) and `tonic` (`Cargo.toml:79`). Codegen configurations exist within `op-cache`, `op-chat`, `op-cognitive-mcp`, `op-grpc-bridge`, `op-mcp`, `op-mcp-proxy`, `op-projection`, and `op-services`.
*   **Validation Gap**: There is **no** dependency on schema validation suites such as `protovalidate` or `protoc-gen-validate`. Although raw serialization contracts are compiled, individual field bounds (e.g., regex constraints, integer ranges) are not declared at the schema level.
*   **JSON-Schema Layering**: The workspace imports `jsonschema` on `Cargo.toml:35`. However, there is no automatic schema generation dependency such as `schemars` or `openapiv3`. Payload definitions in crates like `op-compliance` and `op-state-store` are validated using hand-written, ad-hoc JSON-Schema files or raw strings.
*   **Ad-Hoc Structs Violation**: Database serialization models inside `op-dbus-model` (`Cargo.toml:1284`) and state-store bindings in `op-state-store` (`Cargo.toml:1414`) are defined purely as ad-hoc Rust structs. They lack derived schema representations (e.g., no Protobuf descriptions or JSON-Schema representations), violating strict schema-as-code principles.

### NIST OSCAL & Compliance Tooling Gap
There are no dependencies on formalized OSCAL compliance libraries like `oscal-rs` or `fedramp` within the workspace (`Cargo.toml`). 
While `op-compliance` (`Cargo.toml:1269`) is dedicated to compliance verification, it operates using ad-hoc `serde_json` structures to parse arbitrary configuration rules. There is no mapping to standardized NIST OSCAL control catalogs or system security plans (SSPs). This lack of versioned, structured compliance metadata creates a significant compliance-as-code gap.

---

## 4. Security & Architectural Findings

### [HIGH] Linker Symbol Collision via Coexisting Database Drivers
*   **Citations**: `Cargo.toml:88`, `Cargo.toml:89`, `Cargo.toml:122`
*   **Description**: The system relies on both `sqlx` configured with the `"sqlite"` backend and `rusqlite` with the `"bundled"` feature. 
*   **Impact**: When these sub-crates are linked together into the final `op-dbus` executable, the compiler attempts to link two distinct versions of SQLite symbols. One version comes from the bundled C-source code compiled by `rusqlite`, and the other comes from the system library or dynamic SQLx linking. This can cause linker failure, dynamic load issues, or silent undefined behavior.

### [HIGH] Cryptographically Broken MD5 Algorithm Used in Identity Crates
*   **Citations**: `Cargo.toml:105`, `Cargo.toml:128`, `Cargo.lock:1335`
*   **Description**: The outdated `md5` v0.7 crate is loaded as a core workspace dependency and imported directly by `op-identity` (`Cargo.lock:1335`), `op-plugins`, `op-state`, and `op-state-store`.
*   **Impact**: MD5 is vulnerable to collision attacks and is no longer suitable for security-sensitive calculations. Its presence in identity management and state tracking modules poses a severe security risk. This configuration can allow attackers to forge identities, bypass compliance checks, or trigger state collision issues.

### [MEDIUM] Dependency Version Splits for Critical System Crates
*   **Citations**: `Cargo.lock:1202`, `Cargo.lock:1445`, `Cargo.lock:1702`, `Cargo.lock:1716`
*   **Description**: The cargo lockfile reveals that multiple sub-crates are linked against duplicate, incompatible major versions of core libraries:
    *   `zbus` version split: `op-agents` depends on `zbus 4.4.0` (`Cargo.lock:1202`), while `op-identity` pulls in `zbus 5.13.2` (`Cargo.lock:1445`).
    *   `jsonschema` version split: `op-state-store` depends on `jsonschema 0.29.1` (`Cargo.lock:1702`), while `op-tools` relies on `jsonschema 0.18.3` (`Cargo.lock:1716`).
*   **Impact**: This split causes significant code bloat, extends compilation times, and increases memory usage. More importantly, it can cause type signature errors and runtime failures if types from different versions of these crates are passed across internal module boundaries.

### [MEDIUM] Critical Build-Chain Incompatibility in Protobuf Compilers
*   **Citations**: `Cargo.lock:1219`
*   **Description**: In `op-chat` (`Cargo.lock:1219`), the build chain uses `prost-build 0.12.6` and `tonic-build 0.11.0`, but the workspace dependencies mandate `prost 0.13.5` and `tonic 0.12.3` at runtime.
*   **Impact**: Mixing different code generators and runtime versions can lead to silent schema degradation, compilation failures, or runtime panics. This occurs when the generated structures use mismatched traits or missing functions from different versions of the runtime libraries.

### [LOW] Non-Functional gRPC Feature Gate
*   **Citations**: `Cargo.toml:110-111`, `Cargo.toml:147-151`
*   **Description**: The `Cargo.toml` file defines a `grpc` feature gate, but it does not declare `tonic`, `tonic-reflection`, and `tonic-web` as optional dependencies.
*   **Impact**: The `grpc` feature gate is ineffective. These large dependencies are always compiled and linked into the final binary, even when building with `--no-default-features`. This increases compile times and bloats the binary size for non-gRPC deployments.