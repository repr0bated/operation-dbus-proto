# Dependencies & Feature Inventory

The following table lists every direct dependency declared in `crates/op-plugins/Cargo.toml`, with its version, explicitly enabled features, default features, and any security/architectural flags.

| Dependency | Version | Explicitly Enabled Features | Default Features Enabled | Flags & Notes |
| :--- | :--- | :--- | :--- | :--- |
| `op-core` | Local Path | N/A | N/A | Internal Workspace Crate |
| `op-dbus-model` | Workspace | N/A | N/A | Internal Workspace Crate |
| `op-state` | Local Path | N/A | N/A | Internal Workspace Crate |
| `op-state-store` | Local Path | N/A | N/A | Internal Workspace Crate |
| `op-blockchain` | Local Path | N/A | N/A | Internal Workspace Crate |
| `op-network` | Local Path | N/A | N/A | Internal Workspace Crate |
| `op-dynamic-loader` | Local Path | N/A | N/A | Internal Workspace Crate |
| `op-execution-tracker` | Local Path | N/A | N/A | Internal Workspace Crate |
| `tokio` | Workspace | `["full"]` | Yes | **Flagged**: Full async capabilities enabled (workspace-level). |
| `serde` | Workspace | `["derive"]` | Yes | Core Serialization Crate |
| `simd-json` | Workspace | `["serde", "serde_impl"]` | Yes | Zero-copy high-performance JSON library. |
| `anyhow` | Workspace | None | Yes | **Flagged**: Workspace-level error wrapper. |
| `thiserror` | Workspace | None | Yes | **Flagged**: Workspace-level custom error generation. |
| `tracing` | Workspace | None | Yes | Logging/Tracing Facade |
| `async-trait` | Workspace | None | Yes | Async Trait Macro |
| `zbus` | Workspace | `["tokio"]` | Yes | Native D-Bus bindings. |
| `chrono` | Workspace | `["serde"]` | Yes | Date/Time Library |
| `log` | Workspace | None | Yes | Logging Facade |
| `reqwest` | Workspace | `["json", "stream"]` | Yes | HTTP Client |
| `sha2` | Workspace | None | Yes | SHA-2 Hash Functions |
| `md5` | Workspace | None | Yes | **Flagged**: Cryptographically broken hashing algorithm. |
| `uuid` | Workspace | `["v4", "serde"]` | Yes | UUID Generation |
| `dirs` | `5.0` | None | Yes | Unpinned path-resolution crate. |
| `parking_lot` | Workspace | None | Yes | Synchronization Primitives |

### Crate Features
`crates/op-plugins/Cargo.toml` defines no custom `[features]` section. There are no configuration gates (`cfg(feature = ...)`) implemented in the crate.

---

# Storage Backend Check

| Backend | Found at file:line | Role (KV/Graph/Cache/Queue) |
| :--- | :--- | :--- |
| `SqlitePluginCatalog` (SQLite) | `crates/op-plugins/src/registry.rs:14` <br> `crates/op-plugins/src/registry.rs:25` | Schema Catalog Persistence |
| `SqliteStore` (SQLite via `op-state-store`) | `crates/op-plugins/src/default_registry.rs:218` (test) | Local State Store Persistence |

### Architectural Compliance
*   **Knowledge/Graph Storage**: No usage of SQLite/sqlx for graph or knowledge representation was identified.
*   **Sled/Cozo Absence**: No instances of `sled` or `cozo` are directly configured or used in the `op-plugins` codebase, as expected, as this crate acts solely as a coordinating and orchestrating plugin interface.

---

# Security & Quality Findings

## Critical Security Vulnerabilities

### [CRITICAL] Arbitrary Subvolume Exposure via BTRFS Path Traversal
**File**: `crates/op-plugins/src/state_plugins/lxc.rs:238`

#### Description
In `create_container_from_btrfs_snapshot`, the `golden_image_name` string is extracted directly from the user-controlled desired state property `golden_image` without any sanitization or validation. This value is used to construct a host filesystem path:
```rust
let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
```
An attacker with permission to submit desired configurations can leverage directory traversal sequences (e.g., `../../../../etc/some_subvolume` or other host storage directories) to force the host (running as root) to take copy-on-write snapshots of arbitrary BTRFS subvolumes located anywhere on the host filesystem and expose them directly inside the container's rootfs.

#### Remediation
Sanitize the `golden_image_name` parameter to ensure it does not contain directory traversal sequences (such as `..` or `/`). Restrict the input to alphanumeric characters, dashes, and underscores.

---

### [CRITICAL] Arbitrary File Write & Local Privilege Escalation via PCI Address Path Traversal
**File**: `crates/op-plugins/src/state_plugins/pcidecl.rs:152`

#### Description
The `addr` field of `PciItem` is extracted directly from the desired state and used to format a path to write the `driver_override` parameter:
```rust
fn set_driver_override(addr: &str, val: &str) -> Result<()> {
    let p = format!("{}/driver_override", Self::sys_path(addr));
    fs::write(&p, format!("{}\n", val)).context("write driver_override")?;
    Ok(())
}
```
Because `addr` is not validated and can contain path traversal characters, an attacker can specify an arbitrary path (e.g., `../../../../etc/cron.d`) for `addr`. When `set_driver_override` is called, the root-privileged control plane daemon will write the arbitrary `val` string to a file named `driver_override` in that directory (e.g. `/etc/cron.d/driver_override`), resulting in arbitrary file write and local privilege escalation.

#### Remediation
Validate that `addr` matches a strictly defined PCI slot address regular expression (e.g., `^[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-9a-fA-F]$`) before using it in any path construction.

---

### [CRITICAL] Arbitrary Host Command Execution via wg-quick Configuration Injection
**File**: `crates/op-plugins/src/state_plugins/privacy_router.rs:442`

#### Description
The function `validate_wg_quick_config` naively validates WireGuard configuration files by reading a host file specified by `wgcf_config` inside the user-controlled desired state. While it checks for the presence of the `[Interface]` block and `PrivateKey`, it does not check for, strip, or reject WireGuard shell execution hooks such as `PreUp`, `PostUp`, `PreDown`, or `PostDown`. 

An attacker can place a malicious configuration file on the system (e.g. via an upload, shared storage, or container volume), and reference it in `wgcf_config`. When the system runs `wg-quick` as root (on line 437):
```rust
self.run_command("/usr/bin/wg-quick", &["up", config_path]).await?;
```
the system will execute those embedded shell commands on the host with root privileges.

#### Remediation
Do not allow execution of arbitrary, user-pointed configuration files using `wg-quick`. If dynamic interface generation is necessary, use native Netlink/rtnetlink API interfaces (via the `rtnetlink` crate) rather than shell wrappers, or strip all execution hooks (`PreUp`, `PostUp`, etc.) from the configuration file before executing `wg-quick`.

---

## Medium & Low Findings

### [MEDIUM] Broken Shell Operator in Netmaker Installation Command
**File**: `crates/op-plugins/src/state_plugins/netmaker.rs:197`

#### Description
The netmaker plugin attempts to execute multiple commands in a single invocation of `std::process::Command` using the `&&` operator:
```rust
let install_result = Command::new("apt")
    .args(["update", "&&", "apt", "install", "-y", "netclient"])
    .status()
    .await;
```
Since `Command::new` spawns the binary directly and does not run within a shell context, the `&&` character sequence is treated as a literal argument to `apt`. This causes `apt` to fail execution and abort, making `netclient` installation impossible.

#### Remediation
Invoke the update and install commands separately:
```rust
Command::new("apt").args(["update"]).status().await?;
Command::new("apt").args(["install", "-y", "netclient"]).status().await?;
```

---

### [MEDIUM] Blocking Synchronous System Calls on Async Runtime Threads
**File**: `crates/op-plugins/src/dynamic_loading.rs:146`
**File**: `crates/op-plugins/src/dynamic_loading.rs:164`

#### Description
Within the async methods `ensure_btrfs_subvolume` and `get_btrfs_info`, synchronous process execution is performed using `std::process::Command`:
```rust
let output = Command::new("btrfs")
    .arg("subvolume")
    .arg("list")
    .arg(&self.storage_path)
    .output()?;
```
Running synchronous blocking tasks inside Tokio's cooperative poll loop blocks the executor thread, resulting in performance degradation, request starvation, and high latency across the entire control plane.

#### Remediation
Replace `std::process::Command` imports with `tokio::process::Command` and `.await` their results.

---

### [LOW] Invalid CLI Argument Structure for `incus config set`
**File**: `crates/op-plugins/src/state_plugins/incus.rs:289`

#### Description
The `sync_config` method attempts to update container keys using:
```rust
let kv = format!("{}={}", key, value);
Self::run_incus_command(&["config", "set", name, &kv])
```
However, the standard LXD/Incus CLI syntax for setting configuration options is `incus config set <instance> <key> <value>`. Combining them into `key=value` as a single parameter will cause `incus` to fail or interpret the entire string as the key name with no value.

#### Remediation
Pass the key and value as separate arguments:
```rust
Self::run_incus_command(&["config", "set", name, &key, &value])
```

---

### [LOW] Insecure Use of Weak MD5 Hashing for State Fingerprints
**File**: `crates/op-plugins/src/auto_create.rs:85`
**File**: `crates/op-plugins/src/state_plugins/config.rs:141`
**File**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:172`

#### Description
Multiple plugins rely on MD5 hashes to compute state fingerprints for diffing and verifying desired state matches:
```rust
current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
```
MD5 is highly susceptible to collision attacks. If malicious configurations can be engineered to share MD5 hashes, an attacker can mask state drift or unauthorized modifications from the verification loops.

#### Remediation
Transition state hashing to `sha2::Sha256` (which is already a dependency of the crate and is used correctly in several other modules).

---

### [LOW] Fragile Custom Parser for Systemd Unit Files
**File**: `crates/op-plugins/src/state_plugins/service.rs:76`

#### Description
The custom `from_systemd_unit` function naively parses systemd unit files using a simple loop over lines and splits on the `=` character:
```rust
for line in content.lines() {
    let line = line.trim();
    if let Some((k, v)) = line.split_once('=') {
```
This naive parser has multiple structural flaws:
1.  It does not respect commented-out lines starting with `#` or `;`.
2.  It fails to correctly group variables by sections (e.g., `[Unit]` vs `[Service]`), leading to property collisions.
3.  It splits parameters globally by whitespace, breaking arguments that contain spaces inside quotes (e.g., `ExecStart=/bin/echo "hello world"`).

#### Remediation
Replace this hand-rolled parser with a robust, standard INI parser crate (such as `rust-ini`).

---

### [LOW] D-Bus Session Bus Instantiation inside System Context
**File**: `crates/op-plugins/src/state_plugins/keyring.rs:51`

#### Description
`KeyringPlugin` attempts to interface with the secret service using a D-Bus session bus:
```rust
let conn = Connection::session().await?;
```
Daemons running as system services (managed by dinit/systemd) typically run as root or a dedicated system user without an active D-Bus session context. This call will fail under production system contexts, rendering the keyring integration inoperable.

#### Remediation
Allow the plugin to fall back to or explicitly use the system bus if configured, or document the dependency on user-space execution contexts.