# Configuration Analysis

## 1. Environment Variable (`std::env::var`) Reads

The environment variables are read in `crates/op-grpc-bridge/src/schema_engine.rs` within the mutation function.

| File & Line | Environment Variable | Purpose | Default / Fallback Handling |
| :--- | :--- | :--- | :--- |
| `crates/op-grpc-bridge/src/schema_engine.rs:462` | `SCHEMA_UUID` | OSCAL Schema Unique Identifier | `unwrap_or_default()` (empty string) |
| `crates/op-grpc-bridge/src/schema_engine.rs:463` | `SCHEMA_SUBID` | OSCAL Sub-component ID | `unwrap_or_default()` (empty string) |
| `crates/op-grpc-bridge/src/schema_engine.rs:464` | `SCHEMA_CONTROL_SOURCE` | Compliance Control Source framework | `unwrap_or_else(|_| "NIST_SP_800_53_R5".into())` |
| `crates/op-grpc-bridge/src/schema_engine.rs:466` | `SCHEMA_CONTROL_REFS` | Reference pointers to OSCAL security controls | `unwrap_or_default()` (empty string) |
| `crates/op-grpc-bridge/src/schema_engine.rs:467` | `SCHEMA_STATEMENT_REFS` | Reference pointers to OSCAL control statements | `unwrap_or_default()` (empty string) |
| `crates/op-grpc-bridge/src/schema_engine.rs:468` | `NEXTDNS_PROFILE_ID` | Direct read of DNS profile configuration | `unwrap_or_else(|_| "689ec7".into())` |

### Environment Variables Flagged with Safety/Quality Issues
All read variables provide safe fallback defaults (via `unwrap_or_default` or `unwrap_or_else`), which prevents immediate runtime panic if they are missing. However, there are **quality and validation gaps**:
* **No Format/Type Validation**: `SCHEMA_UUID` (`crates/op-grpc-bridge/src/schema_engine.rs:462`) is treated as a generic string without verifying that it conforms to a standard UUID format. An invalid or malformed UUID will propagate into the identity sled downstream without error.
* **No Length Verification**: `NEXTDNS_PROFILE_ID` (`crates/op-grpc-bridge/src/schema_engine.rs:468`) is assumed to be a valid 6-hex-character identifier but accepts arbitrary input strings, potentially causing malfunctions or injection risks when passed to DNS control tools.

---

## 2. Cargo Features & Additivity

### Root `Cargo.toml` Features
The workspace-level features are:
* **`default`**: `["grpc"]` (Enables the gRPC integration transport layer by default)
* **`grpc`**: `[]` (Configures and builds the D-Bus-to-gRPC bidirectional services)

### Additivity Analysis
Cargo features are **strictly additive**. Activating `grpc` simply enables the compilation of D-Bus-to-gRPC bridges. If multiple dependencies transitively include `op-dbus`, the union of all active features is built. Since there are no mutually exclusive features in the workspace configuration, this adheres to Cargo best practices.

---

## 3. Hardcoded Paths, Ports, and Addresses

| File & Line | Classification | Hardcoded Value | Impact |
| :--- | :--- | :--- | :--- |
| `crates/op-grpc-bridge/src/interceptor.rs:46` | Path | `/dev/shm/plugin_schema.dat` | Restricts schema memory layout mapping exclusively to shared memory-mounted paths. Non-portable outside Linux hosts with standard `/dev/shm` mounts. |
| `crates/op-grpc-bridge/src/grpc_client.rs:33` | Address/Port | `http://127.0.0.1:50051` | Default fallback endpoint address for distributed operation deployments if no remote endpoint is provided. |
| `crates/op-grpc-bridge/src/grpc_server.rs:248` | Port / Log | `0.0.0.0:50051` | Hardcoded port reported during gRPC service force-binding. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1042` | Path | `/etc/hostname` | Reads local system hostname directly from host virtual filesystem. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1047` | Path | `/proc/version` | Inspects local Linux kernel version directly. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1054` | Path | `/proc/uptime` | Retrieves system uptime directly from kernel interface. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1061` | Path | `/proc/meminfo` | Parses system-wide memory metrics from virtual procfs. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1072` | Path | `/run/dinitctl` | Determines local init system presence via dinit control socket path. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1175` | Path | `/sys/class/net` | Enumerates local network devices from sysfs. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1222` | Path | `/sys/devices/system/node` | Hardcoded path to traverse CPU socket topologies in NUMA systems. |
| `crates/op-grpc-bridge/src/grpc_server.rs:1234` | Path | `/sys/devices/system/node/{}/meminfo` | Accesses per-socket physical memory allocation directly. |

---

# Schema-as-Code Compliance

The project enforces versioned schema rules via generated Protocol Buffers (`operation.v1`). However, several violations exist where data contracts are defined as ad-hoc strings or raw native structures rather than versioned, formalized schemas:

1. **Ad-hoc Shared Memory Representation Structure**
   * `crates/op-grpc-bridge/src/interceptor.rs:18-24`: `struct IdentitySled` is represented as a raw C-packed struct (`#[repr(C)]`). This structure maps directly to shared memory using pointer casting (`crates/op-grpc-bridge/src/interceptor.rs:53`). 
   * **Violation**: If compiler layouts or padding configurations change across rustc updates, or if a separate engine compiles with a differing struct definition, the shared memory layout will silently corrupt. It is not managed via an OSCAL or Protobuf model format.
2. **Ad-hoc String Manipulation for Database Tables**
   * `crates/op-grpc-bridge/src/grpc_server.rs:857-858`, `867-868`, `916-921`: Database queries, schemas, and transactions are serialized into raw, untyped JSON strings (e.g., `"Open_vSwitch"`, `&format!("[\"{}\", {}]", db, ops)`).
   * **Violation**: Instead of validating transaction requests against structured protocol definitions, parameters are formatted as raw strings. Changes to database schemas will lead to silent parsing runtime failures instead of compile-time schema mismatches.
3. **Ad-hoc State Mutations**
   * `crates/op-grpc-bridge/src/grpc_server.rs:488-495`, `512-524`: Bypasses versioned schemas to submit generic payload states inside an untyped JSON block (`simd_json::json!({...})`) which is recorded into the state store directly as generic data.

---

# Security & Quality Findings

## Critical Vulnerabilities (Directly Exploitable)

### 1. Out-of-Bounds Memory Read via Unchecked Shared Memory Length
* **Reference**: `crates/op-grpc-bridge/src/interceptor.rs:45-56`
* **Vulnerability Type**: Out-of-bounds Read / Undefined Behavior / Denial of Service
* **Exploitation Vector**: Local Attacker
* **Description**:
  The function `ghostbridge_interceptor` opens and mmaps the shared memory file `/dev/shm/plugin_schema.dat`:
  ```rust
  let file = File::open("/dev/shm/plugin_schema.dat")
      .map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

  let mmap = unsafe {
      MmapOptions::new()
          .map(&file)
          .map_err(|_| Status::internal("Mmap failed"))?
  };
  let sled_ptr = mmap.as_ptr() as *const IdentitySled;

  let is_valid = unsafe { (*sled_ptr).is_valid };
  let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
  ```
  The code **does not verify the mapped file size** before casting and dereferencing the pointer. If `plugin_schema.dat` contains any data but has a size smaller than `std::mem::size_of::<IdentitySled>()` (which is approximately 80 bytes), the mapping succeeds (since `mmap` can map files smaller than a page on some filesystems, or if size > 0). When the code dereferences `(*sled_ptr).is_valid` or `(*sled_ptr).hashed_footprint` on lines 55 and 56, it reads memory beyond the bounds of the mapped file.
* **Remediation**:
  Check the file's metadata size before performing the memory mapping, ensuring it is at least `std::mem::size_of::<IdentitySled>()` bytes:
  ```rust
  let metadata = file.metadata().map_err(|_| Status::internal("Failed to read schema metadata"))?;
  if metadata.len() < std::mem::size_of::<IdentitySled>() as u64 {
      return Err(Status::internal("Corrupted Identity Sled file size"));
  }
  ```

### 2. Local Privilege Escalation / Authentication Bypass via Shared Memory Injection
* **Reference**: `crates/op-grpc-bridge/src/interceptor.rs:45-56`
* **Vulnerability Type**: Authentication Bypass
* **Exploitation Vector**: Local Attacker / Container Escape
* **Description**:
  The gRPC interceptor authenticates inbound HTTP/2 headers by checking them against the `hashed_footprint` values read directly from `/dev/shm/plugin_schema.dat`.
  `/dev/shm` is typically a world-writable directory (`drwxrwxrwt`) on standard Linux environments. Since there is no ownership validation, cryptographic signature verification, or validation that the file was generated by a privileged root service, **any local unprivileged user or process can write a crafted binary file** to `/dev/shm/plugin_schema.dat`. 
  By writing a simulated `IdentitySled` structure with `is_valid: true` and their own `hashed_footprint` payload, an unprivileged user can force the gRPC bridge to authenticate arbitrary connections. This completely bypasses the gRPC Gatekeeper security middleware.
* **Remediation**:
  1. Move the shared schema file out of world-writable directories such as `/dev/shm` to a restricted run directory like `/run/op-grpc-bridge/` with `0700` permissions owned by the system user.
  2. Implement strict file permission checks before reading (e.g., verify that the file owner UID matches the server's running UID).

---

## Medium & Low Priority Findings

### 3. Insecure Host Command Execution via System Shell Callouts
* **Reference**: `crates/op-grpc-bridge/src/grpc_server.rs:1094`, `1131`
* **Severity**: Medium
* **Description**:
  The runtime mirror executes direct system command callouts to `dinitctl` via standard command execution:
  ```rust
  let output = tokio::process::Command::new("dinitctl")
      .arg("list")
      .output()
      .await...
  ```
  While `dinitctl` is invoked with direct arguments on line 1094, the service retrieval on line 1131 takes a dynamically passed `service_name` string from the gRPC request argument:
  ```rust
  let name = &request.get_ref().service_name;
  let output = tokio::process::Command::new("dinitctl")
      .args(["status", name])
      ...
  ```
  If `service_name` is manipulated or injected with special parameters recognized by the `dinitctl` utility, it can trigger unintended command behavior or service state changes depending on how the binary handles arguments.
* **Remediation**:
  Sanitize and restrict `service_name` using a strict whitelist regex (e.g., `^[a-zA-Z0-9_\-]+$`) before passing it as an execution argument.

### 4. Panics due to `unwrap` in Multi-User Environments
* **Reference**: `crates/op-grpc-bridge/src/interceptor.rs:62-63`
* **Severity**: Low
* **Description**:
  The interceptor extracts headers and unwraps them directly without safe error recovery:
  ```rust
  let request_footprint = footprint_value
      .as_ref()
      .unwrap()
      .to_str()
      ...
  ```
  Although a prior check on line 41 ensures that `footprint_value` is not `None`, this pattern is fragile and can lead to unexpected panics during code refactoring or concurrent middleware modifications.
* **Remediation**:
  Replace with safe `if let` or `map_err` propagation to guarantee compile-time panic-freedom.