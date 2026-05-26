### 1. Public API Surface & Quality Audit

#### Public Items Enumeration & Impact Analysis
The `op-plugins` crate exposes an extensive public API surface, primarily consisting of domain-specific state plugins, trait definitions, and common data models.

* **Total Estimated Public Items**: **264** (including structs, traits, enums, public methods, and re-exports).

The 10 most impactful public items, which define the core extensibility, loading, and runtime topology of the system, are detailed below:

| # | Item Name | Type | File:Line | Impact Description |
|---|---|---|---|---|
| 1 | `Plugin` | `trait` | `crates/op-plugins/src/plugin.rs:136` | Defines the core lifecycle, state reconciliation, and downcasting mechanics for all system plugins. |
| 2 | `PluginRegistry` | `struct` | `crates/op-plugins/src/registry.rs:26` | Indexes live plugin instances, manages D-Bus paths, and mirrors schema definitions into the SQLite store. |
| 3 | `DefaultPluginRegistry` | `struct` | `crates/op-plugins/src/default_registry.rs:59` | Performs bootstrap instantiation of all compiled-in state plugins based on startup configs. |
| 4 | `PrivacyRouterPlugin` | `struct` | `crates/op-plugins/src/state_plugins/privacy_router.rs:164` | Orchestrates the root security and privacy network fabric across host bridges, tunnels, and system containers. |
| 5 | `IncusPlugin` | `struct` | `crates/op-plugins/src/state_plugins/incus.rs:55` | Authoritatively controls physical system containers and VMs via direct interaction with the Incus daemon. |
| 6 | `McpStatePlugin` | `struct` | `crates/op-plugins/src/state_plugins/mcp.rs:150` | Integrates Model Context Protocol (MCP) server topologies with systemic audit trails in the state store. |
| 7 | `NetStatePlugin` | `struct` | `crates/op-plugins/src/state_plugins/net.rs:95` | Controls layer-2 network topology and OpenvSwitch bridges with cryptographic ledger footprinting. |
| 8 | `OpenFlowPlugin` | `struct` | `crates/op-plugins/src/state_plugins/openflow.rs:218` | Manages flow table configurations, policy routing, and traffic obfuscation levels. |
| 9 | `DinitStatePlugin` | `struct` | `crates/op-plugins/src/state_plugins/dinit.rs:94` | Handles declarative state tracking for systems running the Chimera `dinit` supervisor over system D-Bus. |
| 10 | `ServicePlugin` | `struct` | `crates/op-plugins/src/state_plugins/service.rs:37` | Provides init-agnostic service generator helpers and stateful dependency lifecycle analysis. |

#### Glob Re-exports
The crate exposes a public glob re-export inside the `prelude` module:
* `crates/op-plugins/src/lib.rs:43`: `pub use super::state_plugins::*;`

**Risk**: Public glob re-exports pollute the namespace of downstream consumers, increase the risk of naming collisions when new plugins are added, and obscure the precise API surface of the crate.

#### Struct Public Field Encapsulation Violations
Several public structs expose internal fields directly instead of using getter/setter encapsulation. This violates the "validation-on-construction" invariant:

* **`ServiceDef` (`crates/op-plugins/src/service_def.rs:194`)**: This struct performs rigorous validation at parse time (e.g., checking for absolute paths, service name constraints, and resource limits). However, all of its fields (such as `name`, `exec_start`, `user`, `group`, and `resources`) are public (`pub`).
  * **Consequence**: Any client can bypass validation after construction by directly mutating public fields (e.g., `service_def.exec_start.program = PathBuf::from("relative/path")`), violating the runtime integrity of the system.
* **`PluginContext` (`crates/op-plugins/src/plugin.rs:18`)**: Fields such as `storage_path` and `config` are public.
  * **Consequence**: External callers can arbitrarily alter the assigned storage path or configuration payload of a plugin after context assembly, leading to inconsistent state directory assumptions.
* **`PluginTunables` (`crates/op-plugins/src/plugin.rs:37`)**: Fields such as `timeout_ms` and `max_retries` are public.
  * **Consequence**: Allows setting nonsensical or unsafe parameters (e.g., a `timeout_ms` of `0` or negative priorities) without validation bounds checking.

---

### 2. Dead Code Audit

#### `#[allow(dead_code)]` Analysis
The following locations suppress compiler warnings instead of resolving unused code patterns:

* `crates/op-plugins/src/state_plugins/keyring.rs:2`: `#![allow(dead_code)]` applied to the entire module.
* `crates/op-plugins/src/state_plugins/netmaker.rs:188`: `#[allow(dead_code)]` applied to `leave_network`.
* `crates/op-plugins/src/state_plugins/systemd.rs:200`: `#[allow(dead_code)]` applied to `apply_unit`.
* `crates/op-plugins/src/state_plugins/web_ui.rs:341`: `#[allow(dead_code)]` applied to the field `blockchain_sender`.
* `crates/op-plugins/src/state_plugins/net.rs:77`: `#[allow(dead_code)]` on `blockchain_sender`.
* `crates/op-plugins/src/state_plugins/net.rs:82`: `#[allow(dead_code)]` on `new`.
* `crates/op-plugins/src/state_plugins/net.rs:86`: `#[allow(dead_code)]` on `with_blockchain_sender`.
* `crates/op-plugins/src/state_plugins/net.rs:137`: `#[allow(dead_code)]` on `query_current_state_dbus`.
* `crates/op-plugins/src/state_plugins/net.rs:309`: `#[allow(dead_code)]` on `apply_ovs_port_config`.
* `crates/op-plugins/src/state_plugins/net.rs:348`: `#[allow(dead_code)]` on `delete_ovs_bridge`.
* `crates/op-plugins/src/state_plugins/openflow.rs:222`: `#[allow(dead_code)]` on `create_openflow_client`.
* `crates/op-plugins/src/state_plugins/openflow.rs:398`: `#[allow(dead_code)]` on `parse_flows`.
* `crates/op-plugins/src/state_plugins/openflow.rs:417`: `#[allow(dead_code)]` on `parse_flow_line`.
* `crates/op-plugins/src/state_plugins/openflow.rs:461`: `#[allow(dead_code)]` on `parse_actions`.
* `crates/op-plugins/src/state_plugins/openflow.rs:497`: `#[allow(dead_code)]` on `flow_to_string`.
* `crates/op-plugins/src/state_plugins/openflow.rs:521`: `#[allow(dead_code)]` on `action_to_string`.

#### Unreferenced Code Definitions
* **`SystemdAutoCreator` (`crates/op-plugins/src/auto_create.rs:14`)**: Struct and its `discover_units` function are defined but never used, referenced, or re-exported.
* **`EchoPlugin` (`crates/op-plugins/src/builtin.rs:13`)**: Struct and its `new` and `Default` implementations are defined but never instantiated or registered in the registry.
* **`SystemdPlugin` (`crates/op-plugins/src/service_def.rs:460`)**: Defined within the systemd service definitions but completely omitted from the active state plugin registry.
* **`ServicePlugin::convert_systemd_to_dinit` (`crates/op-plugins/src/state_plugins/service.rs:131`)**: Defined but never called anywhere in the codebase.
* **Commented-out Modules (`crates/op-plugins/src/state_plugins/mod.rs:5-17`)**: The modules `dnsresolver`, `full_system`, `keyring`, `login1`, `lxc`, `netmaker`, `openflow_obfuscation`, `packagekit`, `pcidecl`, `privacy`, `systemd`, and `systemd_networkd` are present as files in the repository but have their `pub mod` declarations commented out in the parent module, making their code entirely unreachable.

#### Dead Code Table

| Item | Type | File:Line | Recommendation |
|---|---|---|---|
| `_config_path` | Unused variable | `crates/op-plugins/src/default_registry.rs:133` | Remove unused variable assignment. |
| `SystemdAutoCreator` | Struct | `crates/op-plugins/src/auto_create.rs:14` | Remove. |
| `SystemdAutoCreator::discover_units` | Function | `crates/op-plugins/src/auto_create.rs:18` | Remove. |
| `EchoPlugin` | Struct | `crates/op-plugins/src/builtin.rs:13` | Expose for tests or remove. |
| `SystemdPlugin` | Struct | `crates/op-plugins/src/service_def.rs:460` | Remove or integrate with active supervisor modules. |
| `ServicePlugin::convert_systemd_to_dinit` | Function | `crates/op-plugins/src/state_plugins/service.rs:131` | Expose via D-Bus commands or remove. |
| `dnsresolver` | Module File | `crates/op-plugins/src/state_plugins/dnsresolver.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `full_system` | Module File | `crates/op-plugins/src/state_plugins/full_system.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `keyring` | Module File | `crates/op-plugins/src/state_plugins/keyring.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `login1` | Module File | `crates/op-plugins/src/state_plugins/login1.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `lxc` | Module File | `crates/op-plugins/src/state_plugins/lxc.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `netmaker` | Module File | `crates/op-plugins/src/state_plugins/netmaker.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `openflow_obfuscation` | Module File | `crates/op-plugins/src/state_plugins/openflow_obfuscation.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `packagekit` | Module File | `crates/op-plugins/src/state_plugins/packagekit.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `pcidecl` | Module File | `crates/op-plugins/src/state_plugins/pcidecl.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `privacy` | Module File | `crates/op-plugins/src/state_plugins/privacy.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `systemd` | Module File | `crates/op-plugins/src/state_plugins/systemd.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |
| `systemd_networkd` | Module File | `crates/op-plugins/src/state_plugins/systemd_networkd.rs:1` | Expose in `mod.rs` if needed, otherwise remove. |

---

### 3. Production Security & Quality Audit

#### CRITICAL: Host Command Execution & Arbitrary File Write via BTRFS/LXC Path Traversal
* **File:Line**: `crates/op-plugins/src/state_plugins/privacy_router.rs:588` (also replicated in commented-out module `crates/op-plugins/src/state_plugins/lxc.rs:462`)
* **Vulnerability Type**: Path Traversal / Remote Code Execution (RCE)
* **Direct Exploitability**: Directly exploitable via any client capable of writing to the desired state configuration (e.g., via D-Bus, config updates, or public-facing API triggers).
* **Detailed Analysis**:
  In `PrivacyRouterPlugin::inject_firstboot_script`, the path to the target container's root filesystem is dynamically assembled using the `storage` variable:
  ```rust
  let rootfs = format!("/var/lib/pve/{}/images/{}/rootfs", storage, container.id);
  let script_path = format!("{}/usr/local/bin/lxc-firstboot.sh", rootfs);
  let service_path = format!("{}/etc/systemd/system/lxc-firstboot.service", rootfs);
  ```
  The value of `storage` is loaded directly from desired state parameters (`desired` JSON structure) with no validation or directory traversal checks:
  ```rust
  let storage = props.and_then(|p| p.get("storage")).and_then(|v| v.as_str()).unwrap_or("local-btrfs");
  ```
  An attacker can specify `"storage": "../../../.."` (or similar relative sequences) within their desired container properties. This resolves `rootfs` to `/var/lib/pve/../../../../etc/images/{container.id}/rootfs`, which evaluates to `/etc/images/{container.id}/rootfs`. 
  
  By calibrating the path traversal payload, the attacker can cause the plugin (which runs with root/system-level privileges to perform LXC and OVS operations) to write an arbitrary script directly to the host's `/usr/local/bin/lxc-firstboot.sh` and a systemd unit directly to `/etc/systemd/system/lxc-firstboot.service`. Because the plugin subsequently executes:
  ```rust
  tokio::process::Command::new("chmod").args(["+x", &script_path]).output().await?;
  ```
  the host will register and mark the malicious payload as executable. When the container boots or when systemd reloads, the script will execute on the host context with root privileges, leading to complete host compromise.

* **Remediation**:
  1. Enforce strict sanitization on the `storage` string. Reject any input containing directory separators (`/`, `\`) or traversal elements (`..`).
  2. Resolve the path dynamically using `std::fs::canonicalize` and assert that the canonicalized path starts with the designated base directory (e.g., `/var/lib/pve/`):
     ```rust
     let base_dir = Path::new("/var/lib/pve").join(storage);
     if !base_dir.starts_with("/var/lib/pve/") {
         bail!("Directory traversal attempt detected.");
     }
     ```

---

#### MAJOR: Arbitrary Host File Overwrite via Unsanitized Config Paths
* **File:Line**: `crates/op-plugins/src/default_registry.rs:125` (coupled with `crates/op-plugins/src/state_plugins/config.rs:48`)
* **Vulnerability Type**: Privilege Escalation / Arbitrary File Overwrite
* **Direct Exploitability**: Exploitable by users/processes with permission to configure runtime plugin tunables or edit `/etc/op-dbus/` configs.
* **Detailed Analysis**:
  The `DefaultPluginRegistry` instantiates the global `ConfigPlugin` and `McpStatePlugin` by resolving the configuration path from the user-controlled `plugin_configs` map:
  ```rust
  "config" => {
      let config_path = self.get_plugin_config_path("config", "/etc/op-dbus/config-store.json");
      Arc::new(ConfigPlugin::new(config_path))
  }
  ```
  This path is subsequently used by `ConfigPlugin::save_store` to serialize the global key-value configuration state to disk:
  ```rust
  let content = simd_json::to_string_pretty(state).context("serialize config store")?;
  tokio::fs::write(&self.store_path, content).await.context("write config store")?;
  ```
  Because the `config_path` is not sanitized, restricted to a sandbox, or checked for canonical root equivalence, an attacker can specify a critical system target (such as `/etc/shadow`, `/etc/passwd`, or `/etc/cron.d/malicious_cron`) as the `config_path`. 
  
  When the plugin next synchronizes or reconciles its state, it will overwrite the target system file with JSON data. Overwriting `/etc/shadow` with JSON bricks system authentication, while overwriting `/etc/cron.d/` triggers parsing errors or allows command execution depending on the supervisor configuration.

* **Remediation**:
  1. Enforce a strict prefix check on `config_path` to ensure it resides only within `/etc/op-dbus/` or `/var/lib/op-dbus/`.
  2. Reject absolute paths or traversal sequences (`..`) in user-supplied configuration values.

---

#### MEDIUM: Command Flag Hijacking (Argument Injection) in OpenFlow Switch Management
* **File:Line**: `crates/op-plugins/src/state_plugins/openflow.rs:372` (and line 390)
* **Vulnerability Type**: Argument Injection
* **Direct Exploitability**: Exploitable if an attacker can register or name an OVS bridge with leading hyphens.
* **Detailed Analysis**:
  The `OpenFlowPlugin` executes `ovs-ofctl` commands by passing the bridge name as a positional parameter:
  ```rust
  Self::run_ovs_ofctl(&["-O", OPENFLOW_PROTOCOL, "add-flow", bridge, &rule]).await?;
  ```
  Although `Command::new` is executed directly without shell invocation (preventing shell injection), passing a bridge name that begins with a hyphen (e.g., `--help` or `--private-key`) causes `ovs-ofctl` to interpret the parameter as a command-line flag rather than a positional argument. An attacker who can control bridge naming schemas can inject arbitrary switch configurations or cause command failures.

* **Remediation**:
  Validate that all bridge names conform to strict alphanumeric naming rules (e.g., matching the regex `^[a-zA-Z0-9_-]+$`), and explicitly use the `--` double-dash argument separator where supported to isolate positional arguments from preceding flags.