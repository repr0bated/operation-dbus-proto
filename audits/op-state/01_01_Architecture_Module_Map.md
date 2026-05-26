## Architecture & Module Map

### Overview
`op-state` is a declarative, pluggable state management library designed to act as a native Linux system control plane. It coordinates state plugins, calculates desired configuration differences, validates configuration schemas, executes node-based state workflows, and interacts with system and session D-Bus interfaces.

### Module Tree
*   `lib.rs` (Crate root)
    *   `authority`: Handles network management exclusivity by stopping and disabling legacy network controllers (`NetworkManager`, `systemd-networkd`).
    *   `crypto`: Implements symmetric key-file storage (AES-256-GCM) and password-based key derivation (Argon2), along with raw encrypted state file input/output helper utilities.
    *   `dbus_plugin_base`: Provides helper functions for converting types between D-Bus (zvariant) and JSON structures, querying/mutating remote D-Bus properties, and recording cryptographic execution footprint hashes.
    *   `dbus_server`: Exposes the state manager orchestrator and individual plugins on the system/session bus interfaces.
    *   `manager`: The central orchestrator that registers active plugins, validates proposed state changes against a schema catalog, and applies diff operations.
    *   `plugin`: Defines metadata representations and the core `StatePlugin` trait.
    *   `plugin_workflow`: Provides PocketFlow workflow integration, wrapping plugins inside processing nodes to execute linear or branch-based pipelines.
    *   `plugtree`: Implements a hierarchical sub-resource management pattern (known as "pluglets"), facilitating per-resource lifecycle handling (such as individual LXC containers).
    *   `schema_validator`: Implements use-case verification templates and constraints to prevent erratic or invalid state generation.

### Entry Points
*   **Library Root**: `crates/op-state/src/lib.rs` / `crates/op-state/src/mod.rs` (exposes StateManager, PlugTree, and central store types).

### Notes
The parent cargo workspace contains multiple services, but `op-state` itself is compiled strictly as a library crate. Its configuration entry point for the D-Bus system/session daemon is managed by registering `StateManagerDBus` and `PluginDbusHost` on a system `zbus::Connection`.

---

## Production Security & Quality Audit

### Crucial Security Violations & Exploits

| Finding ID | Severity | File & Line Citation | Threat / Vulnerability Description | Remediation |
| :--- | :--- | :--- | :--- | :--- |
| **OP-SEC-01** | **Critical** | `crates/op-state/src/dbus_server.rs:109` <br> `crates/op-state/src/dbus_server.rs:129` | **Undefined Behavior / Local DoS via Unpadded raw string parsing in `simd_json`** <br><br>The unsafe `simd_json::from_str` API is executed on raw, unpadded `String` payloads directly obtained from D-Bus clients (`state_json` and `request_json`). `simd-json` relies on structural padding (at least `simd_json::PADDING` bytes) beyond the slice size for high-speed SIMD vector register reads. Passing unpadded strings allows local unprivileged users with D-Bus access to trigger out-of-bounds memory reads, causing instant segmentation faults or memory layout leakage inside the root-privileged state manager process. | Replace unsafe `simd_json::from_str` with its safe counterparts (such as `simd_json::to_owned_value` or standard `serde_json::from_str`), or ensure the buffer is explicitly padded using `simd_json::to_padded_string` before parsing. |
| **OP-SEC-02** | **Critical** | `crates/op-state/src/crypto.rs:188` <br> `crates/op-state/src/crypto.rs:194` <br> `crates/op-state/src/crypto.rs:199` <br> `crates/op-state/src/crypto.rs:208` | **Undefined Behavior / Segfault via Unpadded file parsing** <br><br>Similar to `OP-SEC-01`, raw string contents read from files via `std::fs::read_to_string` are parsed using `unsafe simd_json::from_str` inside the cryptographically critical `state_file` helper routines. If the state file on disk is modified, padded incorrectly, or truncated, starting the service will trigger undefined behavior or a segmentation fault. | Read the state file into a vector of bytes, convert to a padded string via `simd_json::to_padded_string`, or use a safe parsing engine like `serde_json` for file loading. |
| **OP-SEC-03** | **High** | `crates/op-state/src/crypto.rs:89-106` | **Insecure File Creation / Cryptographic Key Exposure (TOCTOU)** <br><br>In `from_key_file`, when a new 256-bit symmetric AES key is generated, it is written to the file system using `std::fs::write` before Unix permissions are modified. The file is created with default umask permissions (such as `0o644` or `0o664`), creating a race condition where local non-root processes can read the private key file before the process locks it down to `0o600`. | Open the file with platform-specific secure options to set permissions at the moment of creation. On Unix, use `std::fs::OpenOptions` combined with `std::os::unix::fs::OpenOptionsExt::mode(0o600)`. |
| **OP-SEC-04** | **High** | `crates/op-state/src/crypto.rs:52` <br> `crates/op-state/src/crypto.rs:141` | **Cryptographic Defect: Permanent Encrypted State Decryption Failure** <br><br>`StateEncryption::from_password` uses `Argon2` with a dynamically generated random salt (`SaltString::generate(&mut OsRng)`) to derive the 256-bit symmetric key. However, inside `encrypt`, the generated `EncryptedState` struct hardcodes the `salt` field to `None`. Because this salt is not persisted with the ciphertext, subsequent instantiations from the same password will use a different random salt, generating a mismatching key and permanently rendering the state undecryptable. | Update `encrypt` to receive, format, and persist the salt derived in the key setup phase inside `EncryptedState::salt`, and extract it back when calling `from_password` for decryption. |
| **OP-SEC-05** | **Medium** | `crates/op-state/src/dbus_plugin_base.rs:146` <br> `crates/op-state/src/plugin.rs:22` | **Non-Deterministic State Hashing / Signature Bypass Risk** <br><br>`hash_state` and `DesiredState::new` generate cryptographic hashes (SHA-256 and MD5) from JSON strings serialized using `simd_json::to_string`. Because standard JSON map/dictionary structures do not guarantee insertion or iteration order, keys are serialized non-deterministically. This results in varying hash values for structurally identical state maps, causing verification failures or silent footprint desynchronization across the D-Bus framework. | Enforce deterministic JSON serialization. Convert `simd_json::OwnedValue` to a sorted structure (like a BTreeMap) or use a canonical JSON serializer (such as `serde_json::to_string` with sorted keys) before executing cryptographic hashes. |

---

### Quality & Operational Defects

| Finding ID | Severity | File & Line Citation | Issue Description | Remediation |
| :--- | :--- | :--- | :--- | :--- |
| **OP-QLTY-01** | **Medium** | `crates/op-state/src/dbus_plugin_base.rs:83-84` | **Fragile D-Bus Property Translation via Debug String Representation** <br><br>The generic `get_property` function reads property values on D-Bus proxies and serializes them to strings using `format!("{:?}", value)` (which outputs Rust's internal `Debug` representation) before invoking `simd_json::from_str`. The debug format of `zbus::zvariant::OwnedValue` does not map cleanly to standard JSON for complex variants (such as array containers, structs, or dictionaries), leading to constant JSON deserialization failures and returning `Value::null()`. | Perform robust semantic transformation of the `zvariant::Value` into a structured json representation by matching on variant types (e.g. string, integers, maps, lists) recursively, similar to the `zvariant_to_json` helper. |
| **OP-QLTY-02** | **Medium** | `crates/op-state/src/authority.rs:14-30` | **Hardcoded Command Path / Execution on Non-systemd Platforms** <br><br>The authority manager directly executes `systemctl` using un-checked paths (`Command::new("systemctl")`) to stop or disable system services. On non-systemd Linux distributions (such as Alpine Linux or SysVinit systems), or systems where `PATH` does not contain the system control binaries, this will fail or can be subverted if an untrusted directory precedes `/bin` and `/usr/bin` in the path. | Validate system service runner systems first, use absolute paths for execution binaries (e.g., `/usr/bin/systemctl`), and verify success codes instead of discarding results. |
| **OP-QLTY-03** | **Low** | `crates/op-state/src/plugin.rs:22` | **Use of Obsolete MD5 Hash Algorithm** <br><br>`DesiredState::new` uses the insecure and collision-prone MD5 hashing algorithm to assign an identifier hash to newly received configurations. | Migrate the hashing mechanism from `md5` to a cryptographically secure hash function like `sha2::Sha256` or a fast non-cryptographic hash (such as `HighwayHash` or `xxHash`) if security is not desired. |

---

### Schema-as-Code Compliance Audit

The project aims to uphold a strict **schema-as-code** discipline where configurations, contracts, and system constraints are maintained via versioned, validated specifications. A review of the provided files reveals multiple deviations from this goal:

#### 1. Ad-Hoc Data Contracts and Hardcoded Validator Logic
*   **File Citation**: `crates/op-state/src/schema_validator.rs:11-57`
*   **Audit Finding**: Instead of declaring versioned system contracts using Protocol Buffers, OpenOSCAL, or external JSON Schema files, `schema_validator.rs` implements internal, ad-hoc structures such as `UseCaseTemplate`, `FieldCombination`, `Dependency`, and `Constraint` written in native Rust. 
*   **Hardcoded Defaults**:
    `SchemaValidator::load_default_use_cases()` hardcodes complex JSON state structural expectations, required plugins (such as `"privacy_router"`, `"openflow"`, `"net"`, and `"lxc"`), and default nested constraint definitions within the compiled binary code.
*   **Impact**: Modifying validation rules, updating schemas, or defining new configuration topologies requires modifying compiler source code rather than updating external, versioned schema files.

#### 2. D-Bus Host Schema Lookup Fallback
*   **File Citation**: `crates/op-state/src/dbus_server.rs:188`
*   **Audit Finding**: The D-Bus plugin host interface implements schema retrieval through a global, runtime-populated `SchemaRegistry`/`SchemaCatalog` containing generic JSON Schema fields (`json_schema`). The schemas are managed in-memory as arbitrary `simd_json::OwnedValue` blocks rather than highly structured, validated contract types.

#### Remediation Recommendation
To realign with a robust schema-as-code architecture:
1.  **Define Protocol Buffer / gRPC Schemas**: Move use-case structures, constraints, and state transitions to versioned `.proto` definition files. Compile these to native Rust code using `prost` to enforce rigorous type boundaries and compile-time API safety across external interfaces.
2.  **OSCAL Alignment**: Implement standard OSCAL Profile or Component schemas to express platform compliance controls (such as disabling network management utilities to maintain state authority) as structured yaml/json data rather than hardcoded string constraints inside `schema_validator.rs`.
3.  **Externalize Configurations**: Move runtime defaults and curated use-cases out of `schema_validator.rs` and into a designated schema repository loaded dynamically by the `SchemaCatalog` at service startup.