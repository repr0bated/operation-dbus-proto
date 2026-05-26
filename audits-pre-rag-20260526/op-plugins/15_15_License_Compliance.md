# License Audit

## Extracted License
*   **Workspace License**: `Apache-2.0` (defined in workspace `Cargo.toml`, inherited by `op-plugins` via `license.workspace = true`)

## GPL/AGPL/SSPL Crate Scan
*   **Result**: No GPL, AGPL, or SSPL licensed crates were found in the scanned portions of `Cargo.lock`. All listed dependencies use permissive licenses (such as MIT, Apache-2.0, BSD, or CC0).

## Crates with Missing License Field
*   **Result**: All internal workspace crates specify `license.workspace = true` or have explicit permissive licenses. No crates lacking a license field were identified.

---

# Production Security & Quality Audit

## Critical Risk Findings

### Undefined Behavior / Out-of-Bounds Read in `simd_json::from_str` and `from_slice`
*   **Citations**: 
    *   `crates/op-plugins/src/state_plugins/config.rs:36`
    *   `crates/op-plugins/src/state_plugins/incus.rs:107`
    *   `crates/op-plugins/src/state_plugins/mcp.rs:163`
    *   `crates/op-plugins/src/state_plugins/privacy_routes.rs:48`
    *   `crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:141`
    *   `crates/op-plugins/src/state_plugins/privacy_router.rs:356`
    *   `crates/op-plugins/src/state_plugins/net.rs:188`
    *   `crates/op-plugins/src/state_plugins/full_system.rs:569`
*   **Analysis**: The `simd-json` crate requires that the input slice passed to the parser (including `from_str` and `from_slice`) is padded with `simd_json::SIMDJSON_PADDING` bytes. If unpadded data is parsed, the SIMD vector instructions can read past the end of the buffer, causing Undefined Behavior, memory leakage, or segmentation faults. 
    Across multiple files, local string or vector buffers are read directly from files or command outputs (e.g., `tokio::fs::read_to_string` or command stdout) and passed directly to `simd_json::from_str` or `simd_json::from_slice` within `unsafe` blocks without ensuring padding.
*   **Remediation**: Use `simd_json::serde::from_slice` (which handles copying and padding automatically in safe Rust) or allocate a vector with explicit padding using `simd_json::to_padded_bin`.

---

### Path Traversal / Arbitrary BTRFS Subvolume Snapshotting
*   **Citation**: `crates/op-plugins/src/state_plugins/lxc.rs:504`
*   **Analysis**: The LXC plugin constructs a `golden_image_path` using `golden_image_name` directly extracted from properties without sanitization:
    ```rust
    let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
    ```
    An attacker who can modify container configurations can inject path traversal sequences (e.g., `../../../../other_subvolume`) to target an arbitrary BTRFS subvolume on the system. This allows unauthorized copying/snapshotting of other users' containers or sensitive system directories.
*   **Remediation**: Validate `golden_image_name` to ensure it does not contain directory traversal sequences (such as `..` or `/`) and matches a strict alphanumeric pattern before formatting the path.

---

### Command Injection via `wg-quick` Configuration Hooks
*   **Citation**: `crates/op-plugins/src/state_plugins/privacy_router.rs:396`
*   **Analysis**: The `validate_wg_quick_config` function validates a WireGuard configuration file before calling `wg-quick up`. However, it only checks for `[Interface]`, `PrivateKey`, and `Table = off`.
    The `wg-quick` tool natively executes shell commands defined in parameters like `PreUp`, `PostUp`, `PreDown`, and `PostDown`. An attacker who can write or modify the targeted WireGuard configuration file can inject arbitrary shell commands that will be executed as `root` when the bridge starts.
*   **Remediation**: Update `validate_wg_quick_config` to explicitly reject any configuration file containing execution hooks such as `PreUp`, `PostUp`, `PreDown`, `PostDown`, `FwdDistFarking`, etc.

---

### Argument / Option Injection in Package Managers
*   **Citations**:
    *   `crates/op-plugins/src/state_plugins/packagekit.rs:81`
    *   `crates/op-plugins/src/state_plugins/packagekit.rs:105`
*   **Analysis**: In `install_via_direct` and `remove_via_direct`, package names are passed directly to `Command::new` arguments:
    ```rust
    Command::new("apt-get").args(["install", "-y", package_name])
    ```
    If `package_name` starts with a hyphen (e.g., `-oAPT::Sandbox::User=root`), it can reconfigure the underlying package manager's options, potentially enabling sandbox escapes or arbitrary configuration changes during package installation.
*   **Remediation**: Append a double-hyphen `--` separator before the package name to signal the end of command-line options:
    ```rust
    Command::new("apt-get").args(["install", "-y", "--", package_name])
    ```

---

## Medium & Low Risk Findings

### Keyring Connection to User Session Bus in System Daemon Context
*   **Citation**: `crates/op-plugins/src/state_plugins/keyring.rs:48`
*   **Analysis**: The `KeyringPlugin` calls `Connection::session().await?` to communicate with the freedesktop Secret Service. In a system daemon context (where the agent typically runs as `root` or a dedicated system user), there is no active session bus. It will either fail to connect or accidentally bind to an arbitrary user's session bus if environment variables like `DBUS_SESSION_BUS_ADDRESS` are leaked, leading to privilege confusion or data exposure.
*   **Remediation**: Avoid using session-level buses in system contexts. Restrict keyring access to explicit, sandbox-isolated user processes or validate connection environments strictly.

---

### Non-Atomic Configuration File Writes
*   **Citation**: `crates/op-plugins/src/state_plugins/config.rs:55`
*   **Analysis**: Writing the global configuration file directly via `tokio::fs::write` is not atomic. If the system crashes, runs out of disk space, or is interrupted during the write operation, the configuration file will be left partially written and corrupted.
*   **Remediation**: Write to a temporary file in the same directory first, flush it to disk, and then perform an atomic rename using `std::fs::rename`.

---

### Redundant Subprocess Spawning / Use of `cat`
*   **Citation**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:89`
*   **Analysis**: Spawning a `cat` subprocess to read `/etc/resolv.conf` is highly inefficient and resource-heavy:
    ```rust
    Command::new("cat").arg("/etc/resolv.conf").output()
    ```
    It should rely purely on `fs::read_to_string`, which is already implemented as the fallback.
*   **Remediation**: Remove the `Command::new("cat")` block entirely and use `fs::read_to_string` directly.

---

### Insecure `sh -c` Wrapper for File Moving
*   **Citation**: `crates/op-plugins/src/state_plugins/dnsresolver.rs:126`
*   **Analysis**: The DNS resolver plugin uses a shell wrapper to move files:
    ```rust
    let mv_cmd = format!("mv -f {} /etc/resolv.conf", tmp_path);
    Command::new("sh").arg("-c").arg(&mv_cmd)...
    ```
    Although `tmp_path` is currently constant, using `sh -c` to format shell commands is an anti-pattern that can lead to command injection if paths become dynamic. Furthermore, spawning a shell is unnecessary.
*   **Remediation**: Use `std::fs::rename` directly to move the file atomically on the filesystem.