# Production Security and Quality Audit: op-grpc-bridge

## 1. Security & Unsafe Code Audit

### 1.1 Unsafe Blocks Review
This section documents every `unsafe {` block found in the provided codebase, assesses the missing safety documentation, and details the associated exploit risks.

#### Finding 1: Unsafe Shared Memory Mapping without Boundary Validation or Safety Guarantees
* **Location:** `crates/op-grpc-bridge/src/interceptor.rs:49-53`
* **Context:**
  ```rust
  let mmap = unsafe {
      MmapOptions::new()
          .map(&file)
          .map_err(|_| Status::internal("Mmap failed"))?
  };
  ```
* **Missing Safety Comment:** 🔴 **No `// SAFETY:` comment present.**
* **Risk/Exploitability Assessment:** **CRITICAL**. This mapping reads `/dev/shm/plugin_schema.dat`. There is no validation to ensure that the file's size on disk is equal to or greater than `std::mem::size_of::<IdentitySled>()` before mapping and casting. If a local attacker or a concurrent process truncates this file to 0 bytes or a size smaller than the struct, any subsequent dereference of the mapped pointer will immediately trigger a `SIGBUS` signal, crashing the entire gRPC ingress server (Denial of Service).

#### Finding 2: Unsafe Raw C-Pointer Dereference for Struct Field Access
* **Location:** `crates/op-grpc-bridge/src/interceptor.rs:55-57`
* **Context:**
  ```rust
  let is_valid = unsafe { (*sled_ptr).is_valid };
  let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
  ```
* **Missing Safety Comment:** 🔴 **No `// SAFETY:` comment present.**
* **Risk/Exploitability Assessment:** **CRITICAL**. This dereferences a raw pointer cast directly from the `memmap2::Mmap` byte slice. If the memory-mapped file has been modified, corrupted, or is misaligned, this dereference results in undefined behavior. Furthermore, since the memory-mapped file is located in `/dev/shm` (shared memory), any unprivileged local process with access to `/dev/shm` can modify this file, bypassing the gRPC interceptor's validation state (`is_valid`) or causing out-of-bounds memory reads if they decrease the file size.

---

### 1.2 Subprocess Command Execution (`Command::new`)
A total of **2** `Command::new` spawns were identified in the codebase. Both are located in `crates/op-grpc-bridge/src/grpc_server.rs`.

#### Spawning Site 1: Static System Service List Query
* **Location:** `crates/op-grpc-bridge/src/grpc_server.rs:1134`
* **Invocation:**
  ```rust
  let output = tokio::process::Command::new("dinitctl")
      .arg("list")
  ```
* **Validation/Risk Assessment:** **Safe.** The command is hardcoded with a static argument `"list"`. There is no path for user-controlled input to modify the command parameters or invoke shell interpolation.

#### Spawning Site 2: User-Controlled Option/Argument Injection in Service Status Check
* **Location:** `crates/op-grpc-bridge/src/grpc_server.rs:1184`
* **Invocation:**
  ```rust
  let output = tokio::process::Command::new("dinitctl")
      .args(["status", name])
  ```
* **Validation/Risk Assessment:** ⚠️ **HIGH**. The variable `name` is directly extracted from the gRPC request payload (`request.get_ref().service_name`) without any sanitization or strict alphanumeric validation. 
  * Because the command is spawned directly without a shell, arbitrary shell command execution (e.g. via `;` or `&&`) is prevented.
  * However, because the user input is passed as a command argument, it is vulnerable to **Option Injection**. If the user passes a service name starting with `-` (for example, `--help` or options specific to the system's `dinitctl` binary), they can alter the execution flag of `dinitctl`. This can lead to unexpected local information disclosure, service state changes, or server hangs depending on the options supported by the target host's `dinitctl` implementation.

---

### 1.3 Forbidden Commands Assessment
No direct matches for forbidden command executions (`ovs-*` commands, raw OpenFlow tools, raw shells `bash`/`sh`, or exfiltration utilities `curl`/`wget`/`nc`) were found in the `Command::new` spawn locations. 

---

### 1.4 Hardcoded Cryptographic Tokens, IPs, and Sensitive Identifiers

#### Finding 1: Hardcoded Default NextDNS Profile Tracker ID
* **Location:** `crates/op-grpc-bridge/src/schema_engine.rs:926`
* **Code:**
  ```rust
  let nextdns = std::env::var("NEXTDNS_PROFILE_ID")
                    .unwrap_or_else(|_| "689ec7".into());
  ```
* **Severity:** **Medium**. A default profile ID `"689ec7"` is hardcoded as a fallback. This leaks a specific telemetry or DNS filtering configuration profile into the system if the corresponding environment variable is absent.

#### Finding 2: Hardcoded Network Management IP Address
* **Location:** `crates/op-grpc-bridge/src/grpc_server.rs:1018`
* **Code:**
  ```rust
  "management_ip": parsed
      .get("management_ip")
      .and_then(|v| v.as_str())
      .unwrap_or("10.200.0.1")
      .to_string(),
  ```
* **Severity:** **Low**. The IP address `"10.200.0.1"` is hardcoded as a fallback value for the gateway's management interface.

---

### 1.5 D-Bus System-Bus Method Exposure & gRPC Bridging
The gRPC bridge server implements a shared-server topology on port `50051`. Several methods exposed on this port directly map and forward commands to private, local system D-Bus interfaces. 

Because the gRPC server force-binds to `0.0.0.0:50051` (line 330 in `crates/op-grpc-bridge/src/grpc_server.rs`), any remote network peer can call these gRPC endpoints, which are then proxied directly to the system D-Bus without standard local Polkit authorization checks.

#### Exposed Capabilities & Mapped D-Bus Methods:
1. **OVSDB Administration (`OvsdbMirror`):**
   * gRPC `list_dbs` / `get_schema` / `transact` / `monitor` (Lines 777-909) invoke methods on `org.opdbus.OvsdbV1` at path `/org/opdbus/v1/ovsdb` on `org.opdbus.v1`.
   * **Security Impact:** A remote network attacker with access to port 50051 can execute raw OVSDB transactions, list local databases, and alter local Open vSwitch configurations without local root privileges.
2. **Mail Server Administration (`MailService`):**
   * gRPC `send_email` / `get_inbox` / `admin_mail_action` (Lines 1238-1440) invoke methods on `org.opdbus.MailV1` at path `/org/opdbus/v1/mail`.
   * **Security Impact:** Allows arbitrary remote users to read local webmail, fetch user inboxes, and execute administrative actions (such as deleting accounts or modifying mail routing) via system D-Bus proxies.
3. **Network Routing & Privacy Control (`PrivacyNetworkService`):**
   * gRPC `ensure_privacy_network` / `provision_user` / `configure_packet_routing` / `manage_component` (Lines 1450-1845) invoke methods on `org.opdbus.PrivacyV1` at path `/org/opdbus/v1/privacy`.
   * **Security Impact:** Remote users can generate WireGuard keypairs, modify SOCKS/TPROXY rules, and restart or stop critical local privacy-router dinit services.
4. **Identity Registration Control (`RegistrationService`):**
   * gRPC `send_magic_link` / `verify_magic_link` / `register_user` / `admin_user_action` (Lines 1855-2144) invoke methods on `org.opdbus.RegistrationV1` at path `/org/opdbus/v1/registration`.
   * **Security Impact:** Bypasses local enrollment procedures by allowing remote peers to issue administrative link tokens and register new administrator accounts.

---

## 2. Schema-as-Code Compliance

The codebase exhibits several violations of the schema-as-code discipline, where data contracts are expressed as ad-hoc nested objects or unversioned raw strings instead of strictly defined, statically generated Protobuf schemas or formal OSCAL profiles.

### 2.1 Runtime-Generated Protobuf Definitions
* **Location:** `crates/op-grpc-bridge/src/proto_gen.rs:1-350`
* **Compliance Defect:** Rather than compiling static, strictly versioned Protocol Buffer definitions (`.proto` files) during build time, this module converts an in-memory `PluginSchema` into raw string buffers at runtime (`generate_for_schema`, `generate_for_catalog`).
* **Operational Risk:** Dynamically generating protobuf text definitions at runtime bypasses compile-time verification. A drift or mutation in the plugin catalog can dynamically produce invalid syntax, leading to serialization errors, client-side deserialization failures, and silent protocol breakages.

### 2.2 Ad-hoc JSON-to-Zvariant Conversion
* **Location:** `crates/op-grpc-bridge/src/grpc_server.rs:695-775`
* **Compliance Defect:** The method arguments are passed as unstructured `simd_json::OwnedValue` objects and converted to D-Bus `ZOwnedValue` structures using ad-hoc string comparisons for type signatures (`"s"`, `"b"`, `"i"`, `"ay"`).
* **Operational Risk:** This contract is completely ad-hoc and not enforced by any machine-readable schema. If the signature string does not match precisely at runtime, the method call crashes with an unstructured `anyhow::anyhow!("Unsupported signature '{}'")` error.

### 2.3 Unstructured OSCAL Compliance Metadata Insertion
* **Location:** `crates/op-grpc-bridge/src/schema_engine.rs:917-925`
* **Compliance Defect:**
  ```rust
  let uuid          = std::env::var("SCHEMA_UUID").unwrap_or_default();
  let subid         = std::env::var("SCHEMA_SUBID").unwrap_or_default();
  let ctrl          = std::env::var("SCHEMA_CONTROL_SOURCE")
                          .unwrap_or_else(|_| "NIST_SP_800_53_R5".into());
  ```
  These parameters are loaded as unstructured environment string values and directly written into the identity sled.
* **Operational Risk:** This violates the OSCAL schema-as-code principles. It bypasses formal validation against standardized JSON/YAML schemas for NIST controls, risking the injection of malformed control references that break compliance compliance reporting tools downstream.

---

## 3. Actionable Security Recommendations

| ID | File:Line Reference | Severity | Description | Remediation |
| :--- | :--- | :--- | :--- | :--- |
| **01** | `interceptor.rs:49-58` | **Critical** | Zero-copy shared memory read is vulnerable to out-of-bounds panic and Denial of Service (SIGBUS) if `/dev/shm/plugin_schema.dat` is truncated or modified. | Check the size of the mapped file against `std::mem::size_of::<IdentitySled>()` before dereferencing, and map the memory with read-only/copy-on-write protections. Add an explicit safety comment. |
| **02** | `grpc_server.rs:1184` | **High** | Option and argument injection vulnerability via system service name input to `dinitctl status`. | Sanitize `name` with a strict regex check ensuring it only contains alphanumeric characters and dashes (`^[a-zA-Z0-9_-]+$`). Do not allow inputs starting with `-`. |
| **03** | `grpc_server.rs:330` | **High** | gRPC port force-binds to `0.0.0.0`, exposing highly privileged system D-Bus administrative functions to the open network. | Change the force-bind address to listen on localhost `127.0.0.1` by default, or implement robust gRPC TLS authentication and metadata authorization checks before forwarding calls to D-Bus. |
| **04** | `schema_engine.rs:926` | **Medium** | Hardcoded default NextDNS tracking profile ID. | Remove the hardcoded fallback token `"689ec7"`. Require the environment variable to be explicitly configured, or fail gracefully if it is missing. |
| **05** | `proto_gen.rs:1` | **Medium** | Dynamic, string-based protobuf schema generation at runtime instead of compiled schema-as-code. | Deprecate runtime schema generation. Move all plugin schemas to versioned `.proto` files compiled at build-time using `prost-build` / `tonic-build` to guarantee contract safety. |

---
## ⚠ Citation Warnings
- `crates/op-grpc-bridge/src/schema_engine.rs:926`: file has 569 lines
- `crates/op-grpc-bridge/src/schema_engine.rs:917`: file has 569 lines
