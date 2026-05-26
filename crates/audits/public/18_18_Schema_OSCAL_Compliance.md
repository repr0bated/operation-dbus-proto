### Schema-as-Code Audit

The following table identifies components where data contracts are expressed as ad-hoc Rust structs, strings, or untyped formats (e.g., raw JSON-RPC or XML) rather than versioned Protocol Buffer schemas or machine-readable schemas:

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| **JSON-RPC State Transport** | Data / RPC Contract | `Cargo.toml:12` | No | `op-jsonrpc` relies on ad-hoc JSON-RPC structures serialized via `serde_json`/`simd-json` instead of versioned Protobuf messages, violating schema-as-code consistency. |
| **D-Bus Introspection & Mirroring** | Interface Contract | `Cargo.toml:35` | No | `op-dbus-mirror` and `op-dbus-model` interact with raw XML via `zbus_xml` (`Cargo.toml:63`) and `quick-xml` (`Cargo.toml:119`), parsing unstructured D-Bus wire data into ad-hoc Rust types. |
| **Compliance Rules Engine** | Configuration Schema | `Cargo.toml:37` | No | `op-compliance` uses `jsonschema` (`Cargo.toml:60`) to validate raw JSON representations of compliance parameters rather than using strongly-typed, versioned OSCAL schemas or Protobuf contracts. |
| **Network Control Plane** | Control Protocol | `Cargo.toml:18` | No | `op-network` interfaces with `rovs-jsonrpc` and `rovs-ovsdb` using untyped JSON and ad-hoc maps, presenting a schema-drift hazard. |
| **Distributed Cache & State Store** | Database Schema | `Cargo.toml:13` | Partial | `op-state-store` reads and writes raw JSON states to Redis using `jsonschema` for runtime validation, lacking structured schema definitions. |

---

### OSCAL Coverage Audit

The system implements core security controls (such as identity management, cryptographic isolation, and compliance validation) within its crate boundaries, but lacks any tracing or mapping to machine-readable OSCAL (Open Security Controls Assessment Language) artifacts:

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Identification and Authentication (NIST SP 800-53 IA)** | `Cargo.toml:29`, `Cargo.lock` (under `op-identity`) | None | System handles system identities and OS-level keyrings (`keyring`, `x25519-dalek`) without mapping cryptographic key lifecycle and peer identities to an OSCAL `component-definition`. |
| **Cryptographic Protection (NIST SP 800-53 SC-13)** | `Cargo.toml:5`, `Cargo.lock` (under `op-gateway`, `op-state`) | None | Secure state and payload encryption use hardcoded cryptographic suites (`aes-gcm`, `argon2`, `chacha20poly1305`) without matching system security configuration parameters in an OSCAL System Security Plan (SSP). |
| **System and Information Integrity / Compliance (NIST SP 800-53 SI)** | `Cargo.toml:37`, `Cargo.lock` (under `op-compliance`) | None | Validates system integrity and OS properties via raw JSON schemas rather than ingesting or executing assessments against OSCAL Assessment Plans (AP) or Assessment Results (AR). |
| **Access Control (NIST SP 800-53 AC)** | `Cargo.toml:28`, `Cargo.lock` (under `op-grpc-bridge`, `op-mcp-proxy`) | None | Endpoint boundaries and gRPC proxy mappings are defined procedurally in Rust, with no corresponding machine-readable OSCAL definitions cataloging the allowed boundaries. |

---

### Recommendations

#### 1. Resolve Dependency Splitting and Version Skew on `zbus`
* **Risk (Major):** The workspace has a split dependency tree for its primary D-Bus IPC framework. `Cargo.toml:50` defines `zbus = "5.12"`, but `Cargo.lock` reveals that multiple active crates (including `op-agents`, `op-chat`, `op-cognitive-mcp`, `op-core`, `op-dbus-mirror`, `op-grpc-bridge`, `op-introspection`, `op-plugins`, `op-projection`, `op-services`, `op-state`, `op-state-store`, `op-tools`, and `op-web`) are compiled against `zbus 4.4.0`, while `op-identity` is compiled against `zbus 5.13.2`. This split can cause unexpected linker issues, bloat, type incompatibility, and concurrent runtime failures on the D-Bus system loop.
* **Resolution:** Upgrade all workspace members to use the unified workspace dependency `zbus.workspace = true` to force identical compilation features and version bounds across all crates.

#### 2. Deprecate Weak Cryptography (`md5`) in Identity and State Crates
* **Risk (Major):** `Cargo.toml:135` and `Cargo.lock` declare a direct dependency on `md5` (`v0.7`), which is utilized by `op-identity`, `op-state`, `op-state-store`, and `op-plugins`. The MD5 algorithm is cryptographically broken and highly vulnerable to collision attacks. If used for identity verification or state integrity hashing, it represents a high-severity security risk.
* **Resolution:** Replace all usages of `md5` with SHA-256 (`sha2` workspace dependency, `Cargo.toml:68`) or BLAKE3. Enforce a compiler-level policy banning weak hashing algorithms in security-sensitive modules.

#### 3. Establish a Unified Protobuf-Based Schema Registry
* **Risk (Major):** The codebase utilizes heterogeneous serialization formats, parsing D-Bus structures with raw XML parsers (`Cargo.toml:119`), validating state with runtime `jsonschema` engines (`Cargo.toml:60`), and routing messages over JSON-RPC. This creates schema-drift vulnerabilities and performance penalties from dynamic parsing.
* **Resolution:** Consolidate all external and internal data contracts into versioned Protobuf (`.proto`) schemas. Generate Rust bindings using `prost-build` during compile time, ensuring backward-compatible schema evolution through enforced protobuf field numbering rules.

#### 4. Transition Compliance Validations to Native OSCAL Schemas
* **Risk (Medium):** The compliance module (`op-compliance`) relies on custom, ad-hoc JSON schemas to assert system state compliance. This isolates the control plane from federal and industrial risk management frameworks (e.g., FedRAMP, NIST 800-53).
* **Resolution:** Integrate native OSCAL models. Refactor `op-compliance` to parse, ingest, and output standard OSCAL Component Definitions and Assessment Results (AR) JSON, allowing the system to consume standard compliance files directly.