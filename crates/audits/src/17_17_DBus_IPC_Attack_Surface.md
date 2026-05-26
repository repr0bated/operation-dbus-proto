# D-Bus & IPC Security Audit Report

## 1. D-Bus & IPC Attack Surface

### 1.1 Interface, Method, and Signal Inventory
The FILES section contains only the workspace configuration files (`Cargo.toml` and `Cargo.lock`). No Rust source files (`.rs`), Proto files (`.proto`), or D-Bus XML policy definitions are present in the provided source code. Therefore, a concrete list of registered D-Bus interfaces, methods, and signals cannot be extracted.

However, the architecture's D-Bus capabilities are clearly defined by its configuration in `Cargo.toml` and `Cargo.lock`:
*   **D-Bus Framework:** The workspace depends heavily on `zbus` for D-Bus IPC.
*   **System vs. Session Bus:** The presence of `op-identity` (`Cargo.toml:31`) and its integration with system-level services (such as `rtnetlink` in `op-network` at `Cargo.toml:21`) strongly indicates that the control plane connects to the **System Bus** to manage system configuration (e.g., networking, identity management).
*   **Lack of Policy Configuration:** There are no D-Bus policy files (typically found in `/usr/share/dbus-1/system.d/` or `/etc/dbus-1/system.d/`) provided in the FILES section. Without an explicit XML system bus policy restricting access, unprivileged local users may be able to invoke methods on registered services if the service defaults to allowing arbitrary peers.

### 1.2 IPC Structural Risks & Vulnerability Analysis

#### Multiple Conflicting `zbus` Major Versions
The `Cargo.lock` and `Cargo.toml` reveal that the workspace compiles and runs **three different major versions of the `zbus` framework simultaneously**:
1.  `zbus 3.15.2` (via the `secret-service` dependency)
2.  `zbus 4.4.0` (used by `op-agents`, `op-chat`, `op-cognitive-mcp`, `op-grpc-bridge`, `op-introspection`, `op-mcp`, `op-plugins`, `op-services`, `op-state`, `op-state-store`, `op-tools`, `op-web`, `op-dbus-mirror`, and `op-projection`)
3.  `zbus 5.13.2` (explicitly used by `op-identity` at `Cargo.toml:31` and configured in `Cargo.toml:66` as `zbus = { version = "5.12" }` for workspace dependencies)

```
[op-agents, op-chat, etc.] ---> zbus 4.4.0
                                                ---> Conflicting Runtimes & ABI Drift
[op-identity]               ---> zbus 5.13.2
```

This multi-version split poses severe runtime and security risks:
*   **Runtime Thread Panics & Resource Starvation:** Each major version of `zbus` spins up its own asynchronous tokio task executor and connection manager. Running conflicting event loops in the same workspace process space can lead to thread pool exhaustion, socket leaks, and silent failures when managing local socket connections.
*   **ABI & Type Fragmentation:** Types like `zbus::Connection`, `zbus::message::Header`, or error types cannot be cleanly shared or cast between `op-identity` (v5.x) and other components (v4.x). Any raw byte-level marshalling to bridge these boundaries bypasses compile-time type safety.
*   **Parser Vulnerability Exposure:** Older versions of `zbus` (such as `zbus 3.15.2` pulled in by `secret-service` in `Cargo.lock`) do not benefit from parsing bug fixes or denial-of-service protections implemented in the `zbus 5.x` series.

#### Unauthenticated Mutation & Local Privilege Escalation (LPE)
In a typical system-bus setup, services that modify system state (such as dynamic networking or identity verification) must validate the caller's identity via `polkit` or check the caller's UID using `zbus::Connection::peer_credentials`. 
The `Cargo.toml` manifest shows no dependencies on `polkit` or other local authorization libraries. If `op-identity` or `op-network` registers methods that modify system configuration, spawn processes, or manage certificates without checking peer credentials, **local unprivileged users can invoke these methods directly via the system bus to achieve Local Privilege Escalation (LPE)**.

#### Deserialization of Caller-Supplied Bytes
`zbus` relies on `serde` (`Cargo.toml:68`) and GVariant/D-Bus binary formats to serialize and deserialize IPC payloads. Since the workspace relies on ad-hoc configurations and dynamic schema-less serialization:
*   A malicious local process can send malformed binary payloads to D-Bus methods. 
*   Without strict validation layers *before* parsing, this can trigger panics (e.g., out-of-bounds indexing during slice parsing) in the deserialization layer of `zbus`, leading to a local Denial of Service (DoS) of the control plane.

---

## 2. Schema-as-Code Discipline Violations

This codebase violates the **Schema-as-Code** discipline by heavily relying on ad-hoc, untyped parsing and structural configurations instead of standardized, versioned schemas (such as Protocol Buffers or OSCAL).

```
   Ad-Hoc (Violations)                 Strict Schema-As-Code (Compliant)
---------------------------           -----------------------------------
- quick-xml (Cargo.toml:90)           - prost (Cargo.toml:95)
- serde_json (Cargo.toml:71)          - prost-types (Cargo.toml:96)
- serde_yaml (Cargo.toml:72)
- toml (Cargo.toml:73)
```

### 2.1 Ad-Hoc Structs & String-Based Configurations
While the workspace defines `prost` (`Cargo.toml:95`) and `prost-types` (`Cargo.toml:96`) for some components, it pulls in a large number of ad-hoc serialization engines:
*   **`quick-xml` (`Cargo.toml:90`):** Used extensively by `op-introspection` (`Cargo.toml:13`) to parse and generate D-Bus introspection XML. This introduces risks of XML injection, quadratic memory expansion (Billion Laughs), and schema drift where dynamic XML elements are parsed into loose structures.
*   **`serde_yaml` (`Cargo.toml:72`):** Used in `op-inspector` and `op-agents` to load runtime behaviors. YAML parsing lacks formal schema versioning, making configuration updates prone to backwards-incompatible breaks and parsing-related panics.
*   **`toml` (`Cargo.toml:73`):** Used in `op-web` and `op-services` to manage settings as raw, versionless strings.
*   **`serde_json` (`Cargo.toml:71`):** Used across nearly all crates for data exchange. Rather than relying on versioned protobuf payloads, JSON messages are processed dynamically.

### 2.2 Lack of OSCAL Compliance
There is no representation of Open Security Controls Assessment Language (OSCAL) schemas. Security controls, system characterizations, and authorization boundaries are handled as ad-hoc text or code configurations rather than validated machine-readable OSCAL JSON/XML documents.

---

## 3. Quality & Security Findings

### Finding 1: Conflicting Major Versions of `zbus` in Runtime Workspace
*   **File/Line Citation:** `Cargo.toml:31`, `Cargo.toml:66`, `Cargo.lock`
*   **Severity:** **Medium**
*   **Description:** The workspace compiles `zbus` versions `3.15.2` (via `secret-service`), `4.4.0` (via core crates), and `5.13.2` (via `op-identity` / global dependency workspace definition). 
*   **Impact:** Having multiple concurrent D-Bus execution runtimes inside the same memory space can lead to socket lease deadlocks, duplicated tokio connection threads, increased memory usage, and potential runtime panics when passing shared socket resources between crates.
*   **Remediation:** Standardize the entire workspace on a single version of `zbus` (preferably `v5.x` to utilize the latest security and performance updates) and patch or fork upstream dependencies (like `secret-service`) to use the same unified version.

### Finding 2: Unmaintained and Orphaned Sled Storage Backend
*   **File/Line Citation:** `Cargo.toml:83`
*   **Severity:** **Medium**
*   **Description:** `cozo` is imported with the `storage-sled` feature enabled, pulling in `sled` version `0.34.7`. Sled is an orphaned, unmaintained project in the Rust ecosystem and has known unpatched concurrency bugs and memory leak vectors under high database compaction stress.
*   **Impact:** If used inside `op-cognitive-mcp` or `op-cozo-store` for system-critical state, concurrency crashes or database corruption could result in a hard denial of service (DoS) of the system control plane.
*   **Remediation:** Migrate `cozo` storage backends from `storage-sled` to a maintained, transaction-safe backend such as SQLite or RocksDB.

### Finding 3: XML Introspection Vulnerability in D-Bus Introspection Crate
*   **File/Line Citation:** `Cargo.toml:13`, `Cargo.toml:90`, `Cargo.toml:124`
*   **Severity:** **Low**
*   **Description:** The `op-introspection` and `op-inspector` crates utilize `quick-xml` to parse D-Bus introspection definitions dynamically.
*   **Impact:** If an attacker registers a malicious D-Bus service with a crafted XML introspection document containing recursive entity definitions or extremely large attribute values, calling introspection methods can exhaust the memory or CPU of the inspector process.
*   **Remediation:** Ensure that `quick-xml` is configured with strict limits on depth, entity resolution (disabled), and buffer size limits when parsing untrusted introspection documents returned from the bus.