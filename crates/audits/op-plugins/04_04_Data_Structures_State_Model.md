# `op-plugins` Production Security & Quality Audit

---

## 1. Data Structures Analysis

### Concurrent & Synchronization Type Counts per File

| File Path | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` | `.clone()` Count | Large Structs (> 5 public fields) | Globally Mutable State |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- | :--- |
| `crates/op-plugins/src/auto_create.rs` | 1 | 0 | 0 | 1 | 0 | 0 | 10 | None | None |
| `crates/op-plugins/src/builtin.rs` | 2 | 0 | 0 | 2 | 0 | 0 | 4 | None | None |
| `crates/op-plugins/src/chat.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | `ChatMessage` (6) | None |
| `crates/op-plugins/src/dynamic_loading.rs` | 4 | 0 | 0 | 4 | 0 | 0 | 9 | None | None |
| `crates/op-plugins/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/plugin.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | `PluginCapabilities` (8) | None |
| `crates/op-plugins/src/registry.rs` | 4 | 0 | 0 | 4 | 0 | 0 | 9 | None | None |
| `crates/op-plugins/src/service_def.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | `ServiceDef` (18) | None |
| `crates/op-plugins/src/state.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 | `StateChange` (7) | None |
| `crates/op-plugins/src/state_publisher.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/default_registry.rs` | 3 | 0 | 0 | 0 | 0 | 0 | 3 | None | None |
| `crates/op-plugins/src/state_plugins/adc.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/agent_config.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/config.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 12 | None | None |
| `crates/op-plugins/src/state_plugins/dnsresolver.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 10 | None | None |
| `crates/op-plugins/src/state_plugins/endpoint.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/gcloud_adc.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/hardware.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 | None | None |
| `crates/op-plugins/src/state_plugins/incus.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 18 | `IncusInstance` (8) | None |
| `crates/op-plugins/src/state_plugins/keypair.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/keyring.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 | None | None |
| `crates/op-plugins/src/state_plugins/login1.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 | None | None |
| `crates/op-plugins/src/state_plugins/lxc.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 10 | None | None |
| `crates/op-plugins/src/state_plugins/mcp.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 15 | `ToolDefinition` (7) | None |
| `crates/op-plugins/src/state_plugins/netmaker.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 | None | None |
| `crates/op-plugins/src/state_plugins/ovsdb_bridge.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 | `BridgeConfig` (7), `PortConfig` (6), `InterfaceConfig` (7) | None |
| `crates/op-plugins/src/state_plugins/packagekit.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 5 | None | None |
| `crates/op-plugins/src/state_plugins/pcidecl.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 8 | None | None |
| `crates/op-plugins/src/state_plugins/privacy.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 | `PrivacyConfig` (9) | None |
| `crates/op-plugins/src/state_plugins/privacy_routes.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 11 | `PrivacyRoute` (13) | None |
| `crates/op-plugins/src/state_plugins/proxmox.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/proxy_server.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/rtnetlink.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 10 | `RtnetlinkInterfaceConfig` (6) | None |
| `crates/op-plugins/src/state_plugins/schema_contract.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 | None | None |
| `crates/op-plugins/src/state_plugins/service.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/sessdecl.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/software.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/systemd.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 12 | None | None |
| `crates/op-plugins/src/state_plugins/systemd_networkd.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | `NetworkConfig` (7) | None |
| `crates/op-plugins/src/state_plugins/users.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/web_ui.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 10 | `WebUiTunables` (9), `WebUiCapabilities` (8) | None |
| `crates/op-plugins/src/state_plugins/wireguard.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/dinit.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 10 | None | None |
| `crates/op-plugins/src/state_plugins/full_system.rs` | 1 | 0 | 0 | 1 | 0 | 0 | 10 | `FullSystemState` (11), `InterfaceInfo` (5) | None |
| `crates/op-plugins/src/state_plugins/net.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 10 | None | None |
| `crates/op-plugins/src/state_plugins/openflow_obfuscation.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 4 | None | None |
| `crates/op-plugins/src/state_plugins/privacy_router.rs` | 0 | 0 | 0 | 0 | 0 | 0 | **21** 🚩 | `PrivacyRouterConfig` (8), `WireGuardConfig` (6), `ContainerResources` (7), `XRayConfig` (7) | None |
| `crates/op-plugins/src/state_plugins/procfs.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/openflow.rs` | 1 | 0 | 0 | 0 | 0 | 0 | **24** 🚩 | `OpenFlowConfig` (6), `FlowEntry` (7) | None |
| `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 5 | None | None |
| `crates/op-plugins/src/state_plugins/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-plugins/src/state_plugins/unix_socket.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |

### Data Structure Finding Flags

1. **High `.clone()` Count (> 20) in Single Files**:
   * `crates/op-plugins/src/state_plugins/privacy_router.rs` contains **21** clone operations. 
   * `crates/op-plugins/src/state_plugins/openflow.rs` contains **24** clone operations.
2. **Large Structs (> 5 public fields)**: 
   * `ChatMessage` (`chat.rs`) — 6 public fields.
   * `PluginCapabilities` (`plugin.rs`) — 8 public fields.
   * `ServiceDef` (`service_def.rs`) — 18 public fields.
   * `StateChange` (`state.rs`) — 7 public fields.
   * `IncusInstance` (`incus.rs`) — 8 public fields.
   * `ToolDefinition` (`mcp.rs`) — 7 public fields.
   * `BridgeConfig` (`ovsdb_bridge.rs`) — 7 public fields.
   * `PortConfig` (`ovsdb_bridge.rs`) — 6 public fields.
   * `InterfaceConfig` (`ovsdb_bridge.rs`) — 7 public fields.
   * `PrivacyConfig` (`privacy.rs`) — 9 public fields.
   * `PrivacyRoute` (`privacy_routes.rs`) — 13 public fields.
   * `RtnetlinkInterfaceConfig` (`rtnetlink.rs`) — 6 public fields.
   * `NetworkConfig` (`systemd_networkd.rs`) — 7 public fields.
   * `WebUiTunables` (`web_ui.rs`) — 9 public fields.
   * `WebUiCapabilities` (`web_ui.rs`) — 8 public fields.
   * `FullSystemState` (`full_system.rs`) — 11 public fields.
   * `PrivacyRouterConfig` (`privacy_router.rs`) — 8 public fields.
   * `WireGuardConfig` (`privacy_router.rs`) — 6 public fields.
   * `ContainerResources` (`privacy_router.rs`) — 7 public fields.
   * `XRayConfig` (`privacy_router.rs`) — 7 public fields.
   * `OpenFlowConfig` (`openflow.rs`) — 6 public fields.
   * `FlowEntry` (`openflow.rs`) — 7 public fields.

---

## 2. Production Security & Quality Audit Findings

### CRITICAL: Command Injection via Unsanitized PCI Address
* **Location**: `crates/op-plugins/src/state_plugins/pcidecl.rs:121`
* **Vulnerability Type**: Remote/Local Command Injection
* **Description**:
  The `lspci_present` helper formats the target PCI address string (`addr`) directly into a shell command context using `sh -c`:
  ```rust
  fn lspci_present(addr: &str) -> bool {
      if let Ok(out) = Command::new("sh")
          .arg("-c")
          .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
          .output()
      {
          return out.stdout.first().map(|b| *b == b'0').unwrap_or(false);
      }
      false
  }
  ```
  The `addr` parameter is derived directly from user-provided desired state configurations on the control plane. If an attacker submits a target state containing shell metacharacters in the address field (for example, `"0000:00:1f.6; rm -rf /"`), the payload will execute under shell interpretation.
* **Exploitability**: Directly exploitable. This allows full remote/local command execution on the target system with the privilege level of the running agent.
* **Remediation**: Avoid spawning `sh -c`. Pass arguments safely using sequential argument arrays in `Command::new("lspci")` with `arg("-s")` and `arg(addr)`. Validate that the `addr` strictly matches standard PCI address regex (e.g. `^[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-9a-fA-F]$`).

---

### CRITICAL: Arbitrary BTRFS Snapshot Overwrite and Path Traversal
* **Location**: `crates/op-plugins/src/state_plugins/lxc.rs:430`
* **Vulnerability Type**: Arbitrary Path Traversal
* **Description**:
  The LXC plugin extracts `golden_image` as an arbitrary string from the unvalidated desired state JSON:
  ```rust
  let golden_image = props
      .and_then(|p| p.get("golden_image"))
      .and_then(|v| v.as_str());
  ```
  Without sanitizing this name for directory traversal sequences (such as `..`), it directly constructs system file paths:
  ```rust
  let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
  ```
  This unvalidated path is then executed directly via system calls to the `btrfs` CLI binary:
  ```rust
  let snapshot_output = tokio::process::Command::new("btrfs")
      .args([
          "subvolume",
          "snapshot",
          &golden_image_path,
          &container_rootfs,
      ])
      .output()
      .await?;
  ```
  An attacker can supply a value such as `../../../../tmp/evil_subvol` to target and execute snapshot operations on arbitrary filesystem locations.
* **Exploitability**: Directly exploitable. Allows arbitrary directory reading, subvolume creation, and access to out-of-bounds filesystem components.
* **Remediation**: Implement strict path sanitization. Restrict `golden_image` to alphanumeric characters or match against a strictly defined whitelist of local template subvolumes. Verify that the resolved canonical path lies within `/var/lib/pve/` templates directory.

---

### HIGH: Memory Unsafety via Uncontrolled Unsafe Parsing of Local Files
* **Locations**:
  * `crates/op-plugins/src/state_plugins/config.rs:44`
  * `crates/op-plugins/src/state_plugins/mcp.rs:163`
  * `crates/op-plugins/src/state_plugins/privacy_routes.rs:52`
  * `crates/op-plugins/src/state_plugins/net.rs:286`
  * `crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:166`
* **Vulnerability Type**: Potential Memory Corruption / Undefined Behavior
* **Description**:
  The codebase extensively uses `simd_json::from_str` wrapped in `unsafe` blocks to deserialize configuration state from disk files (e.g., `config-store.json`, `privacy-routes.json`). 
  ```rust
  let parsed: ConfigStoreState =
      unsafe { simd_json::from_str(&mut content) }.context("invalid config store")?;
  ```
  `simd_json::from_str`'s safety invariants dictate that the input string must be mutable, must possess a specific amount of trailing padding bytes, and must contain valid UTF-8. If a malicious local user with access to write to these databases, or an unexpected runtime file corruption, truncates or alters the files, running this unsafe deserializer can lead to memory safety violations, segmentation faults, and exploit vectors within root-privileged services.
* **Remediation**: Use `simd_json::from_slice` safely or switch to the safe variant `simd_json::serde::from_str` or `serde_json::from_str` for disk-based system files.

---

### HIGH: Cryptographically Broken Hash Usage for Snowball Audit Footprints
* **Locations**:
  * `crates/op-plugins/src/auto_create.rs:103-104`
  * `crates/op-plugins/src/state_plugins/config.rs:114-115`
  * `crates/op-plugins/src/state_plugins/dnsresolver.rs:194`
  * `crates/op-plugins/src/state_plugins/incus.rs:462`
  * `crates/op-plugins/src/state_plugins/keyring.rs:205`
  * `crates/op-plugins/src/state_plugins/login1.rs:71`
  * `crates/op-plugins/src/state_plugins/net.rs:389`
  * `crates/op-plugins/src/state_plugins/dinit.rs:242`
* **Vulnerability Type**: Cryptographic Integrity Defect
* **Description**:
  The audit trail framework generates state footprint hashes using the MD5 algorithm (`md5::compute(...)`). MD5 is cryptographically broken and prone to collision attacks. If these footprints are written to a snowball or ledger for tamper-proofing and immutability tracking, an attacker can modify the state configuration, craft an MD5 collision, and commit a fraudulent state change that matches a previously verified footprint.
* **Remediation**: Upgrade all MD5 usages in auditing/diff logic to `sha2::Sha256` (which is already imported and used correctly in `crates/op-plugins/src/dynamic_loading.rs` and `crates/op-plugins/src/state_plugins/openflow.rs`).

---

### HIGH: Fake Rollback Implementations Advertising Functional Recovery
* **Locations**:
  * `crates/op-plugins/src/state_plugins/proxmox.rs:114`
  * `crates/op-plugins/src/state_plugins/users.rs:120`
  * `crates/op-plugins/src/state_plugins/software.rs:120`
  * `crates/op-plugins/src/state_plugins/keypair.rs:120`
  * `crates/op-plugins/src/state_plugins/agent_config.rs:200`
  * `crates/op-plugins/src/state_plugins/web_ui.rs:514`
* **Vulnerability Type**: Logic & Operational Safety Defect
* **Description**:
  These state plugins return `supports_rollback: true` inside their `capabilities()` declaration, signaling to the orchestrator that they can revert system state safely. However, their actual `rollback` implementation is an empty block:
  ```rust
  async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
      Ok(())
  }
  ```
  This causes rollback operations to silently complete without executing any actual recovery, leaving the target node in a corrupted or unstable state after a failed rollout.
* **Remediation**: Set `supports_rollback: false` in the plugin capabilities unless a real, functional rollback procedure has been implemented.

---

### MEDIUM: Dysfunctional Command Execution in Netmaker Installation
* **Location**: `crates/op-plugins/src/state_plugins/netmaker.rs:247`
* **Vulnerability Type**: Quality Defect / Functional Failure
* **Description**:
  The netmaker plugin attempts to run sequential commands through `Command::new` without a shell interpreter:
  ```rust
  let install_result = Command::new("apt")
      .args(["update", "&&", "apt", "install", "-y", "netclient"])
      .status()
      .await;
  ```
  This passes `&&` and subsequent arguments directly to `apt` as arguments. The command will always fail immediately, making it impossible to install the netclient package as intended.
* **Remediation**: Execute "update" and "install" as separate, sequential `Command` operations, or invoke them explicitly through a shell command such as `Command::new("sh").args(["-c", "apt update && apt install -y netclient"])`.

---

### MEDIUM: Predictable Temporary File Path in DnsResolver
* **Location**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:121`
* **Vulnerability Type**: Insecure Temporary File Handling
* **Description**:
  The DNS resolver plugin writes system configurations to a hardcoded, highly predictable temporary file location:
  ```rust
  let tmp_path = "/etc/resolv.conf.sysdecl.tmp";
  fs::write(tmp_path, buf.as_bytes()).context("write temp resolv.conf")?;
  ```
  While `/etc` is privileged, a predictable, shared filename risks race conditions, write collisions from parallel tasks, or symlink hijacking if permissions are misconfigured.
* **Remediation**: Use a randomized name generated via the `tempfile` crate in the same directory, then rename it atomically.

---

### MEDIUM: Inefficient Subprocess Spawning for File Reading
* **Location**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:114`
* **Vulnerability Type**: Resource Inefficiency
* **Description**:
  The resolver plugin executes a subprocess running `/bin/cat` to retrieve a file's contents, even though a native, efficient Rust library method is available:
  ```rust
  if let Ok(out) = Command::new("cat").arg("/etc/resolv.conf").output() { ... }
  ```
  Spawning shell processes unnecessarily wastes file descriptors, process IDs, and CPU cycles.
* **Remediation**: Replace the `Command` spawn with a direct call to `tokio::fs::read_to_string("/etc/resolv.conf")`.

---

## 3. Schema-as-Code Violations

The following data structures and payloads express data contracts as ad-hoc structs, freeform values, or unstructured strings instead of utilizing versioned OSCAL schemas or Protocol Buffer definitions:

1. **Ad-hoc LLM Payloads**:
   * `ChatMessage`, `ToolCall`, `ChatRequest`, `ChatResponse`, and `TokenUsage` in `crates/op-plugins/src/chat.rs:1-85` represent core interaction models as ad-hoc serde Rust structs with unstructured `HashMap<String, OwnedValue>` metadata. These structures are not bound to any versioned JSON-schema or Protobuf contracts.
2. **Ad-hoc State JSON Macros**:
   * In `crates/op-plugins/src/auto_create.rs:25`, discovered systemd units are projected into freeform JSON via the `json!` macro rather than using structured schema types defined inside `PluginSchema`.
   * In `crates/op-plugins/src/builtin.rs:24`, `EchoPlugin` instantiates state using `simd_json::json!({})`.
3. **Unstructured Desired State Contracts**:
   * `crates/op-plugins/src/state.rs:17` defines desired target state configurations strictly using `simd_json::OwnedValue` instead of enforcing versioned, schema-validated inputs.

---
## ⚠ Citation Warnings
- `crates/op-plugins/src/state_plugins/proxmox.rs:114`: file has 101 lines
