# D-Bus & IPC Attack Surface Audit Report

## 1. D-Bus & IPC Attack Surface Analysis

### Registered Interfaces, Methods, and Signals
Based strictly on the provided files in the `FILES` section, **no D-Bus interfaces, methods, or signals are directly defined or registered** in `op-inspector`. 

The `Cargo.toml` and `Cargo.lock` indicate that `op-inspector` is used as a library component alongside other control-plane components (such as `op-dbus` and `op-dbus-mirror`), and it lists `op-introspection` as a dependency. However, there are no active `#[dbus_interface]` attributes, `zbus::connection` setups, or D-Bus signal registrations inside the audited source code of `op-inspector`.

### Bus Connection Type
No direct session or system bus connection is initialized within the provided files of this crate.

### Deserialization of Unvalidated Caller-Supplied Bytes
Multiple locations within the universal parser module accept raw, caller-supplied bytes or strings and deserialize them without structural or cryptographic validation:

*   **YAML Deserialization (`introspective_gadget.rs:347`)**:
    ```rust
    let parsed: Value = serde_yaml::from_str(data)?;
    ```
    The `data` parameter is sourced from `input.data` inside `InspectionInput`. Deserializing arbitrary, untrusted YAML can lead to resource exhaustion or denial-of-service (DoS) if complex anchors or recursive structures (e.g., billion laughs attack) are utilized.
*   **JSON Deserialization (`introspective_gadget.rs:273`)**:
    ```rust
    let parsed: Value = unsafe { simd_json::from_str(&mut data_mut)? };
    ```
    Using `unsafe` `simd_json::from_str` with raw caller-supplied input is efficient but carries risk if the underlying mutable string buffer `data_mut` does not strictly satisfy alignment, padding, or lifecycle constraints expected by the SIMD hardware implementation.

---

## 2. Process Spawning & Command Injection Vectors

### CRITICAL: Arbitrary Command Execution via GCloud Schema Deserialization
*   **Location**: `crates/op-inspector/src/datadump.rs` lines 140–151
*   **Vulnerability Type**: Privilege Escalation / Arbitrary Process Spawning
*   **Impact**: Critical

#### Description
The `DataDumper` executing discovered commands from parsed schemas uses the following logic to run commands:
```rust
// Parse the command into parts
let parts: Vec<&str> = cmd.full_command.split_whitespace().collect();
if parts.is_empty() {
    return Ok(Vec::new());
}

let mut command = Command::new(parts[0]);
for part in &parts[1..] {
    command.arg(part);
}
```
The `full_command` field of `DataCommand` is populated from the `full_path` field of a `GCloudCommand` nested within `GCloudSchema`. Because `GCloudSchema` derives `serde::Deserialize` and is designed to represent data imported into the system, any external interface (e.g., a REST endpoint, D-Bus service, or local RPC) that deserializes a user-supplied schema can be used to inject arbitrary executables. 

By splitting `cmd.full_command` by whitespace and executing `parts[0]` as the binary name, `Command::new` will execute *any* binary available in the system's `PATH` rather than constraining execution strictly to the safe `/usr/bin/gcloud` binary. If the inspector daemon runs with elevated system privileges, this allows direct, unauthenticated privilege escalation to root.

#### Remediation
Do not allow the binary path or name to be dynamically resolved from the deserialized schema. Bind the executor strictly to a constant, absolute path to the intended tool:
```rust
// Hardcode the binary to execute
let mut command = Command::new("/usr/bin/gcloud");
// Only pass safe arguments parsed from the schema
```

---

### HIGH: Argument Injection in Container Introspection
*   **Location**: `crates/op-inspector/src/introspective_gadget.rs` lines 177–183 and 199–203
*   **Vulnerability Type**: Argument Injection
*   **Impact**: High

#### Description
The specialized Docker container inspector accepts a `container_name` parameter and executes:
```rust
let inspect_output = tokio::process::Command::new("docker")
    .args(["inspect", container_name])
    ...
```
and
```rust
let top_output = tokio::process::Command::new("docker")
    .args(["top", container_name])
    ...
```
While this does not invoke a shell wrapper, passing `container_name` directly allows an attacker to supply a string beginning with dashes (e.g., `--help`, `--format`, or daemon-specific flags). If the input is exposed to untrusted callers, this allows argument injection on the local Docker client, which can alter CLI behavior or compromise the local Docker socket context.

#### Remediation
Sanitize the `container_name` to ensure it matches strict alphanumeric formatting (`^[a-zA-Z0-9_-]+$`) or insert the end-of-options delimiter (`--`) before the parameter:
```rust
let inspect_output = tokio::process::Command::new("docker")
    .args(["inspect", "--", container_name])
```

---

## 3. Schema-as-Code Discipline Compliance

This codebase utilizes a strict schema-as-code discipline using Protocol Buffers and OSCAL. Flagged below are the modules in `op-inspector` where data contracts are expressed as ad-hoc Rust structs rather than versioned, centralized schemas.

### Ad-hoc CLI Schema Contracts
*   **Location**: `crates/op-inspector/src/cli.rs` lines 31–105
*   **Violation**: The root `CliSchema`, `CliCommand`, `CliFlag`, `CliArg`, and `CliStats` are declared as ad-hoc Rust structs with custom Serde implementations. These structures define the fundamental data contracts for CLI structural representation but lack versioned serialization or Protocol Buffer equivalents.

### Ad-hoc GCloud Schema Contracts
*   **Location**: `crates/op-inspector/src/gcloud.rs` lines 42–108
*   **Violation**: The `GCloudSchema`, `GCloudStats`, `GCloudCommand`, `GCloudFlag`, and `GCloudArg` are declared as ad-hoc Rust structs. They duplicate structural parsing contracts without linking back to versioned, centralized protocol definitions.

### Ad-hoc Introspective Gadget Structures
*   **Location**: `crates/op-inspector/src/introspective_gadget.rs` lines 52–64, lines 475–496, and lines 587–634
*   **Violation**: Structs representing critical payload transfers such as `SchemaDefinition`, `InspectionInput`, `InspectionResult`, `ContainerInspectionWithKnowledge`, `ContainerInspection`, `XmlInspection`, `LegacyInspection`, and `BinaryPattern` are modeled strictly as local Rust structs. This bypasses the centralized schema engine and prevents formal OSCAL or Protocol Buffer validations.

#### Remediation
Migrate all shared struct models under `cli.rs`, `gcloud.rs`, and `introspective_gadget.rs` to Proto3 files located in a centralized schemas directory (e.g., generating them via `prost-build` inside `op-core`). Ensure all exposed state formats conform to versioned schemas.