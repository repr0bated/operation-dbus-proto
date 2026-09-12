### Schema-as-Code (Protobuf & Data Contract Audit)

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `ChatMessage` | Struct | `crates/op-plugins/src/chat.rs:26` | No | Chat models and LLM/Tool invocations are defined as ad-hoc Rust structs with manual Serde mapping rather than versioned Protobuf contracts. |
| `McpConfig` | Struct | `crates/op-plugins/src/state_plugins/mcp.rs:24` | No | Tool and Agent structures are serialized as untyped JSON and arbitrary HashMaps. |
| `SocketEndpoint` | Struct | `crates/op-plugins/src/state_plugins/unix_socket.rs:11` | No | Sockets are declared using native structures with no compiled schema definition. |
| `DesiredState` | Struct | `crates/op-plugins/src/state.rs:11` | No | Core state targets are managed as raw, untyped JSON (`simd_json::OwnedValue`). |
| `StateChange` | Struct | `crates/op-plugins/src/state.rs:101` | No | State transitions are logged using ad-hoc structures, missing strict audit trail schemas. |
| `ServiceDef` | Struct | `crates/op-plugins/src/service_def.rs:194` | No | Hardcoded system service configurations modeled with manual parsing logic instead of versioned contracts. |

---

### OSCAL Compliance & Control Coverage Mapping

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **System & Comm Protection (SC-7)** | `crates/op-plugins/src/state_plugins/privacy_router.rs:234` | None | Hardcoded default privacy and ARP routing rules bypass the compliance catalog. |
| **System & Comm Protection (SC-7)** | `crates/op-plugins/src/state_plugins/openflow_obfuscation.rs:79` | None | Traffic obfuscation flows (jitter, TTL normalization) are defined directly in code rather than mapped to OSCAL control implementations. |
| **Access Control (AC-17 / SC-8)** | `crates/op-plugins/src/state_plugins/web_ui.rs:117` | None | UI access and CSRF controls are managed via ad-hoc parameters without machine-readable OSCAL SSP mapping. |
| **Configuration Management (CM-6)** | `crates/op-plugins/src/state_plugins/mcp.rs:434` | None | Dynamic Model Context Protocol execution logic lacks structural mapping in OSCAL Component Definitions. |
| **System & Comm Protection (SC-7)** | `crates/op-plugins/src/state_plugins/net.rs:518` | None | OpenVSwitch bridge and port configurations are managed dynamically without OSCAL compliance coverage. |

---

### Recommendations & Remediations for Major & Critical Gaps

#### 1. CRITICAL: Remote Command Injection via Shell Interpolation in `PciDeclPlugin`
* **Location:** `crates/op-plugins/src/state_plugins/pcidecl.rs:89`
* **Impact:** Direct command execution as the root/control plane user. The `lspci_present` helper executes a shell command constructed via unsanitized string formatting: `format!("lspci -s {} >/dev/null 2>&1; echo $?", addr)`. Because `addr` is derived from the `desired` configuration input during state diff calculation, any actor capable of modifying the target configuration can inject shell metacharacters (e.g. `"; rm -rf /; "`) to execute arbitrary commands.
* **Remediation:** Remove the shell execution abstraction. Invoke `Command::new("lspci")` directly and pass arguments safely:
  ```rust
  let output = Command::new("lspci")
      .args(["-s", addr])
      .output()
      .await?;
  ```
  Additionally, strictly validate that `addr` matches a predictable PCI address format (e.g., `^[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-9a-fA-F]$`) prior to execution.

#### 2. CRITICAL: Arbitrary File Write via Path Traversal in `SystemdNetworkdManager`
* **Location:** `crates/op-plugins/src/state_plugins/systemd_networkd.rs:36`
* **Impact:** Control plane compromise. The `generate_network_files` method constructs destination paths using unchecked configuration names: `network_dir.join(format!("50-{}.network", name))`. If a configuration contains directory traversal sequences like `../../etc/cron.d/malicious`, the manager will write arbitrary file contents outside of `/etc/systemd/network`, allowing persistent privilege escalation.
* **Remediation:** Extract the base filename using `Path::file_name` or enforce strict alphanumeric constraints on the `name` parameter:
  ```rust
  let safe_name = Path::new(name)
      .file_name()
      .ok_or_else(|| anyhow::anyhow!("Invalid name"))?;
  let file_path = network_dir.join(format!("50-{}.network", safe_name.to_string_lossy()));
  ```

#### 3. MAJOR: Broken Command Invocation in `NetmakerPlugin`
* **Location:** `crates/op-plugins/src/state_plugins/netmaker.rs:353`
* **Impact:** State synchronization failure. The plugin attempts to invoke sequential commands through a non-shell execution context: `Command::new("apt").args(["update", "&&", "apt", "install", "-y", "netclient"])`. Because `Command::new` does not execute in a shell, `&&` is treated as a literal argument passed to `apt`, rendering the execution invalid and preventing automatic installation.
* **Remediation:** Execute separate commands sequentially, or utilize native package manager APIs/D-Bus interfaces (such as PackageKit) rather than raw shell utility chaining:
  ```rust
  let update_status = Command::new("apt-get").arg("update").status().await?;
  if update_status.success() {
      Command::new("apt-get").args(["install", "-y", "netclient"]).status().await?;
  }
  ```

#### 4. MAJOR: Cryptographically Broken Hashing (MD5) for Audit Trail Verification
* **Location:** `crates/op-plugins/src/auto_create.rs:114` (and duplicated across multiple plugins, e.g., `net.rs:677`, `dinit.rs:231`)
* **Impact:** Risk of state manipulation and hash collision attacks in the snowball footprint audit trail. The codebase relies on MD5 to compute current and desired hashes for state verification.
* **Remediation:** Replace all instances of `md5::compute` with a cryptographically secure hashing algorithm such as SHA-256 (`sha2::Sha256`), which is already imported in the cargo configuration:
  ```rust
  use sha2::{Digest, Sha256};
  let mut hasher = Sha256::new();
  hasher.update(simd_json::to_string(current)?);
  let hash = format!("{:x}", hasher.finalize());
  ```

#### 5. SCHEMA-AS-CODE VIOLATION: Ad-hoc Serialization of State Contracts
* **Location:** `crates/op-plugins/src/chat.rs:26`, `crates/op-plugins/src/state_plugins/mcp.rs:24`, `crates/op-plugins/src/state_plugins/unix_socket.rs:11`
* **Impact:** Structural drift and lack of serialization interoperability across components.
* **Remediation:** Define the messaging and configuration contracts as Protocol Buffers (`.proto` files) within a versioned schema registry, compiling them to native Rust structs using `prost` or `tonic` during the build step.

#### 6. OSCAL COMPLIANCE GAP: Hardcoded Policy and Flow Rules
* **Location:** `crates/op-plugins/src/state_plugins/privacy_router.rs:234`, `crates/op-plugins/src/state_plugins/openflow_obfuscation.rs:79`
* **Impact:** Violation of CM-6 and SC-7 controls under NIST SP 800-53 / FedRAMP. Deep security parameters (obfuscation levels, packet matching, routing rules) are compiled directly into the application binary, hindering configuration verification and dynamic authorization.
* **Remediation:** Externalize all routing and traffic-shaping parameters into machine-readable JSON policies. Register and map these files in an OSCAL Component Definition to satisfy traceability requirements for SC-7 (Boundary Protection) and CM-6 (Configuration Settings).