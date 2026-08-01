# D-Bus & IPC Attack Surface Security and Quality Audit

## 1. D-Bus Interface Registry

The following interfaces, methods, and signals are registered across the reviewed codebase:

### Interface: `org.opdbus.ProjectedObjectV1`
*   **Path**: Managed dynamically
*   **Properties**:
    *   `origin_service` (Read-only, `String`): Returns the service of origin.
    *   `origin_path` (Read-only, `String`): Returns the object path of origin.
*   **Caller Identity Checked**: No validation is performed on property accessors.

### Interface: `org.opdbus.StateManager`
*   **Path**: `/org/opdbus/v1/state` (via `register_on_connection` at `crates/op-state/src/dbus_server.rs:188`)
*   **Methods**:
    *   `apply_openflow_state(state_json: String) -> Result<String>`
        *   **Description**: Updates and applies the OpenFlow-specific plugin state.
        *   **Caller Identity Checked**: **No**. Any caller can execute this.
        *   **State Mutation / Process Spawning**: Yes, mutates OpenFlow configuration state.
    *   `query_state() -> Result<String>`
        *   **Description**: Queries the current aggregated plugin states.
        *   **Caller Identity Checked**: **No**.
        *   **State Mutation / Process Spawning**: No, read-only.
    *   `apply_contract_mutation(request_json: String) -> Result<String>`
        *   **Description**: Performs a state mutation against a specified state plugin.
        *   **Caller Identity Checked**: **No**. Any caller can execute this.
        *   **State Mutation / Process Spawning**: Yes, mutates arbitrary plugin configurations.

### Interface: `org.opdbus.PluginV1`
*   **Path**: Managed dynamically per-host
*   **Properties**:
    *   `name` (Read-only, `String`): Returns the name of the plugin.
    *   `version` (Read-only, `String`): Returns the plugin version.
    *   `description` (Read-only, `String`): Returns the plugin metadata description.
*   **Methods**:
    *   `get_state() -> Result<String>`
        *   **Description**: Returns the current state of the plugin serialized as JSON.
        *   **Caller Identity Checked**: **No**.
        *   **State Mutation / Process Spawning**: No.
    *   `get_schema() -> Result<String>`
        *   **Description**: Retrieves the schema copy of the plugin from the schema registry.
        *   **Caller Identity Checked**: **No**.
        *   **State Mutation / Process Spawning**: No.

---

## 2. Security Findings

### CRITICAL: Missing Authentication & Authorization Checks on Mutating System D-Bus Methods
*   **Location**: `crates/op-state/src/dbus_server.rs:76-90` (`apply_openflow_state`), and `crates/op-state/src/dbus_server.rs:105-119` (`apply_contract_mutation`)
*   **Impact**: local privilege escalation, unauthorized modification of network topologies, arbitrary state injection.
*   **Description**:
    The service is designed to connect to the D-Bus system bus via `start_system_bus` (`crates/op-state/src/dbus_server.rs:197`). On a system bus, any local unprivileged process can interact with registered services unless restricted. 
    The methods `apply_openflow_state` and `apply_contract_mutation` allow direct mutation of the system's runtime configurations (e.g., OpenFlow routing tables, firewall policies, container states) by delegating to `self.state_manager.apply_plugin_state(...)` without:
    1. Querying the caller's credentials or connection properties (e.g., checking if the caller's UID is `0`/root via `zbus` connection context).
    2. Integrating with authentication frameworks like PolicyKit (`polkit`).
    Any local attacker can craft D-Bus method calls to inject arbitrary malicious network states, isolate the machine, reroute traffic, or corrupt state.

---

### CRITICAL: Unsafe `simd_json` Deserialization of Untrusted IPC Payloads
*   **Location**: `crates/op-state/src/dbus_server.rs:78` and `crates/op-state/src/dbus_server.rs:108`
*   **Impact**: Memory corruption, buffer overflows, or unexpected process crashes.
*   **Description**:
    The `simd_json` library requires mutable buffers and assumes strict invariants regarding padding, alignment, and string lifetimes. Deserializing untrusted strings supplied directly by the D-Bus IPC client using `unsafe { simd_json::from_str(...) }` bypasses standard safe parsing checks:
    ```rust
    // crates/op-state/src/dbus_server.rs:77-78
    let mut state_json_mut = state_json;
    match unsafe { simd_json::from_str::<DesiredState>(&mut state_json_mut) }
    ```
    If a malicious caller sends a highly nested, malformed, or specially padded JSON payload, it can trigger memory unsafety inside `simd_json`'s SIMD-optimized routines, leading to daemon crashes (DoS) or potentially arbitrary code execution inside the privileged control plane process.

---

### HIGH: Direct Process Spawning with Unchecked Execution Paths
*   **Location**: `crates/op-state/src/authority.rs:15-28` (`enforce_authority`)
*   **Impact**: Local Denial of Service, race conditions, configuration hijacking.
*   **Description**:
    The `NetworkAuthority::enforce_authority()` method executes `systemctl` commands directly to stop and disable `NetworkManager` and `systemd-networkd`:
    ```rust
    let _ = Command::new("systemctl")
        .args(["stop", "NetworkManager"])
        .output();
    ```
    This execution relies on the system path containing `systemctl`. Although it is called to protect the plugin system's authoritative control, executing raw commands without sanitizing the system environment or enforcing fully-qualified paths can lead to command-hijacking vulnerabilities if run under compromised shell environments. Furthermore, errors returned by `systemctl` are silently ignored (`let _ = ...`), which can result in a false sense of authority enforcement if the services fail to stop.

---

### MEDIUM: Ad-Hoc Cryptographic Secret Storage Permission Race Condition
*   **Location**: `crates/op-state/src/crypto.rs:101-113` (`from_key_file`)
*   **Impact**: Exposure of the state encryption key to local unprivileged processes during generation.
*   **Description**:
    When creating a new cryptographic state key file, `StateEncryption::from_key_file` writes the raw key to disk first, and *subsequently* modifies file permissions using `std::fs::set_permissions`:
    ```rust
    // Key is written with default umask first (often 0o644 or 0o666)
    std::fs::write(path, encryption.key.as_slice()).context("Failed to write key file")?;

    #[cfg(unix)]
    {
        // Permission modification occurs after the file has already been flushed to disk
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)
            .context("Failed to set key file permissions")?;
    }
    ```
    This creates a time-of-check to time-of-use (TOCTOU) race condition. A local attacker monitoring the directory can open a read handle to the key file immediately after the `write` completes but before `set_permissions` is executed, exposing the master state encryption key.

---

## 3. Schema-as-Code Compliance Violations

The codebase bypasses standard schema-as-code disciplines, relying instead on ad-hoc structs and unstructured JSON strings:

1.  **JSON over D-Bus IPC**:
    *   **Violation**: `crates/op-state/src/dbus_server.rs:76` and `crates/op-state/src/dbus_server.rs:105`
    *   Instead of exposing strongly typed endpoints using serialized Protocol Buffers or gRPC bridge schemas, data contracts are represented as raw, untyped strings (`state_json` and `request_json`). 
2.  **Ad-Hoc Workflow Context Storage**:
    *   **Violation**: `crates/op-state/src/plugin_workflow.rs:174-177` and `crates/op-state/src/plugin_workflow.rs:212-216`
    *   The workflow system pulls state and returns execution statuses using arbitrary string keys (e.g. `"desired_state"`, `"last_error"`) mapped to generic JSON elements. These contracts should be governed by versioned Protocol Buffer schemas.
3.  **Hardcoded Use-Case Schema Validation**:
    *   **Violation**: `crates/op-state/src/schema_validator.rs:13-64` and `crates/op-state/src/schema_validator.rs:166-261`
    *   Data model validation rules are defined as inline Rust code structures (`UseCaseTemplate`, `FieldCombination`, `Constraint`) rather than declarative, versioned validation schemas (such as JSON Schema drafts, OSCAL profiles, or Protobuf rulesets) loaded from unified schema catalogs.