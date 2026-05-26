# Production Security and Quality Audit Report

This report presents the findings of a production security and quality audit conducted on the `op-plugins` crate.

---

## 1. Security & Unsafe Code Audit

### 1.1 Unsafe Blocks Analysis
The codebase contains **6** `unsafe` blocks. **5 out of 6** of these blocks lack a mandatory `// SAFETY:` comment explaining why the invocation is safe. 

Below is the complete list of all `unsafe` blocks with their file, line, 1-line context, and safety comment audit:

1. **`crates/op-plugins/src/state_plugins/config.rs:47`**
   ```rust
   let parsed: ConfigStoreState = unsafe { simd_json::from_str(&mut content) }.context("invalid config store")?;
   ```
   * **Missing `// SAFETY:` comment.** Deserialization is marked unsafe because `simd_json` mutates the input buffer in-place and expects it to be valid UTF-8. While `content` is a valid `String`, the safety invariant is unrecorded.

2. **`crates/op-plugins/src/state_plugins/mcp.rs:173`**
   ```rust
   unsafe { simd_json::from_str(&mut c_mut) }.context("Failed to parse MCP config")
   ```
   * **Missing `// SAFETY:` comment.** Raw in-place JSON parsing of mutable string content is performed without safety documentation.

3. **`crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:160`**
   ```rust
   let v: std::result::Result<Value, _> = unsafe { simd_json::from_str(&mut buf) };
   ```
   * **Safety Comment Present:** Line 158 contains: `// SAFETY: simd_json requires mutable access for in-place parsing`. (Compliant).

4. **`crates/op-plugins/src/state_plugins/privacy_routes.rs:47`**
   ```rust
   let mut state: PrivacyRoutesState = unsafe { simd_json::from_str(&mut content) }
   ```
   * **Missing `// SAFETY:` comment.** Mutating in-place string parser invoked without safety annotation.

5. **`crates/op-plugins/src/state_plugins/net.rs:243`**
   ```rust
   let mut bridge_info: HashMap<String, Value> = match unsafe { let mut bridge_info_json_mut = bridge_info_json; simd_json::from_str::<HashMap<String, Value>>(&mut bridge_info_json_mut) } {
   ```
   * **Missing `// SAFETY:` comment.** Raw in-place `simd_json` parsing of a local mutable variable is performed without safety documentation.

6. **`crates/op-plugins/src/state_plugins/openflow.rs:274`**
   ```rust
   let mut bridge_info: HashMap<String, Value> = match unsafe { let mut bridge_info_json_mut = bridge_info_json; simd_json::from_str::<HashMap<String, Value>>(&mut bridge_info_json_mut) } {
   ```
   * **Missing `// SAFETY:` comment.** In-place deserialization is performed without safety documentation.

---

### 1.2 Command Execution Audit

An audit of process spawning identified **37** instances of `Command::new(...)` or `tokio::process::Command::new(...)` across the codebase. 

Many commands do not validate arguments or pass user-provided strings (from desired state definitions) directly into binary invocations, opening up a risk of **flag/argument injection**.

Furthermore, **6 forbidden command violations** were discovered.

---

### 1.3 Forbidden Commands (High & Critical Severity)

The codebase contains several forbidden commands, including shell wrappers that bypass validation, forbidden network tools, and prohibited OpenFlow/OVS orchestration utilities.

#### Critical Vulnerability: Shell Command Injection
* **Location:** `crates/op-plugins/src/state_plugins/pcidecl.rs:92`
* **Vulnerable Code:**
  ```rust
  if let Ok(out) = Command::new("sh")
      .arg("-c")
      .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
      .output()
  ```
* **Severity:** **Critical**
* **Description:** This is a directly exploitable remote shell command injection vulnerability. The `addr` parameter is formatted directly into a shell command executed via `sh -c`. `addr` originates from `item.address` within `PciItem` (Line 42), which is parsed directly from `DesiredState` in the plugin's `apply_state` / `calculate_diff` lifecycle hooks. A malicious desired state target configuration (e.g., setting the address value to `"; rm -rf / ;"`) will execute arbitrary commands with the system privileges of the control plane (often `root`).
* **Remediation:** Remove the shell invocation entirely. Execute `/usr/bin/lspci` directly using safe argument vectors:
  ```rust
  Command::new("lspci").args(["-s", addr]).output()
  ```

#### Forbidden Shell Invocation
* **Location:** `crates/op-plugins/src/state_plugins/dnsresolver.rs:129`
* **Forbidden Code:**
  ```rust
  let mv_ok = Command::new("sh")
      .arg("-c")
      .arg(&mv_cmd)
      .status()
  ```
* **Severity:** **High**
* **Description:** Invoking the shell (`sh`) to run a file move (`mv`) is forbidden. It bypasses standard process argument separation.
* **Remediation:** Replace this with `std::fs::rename(tmp_path, "/etc/resolv.conf")` which is already implemented as a fallback on line 135.

#### Forbidden Network Tool
* **Location:** `crates/op-plugins/src/state_plugins/netmaker.rs:122`
* **Forbidden Code:**
  ```rust
  let output = Command::new("curl")
      .args(["-s", "--max-time", "5", "https://api.ipify.org"])
      .output()
      .await;
  ```
* **Severity:** **High**
* **Description:** `curl` is a forbidden network tool due to the risk of data exfiltration and external command execution. 
* **Remediation:** Use `reqwest`, which is already declared in `Cargo.toml`, to retrieve the public IP address asynchronously.

#### Forbidden OVS Orchestration Commands
* **Location:** `crates/op-plugins/src/state_plugins/full_system.rs:333`
* **Forbidden Code:**
  ```rust
  if let Ok(output) = Command::new("ovs-vsctl").arg("list-br").output().await
  ```
* **Severity:** **High**
* **Description:** Spawning `ovs-vsctl` directly to list bridges is forbidden. 
* **Remediation:** Use the native OVSDB JSON-RPC client implemented in `op_network::ovsdb::OvsdbClient` (as done in other modules) to introspect bridges.

* **Location:** `crates/op-plugins/src/state_plugins/full_system.rs:338`
* **Forbidden Code:**
  ```rust
  let ports_output = Command::new("ovs-vsctl")
      .args(["list-ports", bridge])
      .output()
      .await;
  ```
* **Severity:** **High**
* **Description:** `ovs-vsctl` is spawned directly to fetch ports. 
* **Remediation:** Query bridge ports through the native `OvsdbClient` instead of the command-line wrapper.

#### Forbidden OpenFlow Tool
* **Location:** `crates/op-plugins/src/state_plugins/openflow.rs:432`
* **Forbidden Code:**
  ```rust
  let output = tokio::process::Command::new("ovs-ofctl")
      .args(args)
      .output()
      .await
  ```
* **Severity:** **High**
* **Description:** The OpenFlow table plugin directly executes `ovs-ofctl` commands to insert, query, and delete flows (Lines 498, 504, 529).
* **Remediation:** Transition flow management to a native OpenFlow driver or controller interface.

---

### 1.4 Command Execution Logic Bug
* **Location:** `crates/op-plugins/src/state_plugins/netmaker.rs:309`
* **Vulnerable Code:**
  ```rust
  let install_result = Command::new("apt")
      .args(["update", "&&", "apt", "install", "-y", "netclient"])
      .status()
      .await;
  ```
* **Severity:** **Medium**
* **Description:** Passing `"&&"` as an argument to `Command::new` does not execute shell command chaining. It passes literal `"&&"` and `"apt"` as arguments to the `apt` binary, causing the update command to fail with syntax errors.
* **Remediation:** Invoke `apt update` and `apt install` as two separate process executions.

---

## 2. Schema-as-Code Violations

The codebase mandates a strict schema-as-code discipline using Protocol Buffers and OSCAL. Standard data contracts must not be defined as ad-hoc, manual Rust structs or stringified maps. 

The following architectural violations were identified:

1. **`crates/op-plugins/src/chat.rs` (Lines 6–90):**
   The entire chat and LLM interaction interface represents data contracts as ad-hoc Serde structs (`ChatMessage`, `ToolCall`, `ChatRequest`, `ChatResponse`, `TokenUsage`) rather than versioned Protobuf schemas.

2. **`crates/op-plugins/src/auto_create.rs` (Line 23):**
   Systemd auto-discovery yields an ad-hoc JSON structure using `simd_json::json!` rather than using a versioned schema catalog model.
   ```rust
   json!({
       "type": "systemd",
       "name": unit,
       "state": "active",
       "enabled": true
   })
   ```

3. **`crates/op-plugins/src/state_plugins/systemd_networkd.rs` (Lines 7–20):**
   Ad-hoc Rust structs (`SystemdNetworkdConfig`, `NetworkConfig`) are used to represent systemd-networkd L3 configurations rather than importing schema-driven configuration documents.

4. **`crates/op-plugins/src/state_plugins/unix_socket.rs` (Lines 7–18):**
   Custom ad-hoc structs `SocketEndpoint` and `UnixSocketState` define the contract for Unix domain socket mappings.

---

## 3. Hardcoded Credentials & Network Constants

1. **`crates/op-plugins/src/state_plugins/openflow.rs:163`**
   ```rust
   let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 6633));
   ```
   * **Finding:** Hardcoded local OpenFlow switch loopback port.

2. **`crates/op-plugins/src/state_plugins/privacy_router.rs:30-31`**
   ```rust
   const DEFAULT_OPENFLOW_CONTROLLER: &str = "10.200.0.1:6653";
   const DEFAULT_MGMT_CIDR: &str = "10.200.0.1/24";
   ```
   * **Finding:** Hardcoded management and controller network addresses.

3. **`crates/op-plugins/src/state_plugins/privacy_router.rs:115` & `124`**
   ```rust
   vps_address: "vps.example.com".to_string(),
   ```
   * **Finding:** Hardcoded default external VPS domain.

4. **`crates/op-plugins/src/state_plugins/privacy_router.rs:144`**
   ```rust
   actions.push(FlowAction::ArpResponder {
       mac: "00:11:22:33:44:55".to_string(),
       ip: "10.200.0.1".to_string(),
   });
   ```
   * **Finding:** Hardcoded responder MAC and IP used directly inside active OpenFlow policies.

---

## 4. D-Bus Method Exposure Analysis

The plugin catalog exposes system functions over the system bus.

* **Location:** `crates/op-plugins/src/registry.rs:159`
  ```rust
  let host = op_state::dbus_server::PluginDbusHost {
      plugin: plugin.clone(),
      schema_registry: self.schema_catalog.clone(),
  };
  connection
      .object_server()
      .at(dbus_path.as_str(), host)
      .await
  ```

### Exposure Details
Methods implemented under `PluginDbusHost` are exported dynamically to paths like `/org/opdbus/v1/plugins/{plugin_name}` on the system D-Bus (`org.opdbus.v1`). 

Because they are bound to the system bus, any peer (including unprivileged processes running on the host) can call these methods. Unless D-Bus policies (such as `/etc/dbus-1/system.d/org.opdbus.v1.conf`) restrict method invocation to specific UIDs, or method-level authorization (such as checking calling peer credentials or integrating with `polkit`) is enforced, this enables local privilege escalation. This is particularly dangerous for plugins with write capabilities (e.g., `IncusPlugin`, `NetStatePlugin`, `OpenFlowPlugin`), which allow container manipulation and network re-routing.