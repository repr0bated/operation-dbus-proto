# Production Security and Quality Audit

## 1. Public API Surface & Dead Code

### Public API Surface Analysis
Because only the build manifests (`Cargo.toml` and `Cargo.lock`) are provided in the FILES section, there are no Rust source files (`.rs`) available to inspect for code-level item declarations (`pub fn`, `pub struct`, `pub enum`, `pub trait`, etc.) or field visibility. 

However, we can define the **Crate-Level Workspace API Surface** based on the exposed workspace members and their dependency configurations in `Cargo.toml`. Below is the enumeration of the 10 most impactful public workspace crates that constitute the boundaries of the control plane:

| Crate / Workspace Member | Crate Role & Impact | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| **`op-core`** | Defining shared core abstractions and async traits | `Cargo.toml:5` | Standardize core types and prevent raw string-based data passing. |
| **`op-state`** | Core state machine transition and security enforcement | `Cargo.toml:10` | Ensure state changes are cryptographically authenticated. |
| **`op-state-store`** | Persistent state persistence layers (Redis, SQL, sled) | `Cargo.toml:11` | Abstract database engines behind strict schema validation. |
| **`op-dbus-model`** | DBus domain interface models and serialization | `Cargo.toml:31` | Enforce schema validation for incoming DBus messages. |
| **`op-execution-tracker`** | Observability, metric collection, and task auditing | `Cargo.toml:27` | Ensure tamper-proof logging of task execution metadata. |
| **`op-grpc-bridge`** | DBus-to-gRPC event streaming bridge | `Cargo.toml:32` | Strictly enforce mTLS and token-based auth on gRPC interfaces. |
| **`op-compliance`** | Security audit compliance policy evaluation | `Cargo.toml:37` | Integrate schema-defined compliance models (OSCAL). |
| **`op-cognitive-mcp`** | Model Context Protocol implementation for LLM reasoning | `Cargo.toml:29` | Restrict dynamic action execution based on sandboxed environments. |
| **`op-network`** | Low-level Linux network routing (rtnetlink, OpenFlow) | `Cargo.toml:15` | Enforce least-privilege capability execution for netlink commands. |
| **`op-projection`** | Event sourcing projections and read-optimized views | `Cargo.toml:38` | Ensure projection integrity during historical replays. |

* **Glob Re-exports (`pub use *`)**: None identified (no source files provided).
* **Public Fields on Structs that Should Be Private**: None identified (no source files provided).

---

### Dead Code Audit
As no Rust source code is present in this audit context, code-level analysis of `#[allow(dead_code)]` or unused imports cannot be performed. 

* **`#[allow(dead_code)]` counts**: 0 (no Rust source files provided).
* **Unused/Transitive Workspace Dependencies**: The dependencies listed below are declared in the workspace configuration but are not consumed by the root package (`op-dbus`). They are reserved for transitive sub-crate use:

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `cozo` | Workspace Dependency | `Cargo.toml:55` | Expose only via localized graph queries in `op-cognitive-mcp`. |
| `qdrant-client` | Workspace Dependency | `Cargo.toml:70` | Enforce schema compilation for vector metadata. |
| `rusqlite` | Workspace Dependency | `Cargo.toml:111` | Remove in favor of unified `sqlx-sqlite` to prevent linker bloat. |
| `redis` | Workspace Dependency | `Cargo.toml:112` | Consolidate usage in `op-state-store`. |
| `lru` | Workspace Dependency | `Cargo.toml:113` | Constrain cache sizing parameters. |
| `aes-gcm` | Workspace Dependency | `Cargo.toml:129` | Audit encryption key derivation parameters. |
| `argon2` | Workspace Dependency | `Cargo.toml:130` | Ensure work factors (m_cost, t_cost) match current OWASP standards. |

---

## 2. Schema-As-Code Compliance Audit

The codebase asserts a "schema-as-code discipline using Protocol Buffers and OSCAL." However, several high-value discrepancies exist in the workspace configuration:

### [1] Ad-Hoc Runtime Validation vs. Compile-Time Versioned Schemas
* **Finding**: The root crate relies heavily on `jsonschema` and `serde_json` for dynamic message handling.
* **Citations**: `Cargo.toml:169` (`serde_json.workspace = true`) and `Cargo.toml:170` (`jsonschema.workspace = true`).
* **Vulnerability / Quality Risk**: Dynamic JSON validation is highly susceptible to parser differential attacks and schema bypasses. If data contracts are validated at runtime against raw JSON schemas loaded from the filesystem or parsed from strings, arbitrary structural variations or malicious payloads can exploit discrepancies between `serde_json` and the `jsonschema` validator engine.
* **Remediation**: Transition the internal data contracts from ad-hoc JSON structs to versioned Protocol Buffer definitions. Use `prost-build` to compile schemas into strictly-typed Rust structs at build-time, ensuring structural invariants are guaranteed by the type system before any business logic executes.

### [2] Total Absence of OSCAL Native Schema Structures
* **Finding**: There are no OSCAL parser or validator libraries declared in the workspace dependencies.
* **Citations**: `Cargo.toml:40` to `Cargo.toml:142` (Workspace Dependencies).
* **Vulnerability / Quality Risk**: In the absence of an OSCAL schema compilation step (such as parsing OSCAL JSON/XML profiles into native types), the compliance engine (`op-compliance` at `Cargo.toml:37`) must be processing compliance assessments as ad-hoc nested maps, JSON strings, or unchecked custom types. This severely weakens compliance assurances, as it is impossible to programmatically guarantee alignment with NIST OSCAL schemas without robust parser integration.
* **Remediation**: Integrate a code-generation utility or compiled schema crate that maps NIST OSCAL JSON Schemas into validated Rust structures at compile-time.

---

## 3. Production Security & Quality Findings

### CRITICAL: None (Requires source-code validation of exploit paths).

---

### HIGH: Cryptographically Broken Primitive (MD5) Configured in Control Plane
* **ID**: SEC-HIGH-01
* **File:Line**: `Cargo.toml:120` (`md5 = "0.7"`)
* **Impact**: MD5 is highly vulnerable to collision attacks and must never be utilized for cryptographic verification, secure hashing, state integrity, or token generation. If any workspace crate (such as `op-state` at `Cargo.toml:12` or `op-identity` at `Cargo.toml:34`) utilizes this dependency to hash credentials, identify network hosts, or generate checksums for system states, attackers can forge identical hashes to bypass authentication and manipulate state machine execution.
* **Remediation**: Deprecate the `md5` dependency entirely. Replace all occurrences with secure hashing primitives, such as SHA-256 (`sha2` workspace dependency at `Cargo.toml:79`) or BLAKE2b (`blake2` transitive dependency found in lockfile).

---

### MEDIUM: Coexistence of Conflicting SQLite Engines (`sqlx` and `rusqlite`)
* **ID**: QUAL-MED-01
* **File:Line**: `Cargo.toml:110` (`sqlx = ... features = ["sqlite"]`) and `Cargo.toml:111` (`rusqlite = { features = ["bundled"] }`)
* **Impact**: Linking two independent SQLite wrapping libraries in the same compiled binary can cause severe build-time or runtime issues. When `rusqlite` is compiled with the `"bundled"` feature, it statically compiles and links its own copy of `libsqlite3`. Simultaneously, `sqlx` may link against a different static or dynamically linked SQLite instance. This duplication can result in:
  1. Duplicate symbol linker conflicts.
  2. Memory corruption or unexpected undefined behavior if both engines attempt to load extension symbols or write to the same database file pointer.
  3. Increased binary overhead and slow compilation times.
* **Remediation**: Eliminate the `rusqlite` dependency. Consolidate all SQL operations under the `sqlx` framework. If an embedded, non-async SQL engine is required, explicitly separate these components into non-overlapping processes or configure `rusqlite` and `sqlx` to link against the exact same system-provided library version (avoiding the `"bundled"` feature).

---

### LOW: Unpinned / Loose SemVer Ranges for Core Ecosystem Dependencies
* **ID**: QUAL-LOW-01
* **File:Line**: `Cargo.toml:72` (`anyhow = "1"`), `Cargo.toml:73` (`thiserror = "1"`), `Cargo.toml:75` (`serde_json = "1"`)
* **Impact**: Specifying broad version ranges (such as `"1"`) for core system crates allows the compiler to resolve down to any minor release. While Rust maintains strong backward-compatibility guarantees, minor releases in the ecosystem can occasionally introduce compiler errors, change panic behavior under specific conditions, or introduce subtle alterations to serialization formatting. For a deterministic, safety-critical control plane, this introduces unnecessary variability into build output.
* **Remediation**: Lock critical ecosystem dependencies to specific minor versions (e.g., `anyhow = "1.0"`, `serde_json = "1.0"`) to stabilize compiled output while still allowing automatic patch-level security updates. Ensure strict enforcement of lockfile integrity checks in CI pipelines.