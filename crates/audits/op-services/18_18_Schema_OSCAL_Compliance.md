An audit of the codebase has been completed. Below is the report detailing Schema-as-Code compliance, OSCAL coverage, and security vulnerabilities.

## 1. Schema-as-Code Audit

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| D-Bus Interface Payloads | D-Bus Methods | `crates/op-services/src/dbus/interface.rs:25`, `crates/op-services/src/dbus/interface.rs:39`, `crates/op-services/src/dbus/interface.rs:53`, `crates/op-services/src/dbus/interface.rs:67` | No | Structured status is serialized to an ad-hoc JSON string (`serde_json::to_string`) rather than using structured, versioned D-Bus types or a versioned schema contract. |
| SQLite Service Definitions | Database Storage | `crates/op-services/src/store/mod.rs:31`, `crates/op-services/src/store/mod.rs:72` | No | Structured service configurations are persisted directly in a SQLite database as unversioned JSON strings. |
| Duplicate `ServiceDef` Representations | Data Contract | `crates/op-services/src/schema/mod.rs:6` | Yes (partial) | The codebase maintains duplicate representations of the service definition contract (`schema::ServiceDef` and protobuf `ServiceDef`), requiring a fragile, hand-rolled conversion routine (`proto_to_schema_def` in `crates/op-services/src/grpc/server.rs:244`). |
| Hand-rolled DB Migrations | DB Schema | `crates/op-services/src/store/mod.rs:28-59` | No | SQLite tables (`services` and `audit_log`) are defined using ad-hoc, raw SQL DDL strings inside Rust code rather than a unified schema-as-code schema engine or versioned migration files. |

---

## 2. OSCAL Compliance & Security Coverage Audit

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-3 (Access Enforcement) / Authorization** | `crates/op-services/src/bin/op-services.rs:41-47`<br>`crates/op-services/src/grpc/server.rs:33` | None | **[CRITICAL]** The gRPC server listens on a wildcard network interface (`[::]:50053`) with zero authentication, authorization, or TLS verification, enabling unauthenticated remote command execution (RCE) via service creation. |
| **AC-6 (Least Privilege) / Privilege Enforcement** | `crates/op-services/src/manager/process.rs:31` | None | **[CRITICAL]** The fallback process manager spawner ignores configured user and group settings (`service.user` and `service.group`), running child processes as root. |
| **AU-2 (Event Logging) / Audit Log** | `crates/op-services/src/store/mod.rs:109` | None | **[MAJOR]** The `Store::audit` logging interface is defined but never invoked inside the service manager, leaving all administrative state changes (start, stop, create, delete) completely unlogged. |
| **D-Bus & gRPC Endpoint Documentation** | `crates/op-services/src/dbus/interface.rs:18`<br>`crates/op-services/src/grpc/server.rs:33` | None | **[MAJOR]** External control plane boundaries are not registered in a machine-readable OSCAL component definition file or system architecture catalog. |

---

## 3. Detailed Findings & Recommendations

### [CRITICAL] Remote Code Execution via Unauthenticated gRPC Service Creation
- **File & Line**: `crates/op-services/src/grpc/server.rs:136`, `crates/op-services/src/bin/op-services.rs:41-47`
- **Impact**: Any network-adjacent actor can connect to the default gRPC endpoint on `[::]:50053` and call the `create` method with a malicious `ServiceDef` (e.g. executing shell commands), and then invoke `start` to execute that command as `root` on the target system. This constitutes a trivial Remote Code Execution (RCE) vulnerability.
- **Remediation**:
  1. Mandate mutual TLS (mTLS) with client certificate verification in `Server::builder()`.
  2. Implement a gRPC interceptor that checks for authorization tokens or unix socket domain credentials if local-only access is intended.
  3. Change the default listening interface from wildcard `[::]` to local loopback `127.0.0.1` or bind to a Unix Domain Socket with restricted permissions.

### [CRITICAL] Privilege Escalation via Fallback Process Manager Execution
- **File & Line**: `crates/op-services/src/manager/process.rs:31`
- **Impact**: The service manager extracts user and group permissions from incoming service definitions in `proto_to_schema_def` (at `crates/op-services/src/grpc/server.rs:264-265`), but `ProcessManager::start` completely ignores these fields. Every program started under the fallback manager runs with the absolute privileges of the parent daemon process (usually `root`), facilitating privilege escalation if non-privileged services are requested.
- **Remediation**:
  Modify `ProcessManager::start` to configure `uid` and `gid` on the child process using `std::os::unix::process::CommandExt`:
  ```rust
  use std::os::unix::process::CommandExt;

  if let Some(ref user) = service.user {
      if let Ok(uid) = parse_user_to_uid(user) {
          cmd.uid(uid);
      }
  }
  if let Some(ref group) = service.group {
      if let Ok(gid) = parse_group_to_gid(group) {
          cmd.gid(gid);
      }
  }
  ```

### [MAJOR] Non-Functional System Audit Logging
- **File & Line**: `crates/op-services/src/store/mod.rs:109`
- **Impact**: The SQLite database contains an `audit_log` table definition and a corresponding `audit` method on `Store`, but this method is never called in `ServiceManager`. Critical actions like creating a service, deleting a service, or starting/stopping system elements go unlogged.
- **Remediation**:
  Add audit logging calls directly in the state transitions inside `crates/op-services/src/manager/service_manager.rs`:
  ```rust
  // Inside ServiceManager::start
  self.store.audit(Some(name.as_str()), "START_SERVICE", None).await?;
  ```

### [MAJOR] Schema-as-Code Violation: Ad-hoc JSON Serialization Over D-Bus
- **File & Line**: `crates/op-services/src/dbus/interface.rs:25` (and lines 39, 53, 67)
- **Impact**: Returning serialized JSON strings inside a `zbus` interface violates the schema-as-code discipline. Client tools must parse unstructured text payloads rather than structured interfaces. It prevents automatic type validation by the D-Bus daemon and increases integration fragility.
- **Remediation**:
  Expose structured Rust structs implementing `zvariant::Type` directly via the D-Bus methods, or implement a versioned protobuf format over D-Bus instead of returning raw JSON string slices.

### [MAJOR] Missing OSCAL Component Definitions
- **File & Line**: `crates/op-services/src/lib.rs:1`
- **Impact**: High-exposure system-wide services lack machine-readable OSCAL `component-definition` descriptors. Audit validators and security scanners cannot map implemented boundaries and controls to NIST 800-53 baseline controls programmatically.
- **Remediation**:
  Create an OSCAL `component-definition.json` representing the `op-services` system service manager component, documenting endpoints (such as D-Bus interface `org.opdbus.services` and gRPC service `opdbus.services.v1.ServiceManager`) and linking them directly to NIST controls (specifically AC-3, AU-2, and AC-6).

---
## ⚠ Citation Warnings
- `crates/op-services/src/schema/mod.rs:6`: file has 5 lines
