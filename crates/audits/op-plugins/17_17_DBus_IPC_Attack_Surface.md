# Production Security & Quality Audit: op-plugins

This document presents a production security and quality audit of the `op-plugins` crate. The audit focuses on the D-Bus and IPC attack surface, command/argument injection vulnerabilities, and compliance with schema-as-code discipline.

---

## 1. D-Bus & IPC Attack Surface Catalog

The `op-plugins` crate acts as a heavy consumer of system-level IPC and registers its own runtime plugins onto D-Bus. Below is the catalog of every D-Bus interface, method, and signal registered or connected to within the provided files.

### 1.1 Registered & Connected D-Bus Interfaces

| Interface / Service | Path | Role | Methods / Properties Accessed / Registered | Caller Identity Verification | Bus Type | Citations |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `org.opdbus.v1` | `/org/opdbus/v1/plugins/{sanitized_name}` | **Registered Server** (Registers `PluginDbusHost` for each plugin) | Method calls delegated to `StatePlugin` implementations. | **None**. The registration does not perform runtime UID/credentials validation. | System Bus | `crates/op-plugins/src/registry.rs:126-137` |
| `org.chimera.dinit.Manager` | `/org/chimera/dinit` | Connected Client (Proxy) | `StartService`, `StopService`, `GetServiceStatus`, `ListServices` | **None**. The plugin blindly trust system D-Bus response structures and passes unvalidated state changes. | System Bus | `crates/op-plugins/src/state_plugins/dinit.rs:31-52`, `crates/op-plugins/src/state_plugins/service.rs:48-56` |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | Connected Client (Proxy) | `"GetUnit"`, `"GetUnitFileState"`, `"StartUnit"`, `"StopUnit"`, `"EnableUnitFiles"`, `"DisableUnitFiles"`, `"MaskUnitFiles"`, `"UnmaskUnitFiles"` | **None**. Interacts directly with privileged systemd manager without verifying caller authorization context. | System Bus | `crates/op-plugins/src/state_plugins/systemd.rs:44-55` |
| `org.freedesktop.systemd1.Unit` | Dynamic (e.g. `/org/freedesktop/systemd1/unit/...`) | Connected Client (Proxy) | `"ActiveState"` (Property), `"Reload"` | **None**. | System Bus | `crates/op-plugins/src/state_plugins/systemd.rs:67-73`, `crates/op-plugins/src/state_plugins/systemd_networkd.rs:104-113` |
| `org.freedesktop.network1.Manager` | `/org/freedesktop/network1` | Connected Client (Proxy) | `"ListLinks"` | **None**. | System Bus | `crates/op-plugins/src/state_plugins/systemd_networkd.rs:125-131` |
| `org.freedesktop.login1.Manager` | `/org/freedesktop/login1` | Connected Client (Proxy) | `"ListSessions"` | **None**. | System Bus | `crates/op-plugins/src/state_plugins/login1.rs:35-41` |
| `org.freedesktop.secrets` | `/org/freedesktop/secrets` | Connected Client (Proxy) | `"Collections"`, `"ReadAlias"` | **None**. | Session Bus | `crates/op-plugins/src/state_plugins/keyring.rs:54-61` |
| `org.freedesktop.Secret.Collection` | Dynamic | Connected Client (Proxy) | `"Label"`, `"Locked"`, `"Created"`, `"Modified"` (Properties) | **None**. | Session Bus | `crates/op-plugins/src/state_plugins/keyring.rs:77-84` |
| `org.freedesktop.PackageKit` | `/org/freedesktop/PackageKit` | Connected Client (Proxy) | `get_transaction_list`, `create_transaction` | **None**. | System Bus | `crates/op-plugins/src/state_plugins/packagekit.rs:18-26` |
| `org.freedesktop.PackageKit.Transaction` | Dynamic | Connected Client (Proxy) | `install_packages`, `remove_packages`, `resolve` | **None**. | System Bus | `crates/op-plugins/src/state_plugins/packagekit.rs:32-47` |

### 1.2 Over-Permissioned & Trust Model Analysis

The `PluginRegistry` registers `PluginDbusHost` on the system bus (`crates/op-plugins/src/registry.rs:126-137`). There are no credentials or caller security context validations. If the system bus policy (`org.opdbus.v1.conf`) is over-permissioned or missing restrictive `send_destination` policies, **any local unprivileged process can invoke mutation and command-execution endpoints** on the system bus.

---

## 2. Vulnerability Findings

### 2.1 Critical: Remote/Local Command Execution via Shell Injection in `PciDeclPlugin`
* **File:** `crates/op-plugins/src/state_plugins/pcidecl.rs`
* **Lines:** 92-97
* **Severity:** Critical
* **Exploitability:** Directly exploitable by providing a malicious PCI address containing shell metacharacters in the desired state.

#### Description
The `pcidecl` state plugin executes a shell command to verify the presence of a PCI device using the `lspci` command. The address is formatted directly into a string template and executed via a shell (`sh -c`):

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

The `addr` parameter is parsed directly from the deserialized `desired` state without any validation or sanitization:
```rust
let want: PciDecl =
    simd_json::serde::from_owned_value(desired.clone()).context("desired must be PciDecl")?;
```

If an attacker controls the desired state (e.g. through the D-Bus interface `set_desired_state`), they can supply a payload where `address` contains a command payload (for example, `; id > /tmp/pwned ;`). Because `sh` evaluates the string, this executes arbitrary shell commands with the privileges of the parent process (likely `root`, given its capability to modify drivers/devices).

#### Remediation
Remove the dependency on shell execution. Execute the binary directly with safe argument passing (which avoids shell evaluation), or parse `/sys/bus/pci/devices/` directly as done in `live_for` (`crates/op-plugins/src/state_plugins/pcidecl.rs:65`):

```rust
fn lspci_present(addr: &str) -> bool {
    // Validate that addr conforms strictly to a PCI address regex: ^[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-9a-fA-F]$
    // Alternatively, call the binary directly:
    Command::new("lspci")
        .args(["-s", addr])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
```

---

### 2.2 Critical: Arbitrary Command Execution via Flag/Argv Injection in `PackageKitPlugin`
* **File:** `crates/op-plugins/src/state_plugins/packagekit.rs`
* **Lines:** 77-83, 114-120
* **Severity:** Critical
* **Exploitability:** Exploitable by providing package names in the desired state starting with dashes (e.g., `-oAPT::Update::Pre-Invoke::=...`).

#### Description
The `PackageKit` plugin falls back to direct package managers (`apt-get`, `dnf`, `pacman`) when PackageKit D-Bus operations are unavailable. It passes the `package_name` (supplied from the desired state) directly as an argument:

```rust
async fn install_via_direct(&self, package_name: &str) -> Result<()> {
    // Try apt
    if Command::new("apt-get")
        .args(["install", "-y", package_name])
        .status()?
        .success()
    {
        return Ok(());
    }
    ...
```

The `package_name` is taken directly from the deserialized `desired` configuration without verification that it represents a valid package name and is not a command line flag. By submitting a package name such as:
`-oAPT::Update::Pre-Invoke::="touch /tmp/pwned"` or `-c/tmp/evil.conf`

An attacker can force `apt-get` to load malicious configuration files or execute arbitrary shell commands during package manager instantiation, bypassing the shell-less command constraints.

#### Remediation
Enforce strict character validation on `package_name` to prevent flag injection (e.g., must not start with `-` and must match `^[a-zA-Z0-9_\-\.\+]+$`).

```rust
fn validate_package_name(name: &str) -> Result<()> {
    if name.starts_with('-') {
        anyhow::bail!("Invalid package name: cannot start with a dash");
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '+') {
        anyhow::bail!("Invalid characters in package name");
    }
    Ok(())
}
```

---

### 2.3 High: Argv/Flag Injection in `IncusPlugin::apply_create`
* **File:** `crates/op-plugins/src/state_plugins/incus.rs`
* **Lines:** 118-128
* **Severity:** High
* **Exploitability:** Exploitable by setting `image` or `name` in the desired state to modify the behavior of the `/usr/bin/incus` execution.

#### Description
The `IncusPlugin` builds execution arguments for `/usr/bin/incus` dynamically:

```rust
let mut create_args = vec!["init".to_string(), image.to_string(), name.to_string()];
if let Some(pool) = instance.storage_pool.as_deref() {
    create_args.push("--storage".to_string());
    create_args.push(pool.to_string());
}
```

Because the `name` and `image` strings are never validated to ensure they do not start with a dash (`-`), an attacker who can modify the desired state can inject arbitrary command-line options into the `incus init` invocation. This could allow them to pass option overrides (such as profiles or security configurations) that escape the expected bounds of container confinement.

#### Remediation
Implement validation similar to the `ServiceName` constraints on all fields that will map to command arguments (specifically `name`, `image`, and `storage_pool`):

```rust
if name.starts_with('-') || image.starts_with('-') {
    anyhow::bail!("Flag injection detected in name or image");
}
```

---

### 2.4 High: BTRFS Path Traversal in `LxcPlugin::create_container_from_btrfs_snapshot`
* **File:** `crates/op-plugins/src/state_plugins/lxc.rs`
* **Lines:** 294-315
* **Severity:** High
* **Exploitability:** Exploitable by providing a `golden_image` property with relative directory traversal components (`../`).

#### Description
When creating an LXC container from a BTRFS snapshot, the plugin parses a `golden_image` parameter and interpolates it directly to construct `golden_image_path`:

```rust
let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
```

No validation is performed on the `golden_image_name` to prevent path traversal. An attacker could supply a value like `../../../../any/btrfs/volume` to snapshot and duplicate arbitrary BTRFS subvolumes outside the `/var/lib/pve/` directory structure.

#### Remediation
Sanitize the `golden_image_name` to ensure it represents a single, valid filename without path traversal segments:

```rust
let path = std::path::Path::new(golden_image_name);
if path.components().count() != 1 || golden_image_name.contains("..") {
    anyhow::bail!("Invalid golden image name");
}
```

---

## 3. Schema-as-Code Compliance Gap Analysis

The codebase claims to adhere to a **schema-as-code** discipline (utilizing versioned schemas like Protocol Buffers and OSCAL). However, an audit of the provided files reveals **zero integration of versioned schema files (no `.proto` files, no OSCAL files, and no compiled bindings)**. Instead, data contracts are represented entirely as ad-hoc, manually written Rust structures with Serde serialization and Rust-based programmatic schema builders.

### 3.1 Ad-Hoc Data Contracts

The following locations express data contracts as unversioned Rust structs, introducing serialization risks and violating schema-as-code discipline:

1. **`ChatMessage` & `ChatRequest`** (`crates/op-plugins/src/chat.rs:9-85`):
   Data contracts for LLM communication are implemented as raw Rust structs. In a schema-as-code architecture, these should be generated from a canonical versioned schema.
2. **`ServiceDef`** (`crates/op-plugins/src/service_def.rs:114`):
   Ad-hoc structure defining system services and translating them to dinit scripts, bypassing declarative validation schemas.
3. **`IncusState`** (`crates/op-plugins/src/state_plugins/incus.rs:18-43`):
   Ad-hoc representations of Incus/LXD virtualization instances.
4. **`McpConfig`** (`crates/op-plugins/src/state_plugins/mcp.rs:14-92`):
   Model Configurations and Tool Groups are defined as standard Rust structs rather than OSCAL-compliant or Protobuf profiles.
5. **`OvsBridgeState`** (`crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:15-61`):
   Ad-hoc representation of OVS DB structures.
6. **`RtnetlinkState`** (`crates/op-plugins/src/state_plugins/rtnetlink.rs:14-43`):
   Ad-hoc representation of netlink interface states.
7. **`FullSystemState`** (`crates/op-plugins/src/state_plugins/full_system.rs:26-144`):
   An extremely sensitive contract collecting the *complete* system state (including kernel, timezone, users, block devices, and containers) into an ad-hoc Rust struct without any versioned schema enforcement or cryptographic schema validation.

### 3.2 Programmatic Schema Builders

Instead of deriving contracts from versioned schema files, schemas are built using a programmatic Rust builder in `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs` (e.g. `openflow_plugin_schema` on Line 367 and `privacy_router_plugin_schema` on Line 669). 

This approach couples the definition of data contracts directly to the Rust compiler and codebase, making it impossible for external schema engines, policy compilers, or multi-language services to dynamically validate configurations without replicating the Rust code.

### 3.3 Recommendation for Compliance
1. Define all state models and IPC payloads using versioned **Protocol Buffers** (`.proto`) in a centralized `/schemas` workspace directory.
2. Generate the Rust structs automatically using `prost-build` (already present in the workspace but bypassed here) during the build stage.
3. Integrate an OSCAL assessment module to mapping these configuration states to controls automatically.