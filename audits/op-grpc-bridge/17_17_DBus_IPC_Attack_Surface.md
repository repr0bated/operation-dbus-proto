# D-Bus & IPC Attack Surface Security and Quality Audit

## 1. D-Bus & IPC Attack Surface Map

This codebase acts as a bidirectional D-Bus $\leftrightarrow$ gRPC bridge. It connects exclusively as a client/proxy to the **System Bus** (`zbus::Connection::system()`) to route local Linux control-plane actions on behalf of remote gRPC callers. 

Below is the complete map of D-Bus interfaces, methods, and signals used or proxied across the codebase:

| D-Bus Interface | Object Path | Destination Bus Name | Methods / Actions Called | Caller Identity Checked at D-Bus Layer? | State Mutating / Process Spawning? |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `org.freedesktop.DBus.Properties` | Dynamic (user-supplied) | `org.opdbus.{plugin_id}.v1` | `Get`, `Set` | **No** (Proxied directly from gRPC payload) | Yes (for `Set`) |
| `org.opdbus.OvsdbV1` | `/org/opdbus/v1/ovsdb` | `org.opdbus.v1` | `list_dbs`, `get_schema`, `transact`, `dump`, `monitor` | **No** (Proxied directly from gRPC payload) | Yes (`transact` mutates Open_vSwitch state) |
| `org.opdbus.MailV1` | `/org/opdbus/v1/mail` | `org.opdbus.v1` | `send_email`, `get_inbox`, `get_message`, `get_status`, `list_accounts`, `admin_action`, `check_server` | **No** | Yes (Sends emails, performs admin actions) |
| `org.opdbus.PrivacyV1` | `/org/opdbus/v1/privacy` | `org.opdbus.v1` | `ensure_network`, `get_status`, `provision_user`, `get_wireguard_config`, `manage_component`, `get_topology`, `health_check`, `configure_routing`, `generate_keypair` | **No** | Yes (Configures routing, regenerates WireGuard keypairs, manages OS-level components) |
| `org.opdbus.RegistrationV1` | `/org/opdbus/v1/registration` | `org.opdbus.v1` | `send_magic_link`, `verify_magic_link`, `register_user`, `get_user_status`, `list_users`, `get_wireguard_config`, `admin_user_action` | **No** | Yes (Registers/modifies users, issues cryptographically signed tokens) |

---

## 2. Security Vulnerabilities & Attack Surface Findings

### CRITICAL: Memory Corruption & Denial of Service (DoS) via Unvalidated Shared Memory Map
* **File Citation:** `crates/op-grpc-bridge/src/interceptor.rs:59-75`
* **Vulnerability Type:** Out-of-bounds Read / Memory Corruption
* **Exploitability:** **Directly Exploitable**.
* **Description:** 
  The gRPC middleware interceptor maps a raw database file from the memory-backed file system `/dev/shm/plugin_schema.dat` via `memmap2::MmapOptions::new().map(&file)`. It then immediately casts the raw pointer to a shared memory structure:
  ```rust
  let sled_ptr = mmap.as_ptr() as *const IdentitySled;
  let is_valid = unsafe { (*sled_ptr).is_valid };
  let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
  ```
  The interceptor **completely fails to validate the size of the mapped file** before casting and dereferencing the pointer. If `plugin_schema.dat` is empty (0 bytes) or smaller than `std::mem::size_of::<IdentitySled>()` (which is $\ge 73$ bytes), dereferencing `(*sled_ptr)` triggers an immediate segmentation fault (`SIGSEGV`) or bus error (`SIGBUS`) on memory-access. 
  Because `/dev/shm` is a world-writable partition on many default Linux systems, any unprivileged local user or compromised container/plugin can truncate `plugin_schema.dat` to 0 bytes. When the next gRPC request is intercepted on port `50051`, the entire gRPC server process crashes instantly, resulting in a persistent Denial of Service (DoS).
* **Remediation:** 
  Always query and validate the file length before casting mapped memory:
  ```rust
  let metadata = file.metadata().map_err(|_| Status::internal("Metadata error"))?;
  if metadata.len() < std::mem::size_of::<IdentitySled>() as u64 {
      return Err(Status::internal("Corrupted Identity Sled size. Connection dropped."));
  }
  ```

---

### CRITICAL: Arbitrary JSON Injection in OVSDB transact Interface
* **File Citation:** `crates/op-grpc-bridge/src/grpc_server.rs:665-684`
* **Vulnerability Type:** Injection Attack (JSON Injection)
* **Exploitability:** **Directly Exploitable**.
* **Description:** 
  The `OvsdbMirror::transact` method takes a user-supplied JSON payload (`operations_json`) directly from the gRPC input and dynamically interpolates it into a D-Bus method call string:
  ```rust
  let ops = &req.operations_json;
  let call_arg = format!("[\"{}\", {}]", db, ops);
  match self.ovsdb_call("transact", &call_arg).await { ... }
  ```
  This is a structural injection vulnerability equivalent to SQL Injection. Because the input `ops` string is not parsed, sanitized, or validated as a safe JSON array prior to serialization, an attacker can supply unbalanced JSON brackets (e.g., `], ["OvsdbTable", "delete"...`) to breakout of the structured query boundary. Since OVSDB governs systemic network bridge topologies, port assignments, and interfaces on the host operating system, an attacker with network access to port `50051` can alter virtual routing and execute arbitrary transactions against the Open_vSwitch database.
* **Remediation:** 
  Parse the incoming string into a structured representation (e.g., `serde_json::Value` or a custom AST) before passing it over D-Bus, or pass strongly typed fields rather than a raw, ad-hoc JSON string.

---

### HIGH: Privilege Escalation due to Missing Identity & Capability Checks
* **File Citation:** `crates/op-grpc-bridge/src/grpc_server.rs:1145-1153` (Mail Admin), `crates/op-grpc-bridge/src/grpc_server.rs:1416-1430` (Privacy Manager), `crates/op-grpc-bridge/src/grpc_server.rs:1682-1691` (User Registration Admin)
* **Vulnerability Type:** Authentication / Authorization Bypass
* **Exploitability:** **Directly Exploitable**.
* **Description:** 
  The gRPC server exposes several high-privilege administrative endpoints, such as `admin_user_action`, `admin_mail_action`, and `manage_component`. 
  While the global interceptor in `interceptor.rs` verifies that the client has a valid session footprint, **there are no fine-grained role or capability-based authorization checks** inside the gRPC methods themselves. The incoming `actor_id` and `capability_id` parameters are extracted but are purely log/state metadata; they are not evaluated against any access control list (ACL) or capability provider prior to triggering execution on the system bus. 
  Any client with a standard, non-privileged footprint token can execute arbitrary admin commands (such as suspending users, changing mail server states, or stopping system services).
* **Remediation:** 
  Enforce cryptographic validation of capabilities or query a policy engine before translating gRPC administrative requests into system D-Bus mutations.

---

### MEDIUM: Argument Injection / Potential Switch Injection in dinitctl Calls
* **File Citation:** `crates/op-grpc-bridge/src/grpc_server.rs:858-874`
* **Vulnerability Type:** Argument Injection
* **Exploitability:** **High**.
* **Description:** 
  The `get_service` method maps directly to a host CLI call via:
  ```rust
  let name = &request.get_ref().service_name;
  let output = tokio::process::Command::new("dinitctl")
      .args(["status", name])
      .output()
      .await
  ```
  While `tokio::process::Command` does not invoke an intermediate shell (protecting against raw shell-metacharacter execution), it passes the variable `name` directly to the `dinitctl` binary. If `name` begins with a hyphen (e.g., `--help`, `--version`, or target switches specific to dinit), it can cause unexpected command flags to execute, altering binary behavior, logging, or crashing the init supervisor helper.
* **Remediation:** 
  Sanitize the `service_name` to ensure it only contains alphanumeric characters, periods, or underscores, and explicitly block service names starting with a hyphen (`-`). Alternatively, insert a double-hyphen separator to force subsequent parameters to be parsed as positional arguments:
  ```rust
  .args(["status", "--", name])
  ```

---

## 3. Schema-as-Code & Data Contract Violations

The codebase extensively violates the **Schema-as-Code** discipline. Instead of defining versioned, typed Protocol Buffer messages for all systemic data contracts, the gRPC endpoints serve as a transit layer that serializes inputs into **ad-hoc, untyped JSON strings** before passing them over D-Bus.

### Key Schema-as-Code Violations found:

1. **Ad-Hoc JSON Building for Mail Operations:**
   * **File Citation:** `crates/op-grpc-bridge/src/grpc_server.rs:920-927`
   * **Ad-Hoc Struct:**
     ```rust
     let args = simd_json::json!({
         "from": req.from_email,
         "to": req.to_email,
         "subject": req.subject,
         "body": req.body,
         "is_html": req.is_html,
         "domain": req.domain
     });
     let args_str = args.to_string();
     ```
     This structure is compiled on the fly as an untyped JSON string and passed to the D-Bus proxy. This completely bypasses Protocol Buffer compilation and formal interface definition.

2. **Ad-Hoc JSON for Privacy / Packet Routing Configurations:**
   * **File Citation:** `crates/op-grpc-bridge/src/grpc_server.rs:1783-1792`
   * **Ad-Hoc Struct:**
     ```rust
     let args = simd_json::json!({
         "container_name": req.container_name,
         "container_type": req.container_type,
         "enable_http_proxy": req.enable_http_proxy,
         "enable_grpc_proxy": req.enable_grpc_proxy,
         "proxy_type": req.proxy_type,
         "socks_port": req.socks_port,
         "http_port": req.http_port,
         "enable_tproxy": req.enable_tproxy
     });
     ```
     These properties are critical to host-level routing configurations, yet they are encoded as dynamic, schema-less map-keys. Any mismatch in string keys between the gRPC bridge and the local privacy daemon will lead to silent parsing failures.

3. **Dynamic User Provisioning Arguments:**
   * **File Citation:** `crates/op-grpc-bridge/src/grpc_server.rs:1488-1494`
   * **Ad-Hoc Struct:**
     ```rust
     let args = simd_json::json!({
         "email": req.email,
         "wireguard_public_key": req.wireguard_public_key,
         "is_admin": req.is_admin,
         "domain": req.domain,
         "container_type": req.container_type
     });
     ```
     Security-sensitive identity parameters (such as administrative status (`is_admin`) and public keys) are packaged dynamically inside a raw JSON string.

4. **Magic Link / Registration Requests:**
   * **File Citation:** `crates/op-grpc-bridge/src/grpc_server.rs:1882-1886`
   * **Ad-Hoc Struct:**
     ```rust
     let args = simd_json::json!({
         "email": req.email,
         "domain": req.domain,
         "is_admin": req.is_admin
     });
     ```
     Untyped parameters are serialized into string format, preventing structural schema verification of administrative enrollment requests.

### Architectural Impact:
The system fails to maintain schema integrity because the IPC boundary is bridged using unstructured strings. This type-erasure pattern leads to:
* **Fragility:** Renaming or changing fields on either side of the D-Bus daemon silently breaks the integration without compile-time errors.
* **Loss of Auditability:** Ad-hoc JSON serialization cannot be statically verified against OSCAL compliance controls or native Protocol Buffer descriptors.