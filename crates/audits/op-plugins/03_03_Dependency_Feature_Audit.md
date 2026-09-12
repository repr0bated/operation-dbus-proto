# Production Security & Quality Audit: op-plugins

## Dependencies & Feature Inventory

### Direct Dependencies (from `crates/op-plugins/Cargo.toml`)

| Dependency Crate | Version / Source | Enabled Features (Explicit vs. Implicit) | Risk Flag / CVE Adjacent |
| :--- | :--- | :--- | :--- |
| `op-core` | `{ path = "../op-core" }` | None | None |
| `op-dbus-model` | `workspace = true` | Inherited from workspace | None |
| `op-state` | `{ path = "../op-state" }` | None | None |
| `op-state-store` | `{ path = "../op-state-store" }` | None | None |
| `op-snowball` | `{ path = "../op-snowball" }` | None | None |
| `op-network` | `{ path = "../op-network" }` | None | None |
| `op-dynamic-loader` | `{ path = "../op-dynamic-loader" }` | None | None |
| `op-execution-tracker` | `{ path = "../op-execution-tracker" }` | None | None |
| `tokio` | `workspace = true` | `["full"]` (via workspace) | None |
| `serde` | `workspace = true` | `["derive"]` (via workspace) | None |
| `simd-json` | `workspace = true` | `["serde", "serde_impl"]` (via workspace) | None |
| `anyhow` | `workspace = true` | None | None |
| `thiserror` | `workspace = true` | None | None |
| `tracing` | `workspace = true` | None | None |
| `async-trait` | `workspace = true` | None | None |
| `zbus` | `workspace = true` | `["tokio"]` (via workspace) | None |
| `chrono` | `workspace = true` | `["serde"]` (via workspace) | None |
| `log` | `workspace = true` | None | None |
| `reqwest` | `workspace = true` | `["json", "stream"]` (via workspace) | None |
| `sha2` | `workspace = true` | None | None |
| `md5` | `workspace = true` | `v0.7` (Inherited from workspace) | **Weak Hash / Collision Risk** |
| `uuid` | `workspace = true` | `["v4", "serde"]` (via workspace) | None |
| `dirs` | `"5.0"` | Default features | None |
| `parking_lot` | `workspace = true` | None | None |

### Crate Features
* **Crate-specific features**: None defined in `crates/op-plugins/Cargo.toml`.
* **Workspace features**: `default = ["grpc"]`, `grpc = []` in root `Cargo.toml`.

---

## Schema-As-Code Compliance Audit

The `op-plugins` crate represents a major "schema-as-code" gap because several critical data contracts are defined as ad-hoc, localized Rust structs decorated with Serde, rather than versioned, centralized schemas.

### Schema-as-Code Violations
1. **Ad-hoc Chat/LLM Schema Contracts**:
   * **Location**: `crates/op-plugins/src/chat.rs:6-90`
   * **Gaps**: `ChatMessage`, `ToolCall`, `ChatRequest`, `ChatResponse`, and `TokenUsage` are ad-hoc serialization structs. They should be generated from a unified Protocol Buffer specification using `prost` or structured as versioned OpenAPI JSON schemas using `schemars` to maintain strict compatibility across agents, LLMs, and client frontends.
2. **Ad-hoc Service Definition Contracts**:
   * **Location**: `crates/op-plugins/src/service_def.rs:12-282`
   * **Gaps**: The `ServiceDef`, `ExecCommand`, `ResourceLimits`, and `RestartPolicy` types define the process configuration format. Expressing these as raw Serde-serialized Rust structs forces clients to duplicate the parsing logic and prevents foreign-language services (e.g. Python or Go agents) from safely producing or validating dinit/systemd definitions.
3. **Ad-hoc Incident/State Change Contracts**:
   * **Location**: `crates/op-plugins/src/state.rs:10-230`
   * **Gaps**: `DesiredState`, `StateChange`, and `ValidationResult` are local serialization definitions. They contain raw `simd_json::OwnedValue` payloads. This lacks validation constraints at the interface boundaries, allowing unvalidated JSON configurations to bypass type-safety rules.

---

## Storage Backend Inventory

| Backend / Store | Location | Role / Architecture | Verification Status |
| :--- | :--- | :--- | :--- |
| `SqlitePluginCatalog` | `crates/op-plugins/src/registry.rs:35` | Stores serialized runtime plugin `CatalogDocument` records | Validated (used for disk persistence of catalog snapshots) |
| `StateStore` (Abstraction) | `crates/op-plugins/src/default_registry.rs:69` | Authoritative state store interface (implemented as `SqliteStore` or `SqlitePluginCatalog` at runtime) | Validated (wires config plugins to transactional storage) |
| Ad-hoc File System Storage | `crates/op-plugins/src/state_plugins/config.rs:15` | Read/write persistence for global configurations to `/etc/op-dbus/config-store.json` | Architectural Violation (bypasses structured DB backends like Sled/Cozo) |
| Ad-hoc File System Storage | `crates/op-plugins/src/state_plugins/privacy_routes.rs:10` | Read/write persistence for privacy route mapping to `/var/lib/op-dbus/privacy-routes.json` | Architectural Violation (bypasses structured DB backends like Sled/Cozo) |

---

## Security & Quality Audit Findings

### Critical Vulnerabilities (Directly Exploitable)

#### 1. Command Injection in `pcidecl.rs` via Shell Interpolation
* **File & Line**: `crates/op-plugins/src/state_plugins/pcidecl.rs:109`
* **Vulnerability Class**: Command Injection (CWE-78)
* **Description**:
  The `lspci_present` method formats a system command string using the unsanitized `addr` field directly from the `PciItem` structure and executes it inside a shell environment (`sh -c`).
  ```rust
  if let Ok(out) = Command::new("sh")
      .arg("-c")
      .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
      .output()
  ```
  The `addr` parameter is deserialized directly from the `desired` JSON state during diff calculation (`pcidecl.rs:136`).
* **Exploit Scenario**:
  An attacker who can submit or alter the desired state configuration (either via a local D-Bus call or via the state publisher API) can write a payload such as:
  ```json
  {
    "version": 1,
    "items": [
      {
        "id": "exploit",
        "mode": "enforce",
        "address": "0000:00:1f.6; id > /tmp/rce.txt",
        "driver_override": null
      }
    ]
  }
  ```
  When the agent reconciles this state and calls `calculate_diff`, `lspci_present` will format and execute the command:
  ```bash
  sh -c "lspci -s 0000:00:1f.6; id > /tmp/rce.txt >/dev/null 2>&1; echo $?"
  ```
  The injected payload (`id > /tmp/rce.txt`) is executed with the privileges of the control plane agent (typically `root`).
* **Remediation**:
  Avoid using a shell. Spawn `/usr/bin/lspci` directly using safe argument passing, and check the process status instead of echoing `$?`.
  ```rust
  let status = Command::new("lspci")
      .args(["-s", addr])
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status();
  ```

#### 2. Argument Injection leading to RCE in `packagekit.rs`
* **File & Line**: `crates/op-plugins/src/state_plugins/packagekit.rs:136`, `152`, and `172`
* **Vulnerability Class**: Argument Injection (CWE-88)
* **Description**:
  The `install_via_direct`, `remove_via_direct`, and `package_installed` methods accept a `package_name` string from the desired state and pass it as an argument to process builders (`apt-get`, `pacman`, `dpkg`) without sanitizing flags or specifying a `--` separator.
  ```rust
  if Command::new("apt-get")
      .args(["install", "-y", package_name])
      .status()?
      .success()
  ```
* **Exploit Scenario**:
  An attacker can supply a package name that begins with option characters (e.g. `-`). If the target system is Debian-based, the attacker can set the package name to:
  ```text
  -oDPkg::Post-Invoke::="touch /tmp/rce_argument"
  ```
  This single argument is passed directly to the `apt-get` command:
  ```bash
  apt-get install -y "-oDPkg::Post-Invoke::=\"touch /tmp/rce_argument\""
  ```
  APT parses this as a configuration override option. When the installation completes or fails, the registered `Post-Invoke` command is executed as `root`.
* **Remediation**:
  Insert the double-dash `--` argument separator to signify the end of command options before appending user-controlled package names:
  ```rust
  Command::new("apt-get")
      .args(["install", "-y", "--"])
      .arg(package_name)
  ```

---

### High Security & Cryptographic Weaknesses

#### 3. Cryptographically Broken Hashing (MD5) for State Footprints
* **File & Line**: `crates/op-plugins/src/auto_create.rs:92`, `crates/op-plugins/src/state_plugins/config.rs:141`, `crates/op-plugins/src/state_plugins/dnsresolver.rs:252`, `crates/op-plugins/src/state_plugins/incus.rs:499`, `crates/op-plugins/src/state_plugins/keyring.rs:197`, `crates/op-plugins/src/state_plugins/login1.rs:92`, `crates/op-plugins/src/state_plugins/lxc.rs:630`, `crates/op-plugins/src/state_plugins/mcp.rs:411`, `crates/op-plugins/src/state_plugins/netmaker.rs:281`, `crates/op-plugins/src/state_plugins/privacy.rs:88`, `crates/op-plugins/src/state_plugins/privacy_routes.rs:130`, `crates/op-plugins/src/state_plugins/rtnetlink.rs:159`, `crates/op-plugins/src/state_plugins/systemd.rs:327`, and `crates/op-plugins/src/state_plugins/dinit.rs:218`
* **Vulnerability Class**: Use of a Broken Cryptographic Algorithm (CWE-327)
* **Description**:
  The majority of the plugin implementations use the MD5 hashing algorithm to calculate `current_hash` and `desired_hash` metadata inside `calculate_diff`.
  ```rust
  current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?))
  ```
  If these hashes are stored in the block registry or a snowball footprint to ensure integrity and prevent tampering, MD5's known susceptibility to collision attacks allows an attacker to swap a valid configuration with a malicious one that yields the exact same MD5 digest, making the alteration undetectable by hash-based auditing mechanisms.
* **Remediation**:
  Standardize on `sha2::Sha256` for all diff hashing operations, mirroring the correct pattern used in `crates/op-plugins/src/state.rs:136`.

#### 4. Path Traversal in BTRFS Golden Image Snapshots
* **File & Line**: `crates/op-plugins/src/state_plugins/privacy_router.rs:411`
* **Vulnerability Class**: Path Traversal (CWE-22)
* **Description**:
  The `LxcPlugin::create_container_from_btrfs_snapshot` method formats the target subvolume path using the `golden_image_name` string from the configuration properties.
  ```rust
  let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
  ```
  Because the `golden_image_name` parameter is not checked for directory traversal sequences (e.g. `../`), an attacker who can configure desired properties for a container can point the path to an arbitrary subvolume on the disk.
* **Exploit Scenario**:
  By specifying `golden_image_name` as `../../images/101/rootfs`, the computed `golden_image_path` resolves to:
  ```text
  /var/lib/pve/local-btrfs/images/101/rootfs
  ```
  The plugin will subsequently run `btrfs subvolume snapshot` to clone container 101's private files into the attacker's container, enabling cross-container data leakage and security bypasses.
* **Remediation**:
  Ensure that `golden_image_name` does not contain `/` or `.` sequences, and sanitize it using `Path::file_name` comparison.

#### 5. TOCTOU/Race Condition on Fixed Temporary Path
* **File & Line**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:114`
* **Vulnerability Class**: Time-of-Check to Time-of-Use Race Condition / Symlink Attack (CWE-367)
* **Description**:
  The DNS resolver plugin writes new DNS configurations to a hardcoded temporary file path `/etc/resolv.conf.sysdecl.tmp` before renaming it to `/etc/resolv.conf`.
  ```rust
  let tmp_path = "/etc/resolv.conf.sysdecl.tmp";
  fs::write(tmp_path, buf.as_bytes()).context("write temp resolv.conf")?;
  ```
  If a local user is able to write to `/etc/` or create a symbolic link at `/etc/resolv.conf.sysdecl.tmp` pointing to a critical system file (e.g., `/etc/shadow`), the control plane agent will overwrite the target file when writing to the temporary path.
* **Remediation**:
  Generate random, secure temporary files in the same directory using a secure utility (such as the `tempfile` library) instead of writing to a hardcoded path.

---

### Quality & Architectural Defects

#### 6. Blocking Sync Calls in Async Tokio Contexts
* **File & Line**: 
  * `crates/op-plugins/src/dynamic_loading.rs:160` & `181` (`std::process::Command::output()`)
  * `crates/op-plugins/src/state_plugins/packagekit.rs:136`, `152`, & `172` (`std::process::Command::status()`)
  * `crates/op-plugins/src/state_plugins/pcidecl.rs:109` (`std::process::Command::output()`)
* **Defect Class**: Thread Pool Starvation (Architectural Deviation)
* **Description**:
  These plugins run heavy, synchronous, and blocking process executions (e.g. system package installations, hardware queries) directly inside async tokio futures. This blocks tokio's worker threads, potentially starving other tasks and causing timeouts or communication failures on the D-Bus and gRPC interfaces.
* **Remediation**:
  Replace `std::process::Command` imports with `tokio::process::Command`, and call `.await` on their operations.

#### 7. Shell Operator Syntax Error in `netmaker.rs`
* **File & Line**: `crates/op-plugins/src/state_plugins/netmaker.rs:293`
* **Defect Class**: Improper Command Invocation / Logic Error
* **Description**:
  The netmaker plugin attempts to run an updated command sequence containing a shell operator (`&&`) within an argument array:
  ```rust
  let install_result = Command::new("apt")
      .args(["update", "&&", "apt", "install", "-y", "netclient"])
      .status()
      .await;
  ```
  Because `tokio::process::Command` passes the arguments vector directly to the `execve` system call (rather than running it inside a shell), the `apt` process is started with literal `"&&"` and `"apt"` as package arguments. `apt` will fail, meaning netclient is never successfully installed.
* **Remediation**:
  Run `apt-get update` as a separate, prior `Command` invocation before calling `apt-get install`, or use a shell if chaining is strictly required.

#### 8. Use of Unnecessary Shell Wrappers
* **File & Line**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:116`
* **Defect Class**: Anti-Pattern / Unnecessary Shell Overhead
* **Description**:
  The resolver plugin spawns `sh` to run the `mv` utility:
  ```rust
  let mv_cmd = format!("mv -f {} /etc/resolv.conf", tmp_path);
  let mv_ok = Command::new("sh")
      .arg("-c")
      .arg(&mv_cmd)
  ```
  Spawning shell processes increases runtime overhead and exposes the application to argument parsing risks.
* **Remediation**:
  Perform the rename using safe Rust APIs like `std::fs::rename` or `tokio::fs::rename`.